use crate::models::{
    CandidateSource, CandidateTitle, CategoryScores, ExtractMethod, ExtractedDocument, LayoutBlock,
    NormalizedBox, ParagraphBlock, RuleDetail, ScoreDecision, ScoringProfile, ScoringResult,
};
use regex::Regex;

const MIN_AUTO_CANDIDATE_SCORE: u8 = 45;

pub fn score_document(extracted: ExtractedDocument, profile: ScoringProfile) -> ScoringResult {
    let mut candidates = collect_candidates(&extracted, &profile);
    candidates.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.text.cmp(&b.text)));

    let best = candidates.first();
    let confidence = best.map_or(0, |candidate| candidate.score);
    let final_title = best
        .filter(|candidate| candidate.score >= MIN_AUTO_CANDIDATE_SCORE)
        .map(|candidate| candidate.text.clone());
    let decision = match final_title {
        Some(_) if confidence >= profile.auto_output_threshold => ScoreDecision::AutoOutput,
        Some(_) => ScoreDecision::Pending,
        None => ScoreDecision::Failed,
    };

    ScoringResult {
        final_title,
        confidence,
        candidates,
        decision,
    }
}

fn collect_candidates(
    extracted: &ExtractedDocument,
    profile: &ScoringProfile,
) -> Vec<CandidateTitle> {
    if !extracted.pages.is_empty() {
        let mut candidates = vec![];
        for page in &extracted.pages {
            candidates.extend(page.blocks.iter().filter_map(|block| {
                score_layout_block(block, page.page_index, &extracted.extract_method, profile)
            }));
            candidates.extend(score_multi_line_title_candidates(
                &page.blocks,
                page.page_index,
                &extracted.extract_method,
                profile,
            ));
        }
        return candidates;
    }

    collect_word_candidates(extracted, profile)
}

fn collect_word_candidates(
    extracted: &ExtractedDocument,
    profile: &ScoringProfile,
) -> Vec<CandidateTitle> {
    let paragraphs = extracted
        .paragraphs
        .iter()
        .filter(|paragraph| !clean_candidate_text(&paragraph.text).is_empty())
        .take(10)
        .collect::<Vec<_>>();

    let mut candidates = paragraphs
        .iter()
        .filter_map(|paragraph| score_paragraph(paragraph, profile))
        .collect::<Vec<_>>();
    candidates.extend(score_two_paragraph_title_candidates(&paragraphs, profile));
    candidates
}

fn score_two_paragraph_title_candidates(
    paragraphs: &[&ParagraphBlock],
    profile: &ScoringProfile,
) -> Vec<CandidateTitle> {
    paragraphs
        .windows(2)
        .filter_map(|pair| {
            build_two_paragraph_title(pair[0], pair[1], profile.max_title_chars).and_then(
                |combined| {
                    let mut candidate = score_paragraph(&combined, profile)?;
                    candidate.rule_details.push(RuleDetail {
                        rule_name: "word-two-paragraph-title".into(),
                        category: "textQuality".into(),
                        delta: 12,
                        description: "相邻 Word 段落合并为标题候选".into(),
                    });
                    candidate.category_scores.text_quality += 12;
                    candidate.score = candidate.score.saturating_add(12).min(100);
                    Some(candidate)
                },
            )
        })
        .collect()
}

fn build_two_paragraph_title(
    first: &ParagraphBlock,
    second: &ParagraphBlock,
    max_title_chars: u16,
) -> Option<ParagraphBlock> {
    let first_text = clean_candidate_text(&first.text);
    let second_text = clean_candidate_text(&second.text);
    if first_text.is_empty()
        || second_text.is_empty()
        || is_symbol_only_noise(&first_text)
        || is_symbol_only_noise(&second_text)
        || looks_like_noise(&first_text)
        || looks_like_noise(&second_text)
        || looks_like_word_body_continuation(&first_text)
    {
        return None;
    }

    let combined_text = format!(
        "{}{}",
        first_text.split_whitespace().collect::<String>(),
        second_text.split_whitespace().collect::<String>()
    );
    let combined_char_count = combined_text.chars().count();
    if combined_char_count < 6
        || combined_char_count > max_title_chars as usize
        || looks_like_sentence(&combined_text)
        || looks_like_promulgation_sentence(&combined_text)
        || !looks_like_word_title_continuation(&second_text)
    {
        return None;
    }

    Some(ParagraphBlock {
        text: combined_text,
        paragraph_index: first.paragraph_index,
    })
}

fn score_multi_line_title_candidates(
    blocks: &[LayoutBlock],
    page_index: usize,
    extract_method: &ExtractMethod,
    profile: &ScoringProfile,
) -> Vec<CandidateTitle> {
    let mut ordered_blocks = blocks.iter().collect::<Vec<_>>();
    ordered_blocks.sort_by(|a, b| {
        a.bbox
            .y0
            .total_cmp(&b.bbox.y0)
            .then_with(|| a.bbox.x0.total_cmp(&b.bbox.x0))
    });

    let mut candidates = vec![];
    for line_count in 2..=3 {
        candidates.extend(ordered_blocks.windows(line_count).filter_map(|lines| {
            build_multi_line_title_block(lines, profile.max_title_chars).and_then(|combined| {
                let mut candidate =
                    score_layout_block(&combined, page_index, extract_method, profile)?;
                let rule_name = if line_count == 2 {
                    "layout-two-line-title"
                } else {
                    "layout-three-line-title"
                };
                let delta = if line_count == 2 { 8 } else { 12 };
                candidate.rule_details.push(RuleDetail {
                    rule_name: rule_name.into(),
                    category: "layout".into(),
                    delta,
                    description: if line_count == 2 {
                        "相邻两行版式接近，合并为标题候选".into()
                    } else {
                        "相邻三行版式接近，合并为标题候选".into()
                    },
                });
                candidate.category_scores.layout += delta;
                candidate.score = candidate.score.saturating_add(delta as u8).min(100);
                Some(candidate)
            })
        }));
    }

    candidates
}

fn build_multi_line_title_block(
    lines: &[&LayoutBlock],
    max_title_chars: u16,
) -> Option<LayoutBlock> {
    if !(2..=3).contains(&lines.len()) {
        return None;
    }

    let line_texts = lines
        .iter()
        .map(|line| clean_candidate_text(&line.text))
        .collect::<Vec<_>>();
    if line_texts.iter().any(|text| {
        text.is_empty()
            || is_symbol_only_noise(text)
            || looks_like_noise(text)
            || looks_like_sentence(text)
    }) {
        return None;
    }

    let combined_text = line_texts
        .iter()
        .map(|text| text.split_whitespace().collect::<String>())
        .collect::<String>();
    let combined_char_count = combined_text.chars().count();
    if combined_char_count < 6
        || combined_char_count > max_title_chars as usize
        || looks_like_sentence(&combined_text)
        || looks_like_promulgation_sentence(&combined_text)
    {
        return None;
    }

    if !lines
        .windows(2)
        .all(|pair| two_lines_have_title_geometry(pair[0], pair[1]))
    {
        return None;
    }

    Some(LayoutBlock {
        text: combined_text,
        bbox: NormalizedBox {
            x0: lines
                .iter()
                .map(|line| line.bbox.x0)
                .fold(f32::INFINITY, f32::min),
            y0: lines
                .iter()
                .map(|line| line.bbox.y0)
                .fold(f32::INFINITY, f32::min),
            x1: lines
                .iter()
                .map(|line| line.bbox.x1)
                .fold(f32::NEG_INFINITY, f32::max),
            y1: lines
                .iter()
                .map(|line| line.bbox.y1)
                .fold(f32::NEG_INFINITY, f32::max),
        },
        raw_bbox: None,
        font_size: average_optional_values(lines.iter().filter_map(|line| line.font_size)),
        bold: merge_optional_bold(lines.iter().map(|line| line.bold)),
        ocr_confidence: average_optional_values(
            lines.iter().filter_map(|line| line.ocr_confidence),
        ),
        line_index: None,
    })
}

fn two_lines_have_title_geometry(first: &LayoutBlock, second: &LayoutBlock) -> bool {
    let first_mid_y = (first.bbox.y0 + first.bbox.y1) / 2.0;
    let second_mid_y = (second.bbox.y0 + second.bbox.y1) / 2.0;
    if second_mid_y <= first_mid_y {
        return false;
    }

    let first_height = (first.bbox.y1 - first.bbox.y0).max(0.01);
    let second_height = (second.bbox.y1 - second.bbox.y0).max(0.01);
    let line_gap = second.bbox.y0 - first.bbox.y1;
    if line_gap < -first_height.max(second_height) * 0.25
        || line_gap > first_height.max(second_height) * 1.3
    {
        return false;
    }

    let first_width = first.bbox.x1 - first.bbox.x0;
    let second_width = second.bbox.x1 - second.bbox.x0;
    if first_width < 0.18 || second_width < 0.18 {
        return false;
    }

    let first_x_mid = (first.bbox.x0 + first.bbox.x1) / 2.0;
    let second_x_mid = (second.bbox.x0 + second.bbox.x1) / 2.0;
    let centered_together = (first_x_mid - 0.5).abs() <= 0.16 && (second_x_mid - 0.5).abs() <= 0.16;
    let aligned_left = (first.bbox.x0 - second.bbox.x0).abs() <= 0.08;
    let aligned_right = (first.bbox.x1 - second.bbox.x1).abs() <= 0.08;
    if !(centered_together || aligned_left || aligned_right) {
        return false;
    }

    match (first.font_size, second.font_size) {
        (Some(first_size), Some(second_size)) if (first_size - second_size).abs() > 2.0 => {
            return false;
        }
        _ => {}
    }

    true
}

fn average_optional_values(values: impl Iterator<Item = f32>) -> Option<f32> {
    let mut sum = 0.0;
    let mut count = 0;
    for value in values {
        sum += value;
        count += 1;
    }

    (count > 0).then_some(sum / count as f32)
}

fn merge_optional_bold(values: impl Iterator<Item = Option<bool>>) -> Option<bool> {
    let collected = values.collect::<Vec<_>>();
    if collected.iter().all(|value| *value == Some(true)) {
        Some(true)
    } else if collected.iter().all(|value| *value == Some(false)) {
        Some(false)
    } else {
        None
    }
}

fn score_layout_block(
    block: &LayoutBlock,
    page_index: usize,
    extract_method: &ExtractMethod,
    profile: &ScoringProfile,
) -> Option<CandidateTitle> {
    let text = clean_candidate_text(&block.text);
    if text.is_empty() || is_symbol_only_noise(&text) {
        return None;
    }
    if text.chars().count() > profile.max_title_chars as usize {
        return None;
    }

    let source = match extract_method {
        ExtractMethod::ImageOcrTesseract | ExtractMethod::PdfOcrFallbackTesseract => {
            CandidateSource::OcrBlock
        }
        _ => CandidateSource::PdfLayout,
    };
    let mut context = ScoreContext::new(text.clone(), source.clone());

    apply_text_quality_rules(&mut context);
    apply_layout_rules(&mut context, block, profile);
    apply_position_rules(&mut context, block, page_index, profile);
    apply_line_shape_rules(&mut context, block, page_index);
    apply_keyword_rules(&mut context, profile);

    if matches!(source, CandidateSource::OcrBlock) {
        let penalty = (8.0 * profile.ocr_conservatism).round() as i16;
        context.add_rule("ocr-conservatism", "penalty", -penalty, "OCR 候选保守降权");
    }

    Some(context.into_candidate(Some(page_index), None))
}

fn score_paragraph(paragraph: &ParagraphBlock, profile: &ScoringProfile) -> Option<CandidateTitle> {
    let text = clean_candidate_text(&paragraph.text);
    if text.is_empty() || is_symbol_only_noise(&text) {
        return None;
    }
    if text.chars().count() > profile.max_title_chars as usize {
        return None;
    }

    let mut context = ScoreContext::new(text, CandidateSource::WordParagraph);
    apply_text_quality_rules(&mut context);
    apply_word_position_rules(&mut context, paragraph);
    apply_keyword_rules(&mut context, profile);
    context.add_rule(
        "word-conservatism",
        "penalty",
        -10,
        "Word 纯文本候选保守降权",
    );

    Some(context.into_candidate(None, Some(paragraph.paragraph_index)))
}

#[derive(Debug)]
struct ScoreContext {
    text: String,
    source: CandidateSource,
    category_scores: CategoryScores,
    rule_details: Vec<RuleDetail>,
}

impl ScoreContext {
    fn new(text: String, source: CandidateSource) -> Self {
        Self {
            text,
            source,
            category_scores: CategoryScores {
                layout: 0,
                position: 0,
                keyword: 0,
                text_quality: 0,
                penalty: 0,
            },
            rule_details: vec![],
        }
    }

    fn add_rule(
        &mut self,
        rule_name: impl Into<String>,
        category: impl Into<String>,
        delta: i16,
        description: impl Into<String>,
    ) {
        let category = category.into();
        match category.as_str() {
            "layout" => self.category_scores.layout += delta,
            "position" => self.category_scores.position += delta,
            "keyword" => self.category_scores.keyword += delta,
            "textQuality" => self.category_scores.text_quality += delta,
            "penalty" => self.category_scores.penalty += delta,
            _ => {}
        }
        self.rule_details.push(RuleDetail {
            rule_name: rule_name.into(),
            category,
            delta,
            description: description.into(),
        });
    }

    fn into_candidate(
        self,
        page_index: Option<usize>,
        paragraph_index: Option<usize>,
    ) -> CandidateTitle {
        let total = 30
            + self.category_scores.layout
            + self.category_scores.position
            + self.category_scores.keyword
            + self.category_scores.text_quality
            + self.category_scores.penalty;
        let score = total.clamp(0, 100) as u8;

        CandidateTitle {
            text: self.text,
            source: self.source,
            page_index,
            paragraph_index,
            score,
            category_scores: self.category_scores,
            rule_details: self.rule_details,
        }
    }
}

fn clean_candidate_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_symbol_only_noise(text: &str) -> bool {
    let visible_chars = text.chars().filter(|ch| !ch.is_whitespace()).count();
    if visible_chars == 0 {
        return true;
    }

    let alphabetic_chars = text.chars().filter(|ch| ch.is_alphabetic()).count();
    let numeric_chars = text.chars().filter(|ch| ch.is_numeric()).count();
    if alphabetic_chars > 0 || numeric_chars > 1 {
        return false;
    }

    let symbol_chars = text
        .chars()
        .filter(|ch| {
            matches!(
                *ch,
                '@' | '+'
                    | '-'
                    | '_'
                    | '='
                    | '*'
                    | '#'
                    | '□'
                    | '■'
                    | '▪'
                    | '▬'
                    | '▁'
                    | '·'
                    | '•'
                    | '\u{fffd}'
            ) || ch.is_ascii_punctuation()
        })
        .count();

    symbol_chars == visible_chars
}

fn apply_text_quality_rules(context: &mut ScoreContext) {
    let char_count = context.text.chars().count();
    if (2..=40).contains(&char_count) {
        context.add_rule(
            "text-length-title-like",
            "textQuality",
            18,
            "标题长度在偏好范围内",
        );
    } else if char_count > 60 {
        context.add_rule("text-too-long", "penalty", -35, "候选文本过长");
    } else {
        context.add_rule("text-length-weak", "textQuality", 4, "候选文本长度可用");
    }

    let punctuation_count = context
        .text
        .chars()
        .filter(|ch| is_punctuation(*ch))
        .count();
    if punctuation_count >= 3 {
        context.add_rule("punctuation-dense", "penalty", -20, "标点密度过高");
    }

    if looks_like_noise(&context.text) {
        context.add_rule(
            "noise-exclusion",
            "penalty",
            -60,
            "疑似页码、日期、密级或落款",
        );
    }

    if looks_like_spaced_ocr_fragment(&context.text) {
        context.add_rule(
            "spaced-ocr-fragment",
            "penalty",
            -35,
            "疑似 OCR 拆散的短词噪声",
        );
    }

    if looks_like_letterhead_or_reference_number(&context.text) {
        context.add_rule(
            "letterhead-reference-noise",
            "penalty",
            -34,
            "疑似红头机关名或发文字号",
        );
    }

    if looks_like_sentence(&context.text) {
        context.add_rule("sentence-like", "penalty", -18, "疑似正文长句");
    }
}

fn apply_layout_rules(context: &mut ScoreContext, block: &LayoutBlock, profile: &ScoringProfile) {
    if let Some(font_size) = block.font_size {
        if font_size >= 20.0 {
            context.add_rule(
                "layout-font-large",
                "layout",
                scaled(22, profile.layout_sensitivity),
                "字号明显偏大",
            );
        } else if font_size >= 16.0 {
            context.add_rule(
                "layout-font-medium",
                "layout",
                scaled(10, profile.layout_sensitivity),
                "字号略大",
            );
        }
    } else {
        let height = block.bbox.y1 - block.bbox.y0;
        if height >= 0.045 {
            context.add_rule(
                "layout-visual-height",
                "layout",
                scaled(14, profile.layout_sensitivity),
                "视觉高度接近标题",
            );
        }
    }

    if block.bold == Some(true) {
        context.add_rule(
            "layout-bold",
            "layout",
            scaled(10, profile.layout_sensitivity),
            "文本加粗",
        );
    }
}

fn apply_position_rules(
    context: &mut ScoreContext,
    block: &LayoutBlock,
    page_index: usize,
    profile: &ScoringProfile,
) {
    let y_mid = (block.bbox.y0 + block.bbox.y1) / 2.0;
    if y_mid <= 0.25 {
        context.add_rule(
            "position-upper-page",
            "position",
            scaled(16, profile.position_sensitivity),
            "位于页面上部",
        );
    } else if y_mid <= 0.40 {
        context.add_rule(
            "position-title-band",
            "position",
            scaled(7, profile.position_sensitivity),
            "位于合理标题区域",
        );
    }

    let x_mid = (block.bbox.x0 + block.bbox.x1) / 2.0;
    let centered = (x_mid - 0.5).abs() <= 0.12;
    if centered {
        context.add_rule(
            "position-centered",
            "position",
            scaled(12, profile.position_sensitivity),
            "接近水平中轴",
        );
    }

    if page_index == 0 {
        if centered && (0.40..=0.62).contains(&y_mid) {
            context.add_rule(
                "position-first-page-centered-title-band",
                "position",
                16,
                "位于第一页居中标题带",
            );
        }
    } else {
        context.add_rule(
            "position-later-page",
            "penalty",
            -14,
            "后续页面候选保守降权",
        );
    }
}

fn apply_line_shape_rules(context: &mut ScoreContext, block: &LayoutBlock, page_index: usize) {
    let width = block.bbox.x1 - block.bbox.x0;
    let char_count = context.text.chars().count();
    if block.line_index.is_some() && width >= 0.60 && char_count >= 24 {
        context.add_rule("wide-body-line", "penalty", -20, "疑似正文整行");
    }

    if page_index == 0 && looks_like_promulgation_sentence(&context.text) {
        context.add_rule(
            "promulgation-sentence",
            "penalty",
            -22,
            "疑似法规公布说明句",
        );
    }
}

fn apply_word_position_rules(context: &mut ScoreContext, paragraph: &ParagraphBlock) {
    if paragraph.paragraph_index <= 3 {
        context.add_rule("word-early-paragraph", "position", 16, "段落位置靠前");
    } else if paragraph.paragraph_index <= 10 {
        context.add_rule("word-first-ten", "position", 7, "位于前 10 个非空段落内");
    }
}

fn apply_keyword_rules(context: &mut ScoreContext, profile: &ScoringProfile) {
    let default_profile = ScoringProfile::default();
    for rule in &default_profile.keyword_rules {
        if context.text.contains(&rule.keyword) {
            context.add_rule(
                "keyword-default",
                "keyword",
                scaled(rule.score_delta, profile.keyword_sensitivity),
                format!("命中默认关键词：{}", rule.keyword),
            );
        }
    }

    for rule in &profile.keyword_rules {
        if default_profile
            .keyword_rules
            .iter()
            .any(|default_rule| default_rule.keyword == rule.keyword)
        {
            continue;
        }

        if context.text.contains(&rule.keyword) {
            context.add_rule(
                "keyword-custom",
                "keyword",
                scaled(rule.score_delta, profile.keyword_sensitivity),
                format!("命中用户关键词：{}", rule.keyword),
            );
        }
    }

    for rule in &profile.regex_rules {
        if let Ok(regex) = Regex::new(&rule.pattern) {
            if regex.is_match(&context.text) {
                context.add_rule(
                    "regex-custom",
                    "keyword",
                    scaled(rule.score_delta, profile.keyword_sensitivity),
                    format!("命中用户正则：{}", rule.pattern),
                );
            }
        }
    }

    if looks_like_document_title(&context.text) {
        context.add_rule(
            "title-keyword-default",
            "keyword",
            10,
            "命中文档标题类关键词",
        );
    }

    if looks_like_notice_title_topic(&context.text) {
        context.add_rule("notice-title-topic", "keyword", 16, "疑似通知标题主题");
    }
}

fn scaled(delta: i16, sensitivity: f32) -> i16 {
    (delta as f32 * sensitivity).round() as i16
}

fn looks_like_sentence(text: &str) -> bool {
    text.chars().count() > 32
        && ["，", "。", "；", ",", ".", ";", "已经", "现予", "施行"]
            .iter()
            .any(|mark| text.contains(mark))
}

fn looks_like_promulgation_sentence(text: &str) -> bool {
    (text.contains("已经") || text.contains("现予") || text.contains("施行"))
        && (text.contains("国务院") || text.contains("会议") || text.contains("公布"))
}

fn looks_like_noise(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }

    let date_like = Regex::new(r"^\d{4}年\d{1,2}月\d{1,2}日$").unwrap();
    let page_like = Regex::new(r"^第\s*\d+\s*页$|^\d+\s*/\s*\d+$|^-\s*\d+\s*-$").unwrap();
    let numbered_like = Regex::new(r"^[一二三四五六七八九十]+[、.．]").unwrap();
    let digits_only = Regex::new(r"^\d+$").unwrap();
    let document_number_like = Regex::new(r"^第\s*\d+\s*号$").unwrap();

    date_like.is_match(trimmed)
        || page_like.is_match(trimmed)
        || numbered_like.is_match(trimmed)
        || digits_only.is_match(trimmed)
        || document_number_like.is_match(trimmed)
        || trimmed.ends_with("国务院令")
        || trimmed.contains("机密")
        || trimmed.contains("秘密")
        || trimmed.ends_with("公司")
        || trimmed.ends_with("办公室")
}

fn looks_like_spaced_ocr_fragment(text: &str) -> bool {
    let trimmed = text.trim();
    let chars = trimmed.chars().collect::<Vec<_>>();
    let non_space_count = chars.iter().filter(|ch| !ch.is_whitespace()).count();
    let space_count = chars.iter().filter(|ch| ch.is_whitespace()).count();

    non_space_count <= 4 && space_count >= 2
}

fn looks_like_letterhead_or_reference_number(text: &str) -> bool {
    let trimmed = text.trim();
    let compact = trimmed.split_whitespace().collect::<String>();
    let reference_number_like =
        Regex::new(r"^[\p{Han}]{1,8}\s*[\[〔（(]?\s*\d{4}\s*[\]〕）)]?\s*\d+\s*号?$").unwrap();

    compact.ends_with("办公厅")
        || compact.ends_with("办公室")
        || compact.ends_with("生态环境部")
        || compact.ends_with("生态环境厅")
        || compact.ends_with("生态环境局")
        || compact.ends_with("国务院")
        || compact.contains("生态环境部办公")
        || compact.contains("生态环境厅办公")
        || compact.contains("生态环境局办公")
        || reference_number_like.is_match(trimmed)
}

fn looks_like_document_title(text: &str) -> bool {
    let trimmed = text.trim();
    [
        "条例", "办法", "规定", "决定", "公告", "通告", "意见", "细则", "章程", "规划", "计划",
        "总结", "纪要",
    ]
    .iter()
    .any(|keyword| trimmed.contains(keyword))
}

fn looks_like_notice_title_topic(text: &str) -> bool {
    let trimmed = text.trim();
    let contains_notice_word = trimmed.contains("通知");
    let contains_topic_word = [
        "关于",
        "推进",
        "统筹",
        "工作",
        "有关事项",
        "监管",
        "保护",
        "生态",
    ]
    .iter()
    .any(|keyword| trimmed.contains(keyword));

    contains_notice_word && contains_topic_word
}

fn looks_like_word_title_continuation(text: &str) -> bool {
    let compact = text.split_whitespace().collect::<String>();
    let char_count = compact.chars().count();
    if !(4..=32).contains(&char_count) || looks_like_word_body_continuation(&compact) {
        return false;
    }

    let title_keywords = [
        "通知", "报告", "方案", "制度", "合同", "函", "请示", "意见", "办法", "规定", "条例",
        "决定", "公告", "通告", "结果", "情况", "事项", "工作", "说明", "申请", "名单",
    ];
    if title_keywords
        .iter()
        .any(|keyword| compact.contains(keyword))
    {
        return true;
    }

    let period_like =
        Regex::new(r"^(\d{4}年)?第?[一二三四五六七八九十\d]+次[（(]?[一二三四五六七八九十\d]+[—\-至到][一二三四五六七八九十\d]+月[）)]?$")
            .unwrap();
    period_like.is_match(&compact)
}

fn looks_like_word_body_continuation(text: &str) -> bool {
    let body_starts = [
        "正文",
        "各单位",
        "各部门",
        "根据",
        "按照",
        "为进一步",
        "现将",
        "请各",
        "附件",
        "联系人",
    ];

    body_starts.iter().any(|prefix| text.starts_with(prefix))
        || text.contains("正文段落")
        || text.contains("段落内容")
        || looks_like_sentence(text)
        || Regex::new(r"^[一二三四五六七八九十]+[、.．]")
            .unwrap()
            .is_match(text)
}

fn is_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '，' | '。' | '、' | '；' | '：' | '！' | '？' | ',' | '.' | ';' | ':' | '!' | '?'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        CandidateSource, ExtractMethod, ExtractedDocument, ExtractedPage, FileType, KeywordRule,
        LayoutBlock, NormalizedBox, ParagraphBlock, RawBox, RegexRule, ScoringProfile, SourceUnit,
    };

    #[test]
    fn scores_pdf_geometry_title_as_auto_output() {
        let result = score_document(
            pdf_document(vec![
                block("第 1 页", 0.46, 0.03, 0.54, 0.05, Some(9.0), Some(false)),
                block(
                    "关于召开年度工作会议的通知",
                    0.24,
                    0.12,
                    0.76,
                    0.18,
                    Some(24.0),
                    Some(true),
                ),
                block(
                    "各部门请按时参会，会议材料另行通知。",
                    0.12,
                    0.38,
                    0.88,
                    0.42,
                    Some(11.0),
                    Some(false),
                ),
            ]),
            ScoringProfile::default(),
        );

        assert_eq!(
            result.final_title.as_deref(),
            Some("关于召开年度工作会议的通知")
        );
        assert_eq!(result.decision, crate::models::ScoreDecision::AutoOutput);
        assert!(result.confidence >= 70);
        assert_eq!(result.candidates[0].source, CandidateSource::PdfLayout);
        assert!(result.candidates[0].category_scores.layout > 0);
        assert!(result.candidates[0].category_scores.position > 0);
        assert!(result.candidates[0]
            .rule_details
            .iter()
            .any(|rule| rule.rule_name == "layout-bold"));
    }

    #[test]
    fn combines_adjacent_pdf_title_lines_into_one_candidate() {
        let result = score_document(
            pdf_document(vec![
                block(
                    "关于统筹推进自然保护地和生态保护红线",
                    0.18,
                    0.12,
                    0.82,
                    0.16,
                    Some(16.0),
                    Some(false),
                ),
                block(
                    "生态环境监管工作的通知",
                    0.30,
                    0.17,
                    0.70,
                    0.21,
                    Some(16.0),
                    Some(false),
                ),
                block(
                    "各单位要结合实际认真组织实施。",
                    0.12,
                    0.34,
                    0.88,
                    0.38,
                    Some(11.0),
                    Some(false),
                ),
            ]),
            ScoringProfile::default(),
        );

        assert_eq!(
            result.final_title.as_deref(),
            Some("关于统筹推进自然保护地和生态保护红线生态环境监管工作的通知")
        );
        assert!(result.candidates[0]
            .rule_details
            .iter()
            .any(|rule| rule.rule_name == "layout-two-line-title"));
    }

    #[test]
    fn combines_adjacent_pdf_title_three_lines_into_one_candidate() {
        let result = score_document(
            pdf_document(vec![
                block(
                    "关于开展自然保护地生态环境监管",
                    0.22,
                    0.12,
                    0.78,
                    0.16,
                    Some(16.0),
                    Some(false),
                ),
                block(
                    "遥感监测线索核查整改工作",
                    0.25,
                    0.17,
                    0.75,
                    0.21,
                    Some(16.0),
                    Some(false),
                ),
                block(
                    "有关事项并持续推进规范化建设的通知",
                    0.19,
                    0.22,
                    0.81,
                    0.26,
                    Some(16.0),
                    Some(false),
                ),
                block(
                    "各单位要结合实际认真组织实施。",
                    0.12,
                    0.38,
                    0.88,
                    0.42,
                    Some(11.0),
                    Some(false),
                ),
            ]),
            ScoringProfile::default(),
        );

        assert_eq!(
            result.final_title.as_deref(),
            Some("关于开展自然保护地生态环境监管遥感监测线索核查整改工作有关事项并持续推进规范化建设的通知")
        );
        assert!(result.candidates[0]
            .rule_details
            .iter()
            .any(|rule| rule.rule_name == "layout-three-line-title"));
    }

    #[test]
    fn filters_title_candidates_over_default_max_title_chars() {
        let too_long_title =
            "关于开展自然保护地生态环境监管遥感监测线索核查整改工作有关事项并持续推进规范化建设相关工作的通知";

        let result = score_document(
            pdf_document(vec![block(
                too_long_title,
                0.14,
                0.12,
                0.86,
                0.17,
                Some(22.0),
                Some(true),
            )]),
            ScoringProfile::default(),
        );

        assert!(too_long_title.chars().count() > 45);
        assert!(result
            .candidates
            .iter()
            .all(|candidate| candidate.text != too_long_title));
        assert!(result.final_title.is_none());
    }

    #[test]
    fn scores_image_ocr_title_but_applies_ocr_conservatism() {
        let profile = ScoringProfile {
            ocr_conservatism: 1.5,
            ..ScoringProfile::default()
        };

        let result = score_document(
            ExtractedDocument {
                source_type: FileType::Png,
                extract_method: ExtractMethod::ImageOcrTesseract,
                pages: vec![ExtractedPage {
                    page_index: 0,
                    width: 1200.0,
                    height: 1600.0,
                    unit: SourceUnit::Pixel,
                    blocks: vec![LayoutBlock {
                        text: "项目实施方案".into(),
                        bbox: NormalizedBox {
                            x0: 0.32,
                            y0: 0.10,
                            x1: 0.68,
                            y1: 0.16,
                        },
                        raw_bbox: None,
                        font_size: None,
                        bold: None,
                        ocr_confidence: Some(0.94),
                        line_index: Some(0),
                    }],
                }],
                paragraphs: vec![],
                diagnostics_ref: None,
            },
            profile,
        );

        assert_eq!(result.final_title.as_deref(), Some("项目实施方案"));
        assert_eq!(result.candidates[0].source, CandidateSource::OcrBlock);
        assert!(result.candidates[0]
            .rule_details
            .iter()
            .any(|rule| rule.rule_name == "ocr-conservatism"));
    }

    #[test]
    fn combines_adjacent_ocr_title_lines_into_one_candidate() {
        let result = score_document(
            ExtractedDocument {
                source_type: FileType::Pdf,
                extract_method: ExtractMethod::PdfOcrFallbackTesseract,
                pages: vec![ExtractedPage {
                    page_index: 0,
                    width: 1200.0,
                    height: 1600.0,
                    unit: SourceUnit::Pixel,
                    blocks: vec![
                        ocr_block(
                            "关于统筹推进自然保护地和生态保护红线",
                            0.18,
                            0.32,
                            0.82,
                            0.36,
                            0.88,
                        ),
                        ocr_block("生态环境监管有关事项的通知", 0.27, 0.37, 0.73, 0.41, 0.88),
                    ],
                }],
                paragraphs: vec![],
                diagnostics_ref: None,
            },
            ScoringProfile::default(),
        );

        assert_eq!(
            result.final_title.as_deref(),
            Some("关于统筹推进自然保护地和生态保护红线生态环境监管有关事项的通知")
        );
        assert_eq!(result.candidates[0].source, CandidateSource::OcrBlock);
        assert!(result.candidates[0]
            .rule_details
            .iter()
            .any(|rule| rule.rule_name == "layout-two-line-title"));
    }

    #[test]
    fn rejects_isolated_numeric_ocr_line_as_title() {
        let result = score_document(
            ExtractedDocument {
                source_type: FileType::Pdf,
                extract_method: ExtractMethod::PdfOcrFallbackTesseract,
                pages: vec![ExtractedPage {
                    page_index: 0,
                    width: 1200.0,
                    height: 1600.0,
                    unit: SourceUnit::Pixel,
                    blocks: vec![
                        ocr_block("830", 0.48, 0.18, 0.52, 0.21, 0.90),
                        ocr_block("中华人民共和国自然保护区条例", 0.25, 0.50, 0.78, 0.55, 0.88),
                    ],
                }],
                paragraphs: vec![],
                diagnostics_ref: None,
            },
            ScoringProfile::default(),
        );

        assert_eq!(
            result.final_title.as_deref(),
            Some("中华人民共和国自然保护区条例")
        );
        assert_ne!(result.candidates[0].text, "830");
        assert!(result
            .candidates
            .iter()
            .find(|candidate| candidate.text == "830")
            .is_none_or(|candidate| candidate.score < 45));
    }

    #[test]
    fn government_order_header_does_not_beat_regulation_title() {
        let result = score_document(
            ExtractedDocument {
                source_type: FileType::Pdf,
                extract_method: ExtractMethod::PdfOcrFallbackTesseract,
                pages: vec![ExtractedPage {
                    page_index: 0,
                    width: 1200.0,
                    height: 1600.0,
                    unit: SourceUnit::Pixel,
                    blocks: vec![
                        ocr_block("中华人民共和国国务院令", 0.30, 0.18, 0.70, 0.23, 0.90),
                        ocr_block("第830号", 0.46, 0.24, 0.54, 0.27, 0.90),
                        ocr_block("中华人民共和国自然保护区条例", 0.25, 0.50, 0.78, 0.55, 0.88),
                    ],
                }],
                paragraphs: vec![],
                diagnostics_ref: None,
            },
            ScoringProfile::default(),
        );

        assert_eq!(
            result.final_title.as_deref(),
            Some("中华人民共和国自然保护区条例")
        );
        assert_eq!(result.candidates[0].text, "中华人民共和国自然保护区条例");
    }

    #[test]
    fn first_page_centered_title_beats_body_lines_on_later_pages() {
        let result = score_document(
            ExtractedDocument {
                source_type: FileType::Pdf,
                extract_method: ExtractMethod::PdfOcrFallbackTesseract,
                pages: vec![
                    ExtractedPage {
                        page_index: 0,
                        width: 1200.0,
                        height: 1600.0,
                        unit: SourceUnit::Pixel,
                        blocks: vec![
                            ocr_block(
                                "《中华人民共和国自然保护区条例》已经2026年1月9日国务院",
                                0.207,
                                0.278,
                                0.855,
                                0.293,
                                0.93,
                            ),
                            ocr_block(
                                "中华人民共和国自然保护区条例",
                                0.260,
                                0.482,
                                0.742,
                                0.505,
                                0.93,
                            ),
                        ],
                    },
                    ExtractedPage {
                        page_index: 2,
                        width: 1200.0,
                        height: 1600.0,
                        unit: SourceUnit::Pixel,
                        blocks: vec![ocr_block(
                            "按照规定设立的各自然保护区管理机构依照本条例和规定的职",
                            0.195,
                            0.181,
                            0.853,
                            0.197,
                            0.94,
                        )],
                    },
                ],
                paragraphs: vec![],
                diagnostics_ref: None,
            },
            ScoringProfile::default(),
        );

        assert_eq!(
            result.final_title.as_deref(),
            Some("中华人民共和国自然保护区条例")
        );
        assert_eq!(result.candidates[0].text, "中华人民共和国自然保护区条例");
    }

    #[test]
    fn ministry_red_header_and_reference_number_do_not_beat_notice_title_topic() {
        let result = score_document(
            ExtractedDocument {
                source_type: FileType::Pdf,
                extract_method: ExtractMethod::PdfOcrFallbackTesseract,
                pages: vec![ExtractedPage {
                    page_index: 0,
                    width: 1200.0,
                    height: 1600.0,
                    unit: SourceUnit::Pixel,
                    blocks: vec![
                        ocr_block("人 自 人", 0.39, 0.10, 0.61, 0.14, 0.73),
                        ocr_block("民共和国生态环境部办公打", 0.24, 0.15, 0.76, 0.19, 0.68),
                        ocr_block("环办生态函 [2024] 311", 0.56, 0.19, 0.86, 0.22, 0.59),
                        ocr_block(
                            "进自然保护地和生态保护红线生态环境监管有关事项通知如下",
                            0.18,
                            0.33,
                            0.84,
                            0.37,
                            0.88,
                        ),
                    ],
                }],
                paragraphs: vec![],
                diagnostics_ref: None,
            },
            ScoringProfile::default(),
        );

        assert_eq!(
            result.final_title.as_deref(),
            Some("进自然保护地和生态保护红线生态环境监管有关事项通知如下")
        );
        assert_eq!(
            result.candidates[0].text,
            "进自然保护地和生态保护红线生态环境监管有关事项通知如下"
        );
    }

    #[test]
    fn scores_word_from_first_ten_non_empty_paragraphs_with_conservative_penalty() {
        let mut paragraphs = vec![ParagraphBlock {
            text: " ".into(),
            paragraph_index: 0,
        }];
        paragraphs.extend((1..12).map(|index| ParagraphBlock {
            text: if index == 3 {
                "劳动合同".into()
            } else if index == 11 {
                "管理制度".into()
            } else {
                format!("正文段落内容 {index}")
            },
            paragraph_index: index,
        }));

        let result = score_document(
            ExtractedDocument {
                source_type: FileType::Docx,
                extract_method: ExtractMethod::WordUndoc,
                pages: vec![],
                paragraphs,
                diagnostics_ref: None,
            },
            ScoringProfile::default(),
        );

        assert_eq!(result.final_title.as_deref(), Some("劳动合同"));
        assert!(result
            .candidates
            .iter()
            .all(|candidate| candidate.paragraph_index != Some(11)));
        assert!(result.candidates[0]
            .rule_details
            .iter()
            .any(|rule| rule.rule_name == "word-conservatism"));
    }

    #[test]
    fn combines_adjacent_word_paragraphs_into_one_title_candidate() {
        let result = score_document(
            ExtractedDocument {
                source_type: FileType::Doc,
                extract_method: ExtractMethod::DocConvertedUndoc,
                pages: vec![],
                paragraphs: vec![
                    ParagraphBlock {
                        text: "生态处关于报送2026年第一次（1—2月）".into(),
                        paragraph_index: 0,
                    },
                    ParagraphBlock {
                        text: "双月遥感监测线索核查结果的请示".into(),
                        paragraph_index: 1,
                    },
                    ParagraphBlock {
                        text: "自然保护地内违法违规点位情况".into(),
                        paragraph_index: 2,
                    },
                ],
                diagnostics_ref: None,
            },
            ScoringProfile::default(),
        );

        assert_eq!(
            result.final_title.as_deref(),
            Some("生态处关于报送2026年第一次（1—2月）双月遥感监测线索核查结果的请示")
        );
        assert_eq!(
            result.candidates[0].text,
            "生态处关于报送2026年第一次（1—2月）双月遥感监测线索核查结果的请示"
        );
        assert_eq!(result.candidates[0].source, CandidateSource::WordParagraph);
        assert_eq!(result.candidates[0].paragraph_index, Some(0));
        assert!(result.candidates[0]
            .rule_details
            .iter()
            .any(|rule| rule.rule_name == "word-two-paragraph-title"));
    }

    #[test]
    fn filters_empty_text_and_penalizes_noise() {
        let result = score_document(
            pdf_document(vec![
                block("   ", 0.2, 0.1, 0.8, 0.15, Some(24.0), Some(true)),
                block(
                    "2026年6月26日",
                    0.40,
                    0.12,
                    0.60,
                    0.15,
                    Some(18.0),
                    Some(false),
                ),
                block("机密★一年", 0.45, 0.16, 0.55, 0.19, Some(18.0), Some(true)),
            ]),
            ScoringProfile::default(),
        );

        assert!(result.final_title.is_none());
        assert_eq!(result.decision, crate::models::ScoreDecision::Failed);
        assert!(result
            .candidates
            .iter()
            .all(|candidate| candidate.score < 70));
    }

    #[test]
    fn filters_symbol_only_layout_blocks() {
        let result = score_document(
            pdf_document(vec![
                block("@", 0.48, 0.10, 0.52, 0.14, Some(22.0), Some(false)),
                block("□", 0.48, 0.16, 0.52, 0.20, Some(18.0), Some(false)),
                block("+", 0.48, 0.22, 0.52, 0.26, Some(18.0), Some(false)),
                block("▬", 0.48, 0.28, 0.52, 0.32, Some(18.0), Some(false)),
                block("·", 0.48, 0.34, 0.52, 0.38, Some(18.0), Some(false)),
            ]),
            ScoringProfile::default(),
        );

        assert!(result.candidates.is_empty());
        assert!(result.final_title.is_none());
        assert_eq!(result.decision, crate::models::ScoreDecision::Failed);
    }

    #[test]
    fn applies_custom_keyword_and_regex_rules() {
        let mut profile = ScoringProfile::default();
        profile.keyword_rules.push(KeywordRule {
            keyword: "验收".into(),
            score_delta: 12,
        });
        profile.regex_rules.push(RegexRule {
            pattern: "^项目.*报告$".into(),
            score_delta: 9,
        });

        let result = score_document(
            pdf_document(vec![block(
                "项目验收报告",
                0.30,
                0.12,
                0.70,
                0.17,
                Some(22.0),
                Some(true),
            )]),
            profile,
        );

        let rule_names: Vec<_> = result.candidates[0]
            .rule_details
            .iter()
            .map(|rule| rule.rule_name.as_str())
            .collect();
        assert!(rule_names.contains(&"keyword-custom"));
        assert!(rule_names.contains(&"regex-custom"));
    }

    #[test]
    fn low_confidence_candidate_stays_pending() {
        let profile = ScoringProfile {
            auto_output_threshold: 95,
            ..ScoringProfile::default()
        };

        let result = score_document(
            pdf_document(vec![block(
                "情况报告",
                0.30,
                0.20,
                0.70,
                0.24,
                Some(16.0),
                Some(false),
            )]),
            profile,
        );

        assert_eq!(result.decision, crate::models::ScoreDecision::Pending);
        assert!(result.confidence < 95);
        assert!(result.final_title.is_some());
    }

    fn pdf_document(blocks: Vec<LayoutBlock>) -> ExtractedDocument {
        ExtractedDocument {
            source_type: FileType::Pdf,
            extract_method: ExtractMethod::PdfNativeLiteparse,
            pages: vec![ExtractedPage {
                page_index: 0,
                width: 595.0,
                height: 842.0,
                unit: SourceUnit::PdfPoint,
                blocks,
            }],
            paragraphs: vec![],
            diagnostics_ref: None,
        }
    }

    fn block(
        text: &str,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        font_size: Option<f32>,
        bold: Option<bool>,
    ) -> LayoutBlock {
        LayoutBlock {
            text: text.into(),
            bbox: NormalizedBox { x0, y0, x1, y1 },
            raw_bbox: Some(RawBox {
                x0: x0 * 595.0,
                y0: y0 * 842.0,
                x1: x1 * 595.0,
                y1: y1 * 842.0,
            }),
            font_size,
            bold,
            ocr_confidence: None,
            line_index: Some(0),
        }
    }

    fn ocr_block(text: &str, x0: f32, y0: f32, x1: f32, y1: f32, confidence: f32) -> LayoutBlock {
        LayoutBlock {
            text: text.into(),
            bbox: NormalizedBox { x0, y0, x1, y1 },
            raw_bbox: Some(RawBox {
                x0: x0 * 1200.0,
                y0: y0 * 1600.0,
                x1: x1 * 1200.0,
                y1: y1 * 1600.0,
            }),
            font_size: None,
            bold: None,
            ocr_confidence: Some(confidence),
            line_index: Some(0),
        }
    }
}
