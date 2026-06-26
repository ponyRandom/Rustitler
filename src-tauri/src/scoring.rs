use crate::models::{
    CandidateSource, CandidateTitle, CategoryScores, ExtractMethod, ExtractedDocument, LayoutBlock,
    ParagraphBlock, RuleDetail, ScoreDecision, ScoringProfile, ScoringResult,
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
        return extracted
            .pages
            .iter()
            .flat_map(|page| {
                page.blocks.iter().filter_map(|block| {
                    score_layout_block(block, page.page_index, &extracted.extract_method, profile)
                })
            })
            .collect();
    }

    extracted
        .paragraphs
        .iter()
        .filter(|paragraph| !clean_candidate_text(&paragraph.text).is_empty())
        .take(10)
        .filter_map(|paragraph| score_paragraph(paragraph, profile))
        .collect()
}

fn score_layout_block(
    block: &LayoutBlock,
    page_index: usize,
    extract_method: &ExtractMethod,
    profile: &ScoringProfile,
) -> Option<CandidateTitle> {
    let text = clean_candidate_text(&block.text);
    if text.is_empty() {
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
    apply_position_rules(&mut context, block, profile);
    apply_keyword_rules(&mut context, profile);

    if matches!(source, CandidateSource::OcrBlock) {
        let penalty = (8.0 * profile.ocr_conservatism).round() as i16;
        context.add_rule("ocr-conservatism", "penalty", -penalty, "OCR 候选保守降权");
    }

    Some(context.into_candidate(Some(page_index), None))
}

fn score_paragraph(paragraph: &ParagraphBlock, profile: &ScoringProfile) -> Option<CandidateTitle> {
    let text = clean_candidate_text(&paragraph.text);
    if text.is_empty() {
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

fn apply_position_rules(context: &mut ScoreContext, block: &LayoutBlock, profile: &ScoringProfile) {
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
    if (x_mid - 0.5).abs() <= 0.12 {
        context.add_rule(
            "position-centered",
            "position",
            scaled(12, profile.position_sensitivity),
            "接近水平中轴",
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
}

fn scaled(delta: i16, sensitivity: f32) -> i16 {
    (delta as f32 * sensitivity).round() as i16
}

fn looks_like_sentence(text: &str) -> bool {
    text.chars().count() > 32
        && ["，", "。", "；", ",", ".", ";"]
            .iter()
            .any(|mark| text.contains(mark))
}

fn looks_like_noise(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }

    let date_like = Regex::new(r"^\d{4}年\d{1,2}月\d{1,2}日$").unwrap();
    let page_like = Regex::new(r"^第\s*\d+\s*页$|^\d+\s*/\s*\d+$|^-\s*\d+\s*-$").unwrap();
    let numbered_like = Regex::new(r"^[一二三四五六七八九十]+[、.．]").unwrap();

    date_like.is_match(trimmed)
        || page_like.is_match(trimmed)
        || numbered_like.is_match(trimmed)
        || trimmed.contains("机密")
        || trimmed.contains("秘密")
        || trimmed.ends_with("公司")
        || trimmed.ends_with("办公室")
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
}
