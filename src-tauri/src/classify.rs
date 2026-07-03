use crate::errors::{AppError, ErrorCategory, ErrorCode};
use crate::models::{
    CategoryCount, ClassificationFailure, ClassificationSettings, ClassificationSummary,
};
use crate::settings::normalize_classification_settings_for_runtime;
use chrono::{Local, NaiveDateTime};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const OUTPUT_PREFIX: &str = "Rustitler 分类输出";
const OTHER_CATEGORY: &str = "其他";
const NEEDS_REVIEW_CATEGORY: &str = "待确认";

#[derive(Debug, Clone, PartialEq, Eq)]
enum ClassificationDecision {
    Category(String),
    Other,
    NeedsReview,
}

pub fn classify_folder(
    source_path: impl AsRef<Path>,
    settings: &ClassificationSettings,
) -> Result<ClassificationSummary, AppError> {
    classify_folder_at(source_path, settings, Local::now().naive_local())
}

fn classify_folder_at(
    source_path: impl AsRef<Path>,
    settings: &ClassificationSettings,
    timestamp: NaiveDateTime,
) -> Result<ClassificationSummary, AppError> {
    let source_path = source_path.as_ref();
    validate_source_folder(source_path)?;
    let settings = normalize_classification_settings_for_runtime(settings)?;

    let output_path = create_output_directory(source_path, timestamp)?;
    let runtime_categories = runtime_categories(&settings);
    let mut unavailable_categories = HashMap::new();
    for category in &runtime_categories {
        let category_path = output_path.join(category);
        if let Err(err) = fs::create_dir_all(&category_path) {
            unavailable_categories.insert(category.clone(), err.to_string());
        }
    }

    let mut files = Vec::new();
    let mut scan_failures = Vec::new();
    scan_files(source_path, &mut files, &mut scan_failures);

    let mut category_counts = runtime_categories
        .iter()
        .map(|category| (category.clone(), 0usize))
        .collect::<HashMap<_, _>>();
    let mut failures = scan_failures;
    let total_files = files.len() + failures.len();
    let mut copied_files = 0usize;

    for source_file in files {
        let category = decision_category(decide_classification(&source_file, &settings));
        if let Some(reason) = unavailable_categories.get(&category) {
            failures.push(classification_failure(
                &source_file,
                format!("创建分类目录失败：{reason}"),
            ));
            continue;
        }

        let category_dir = output_path.join(&category);
        let target_path = match unique_target_path(&category_dir, &source_file) {
            Some(path) => path,
            None => {
                failures.push(classification_failure(&source_file, "生成目标文件路径失败"));
                continue;
            }
        };

        match fs::copy(&source_file, &target_path) {
            Ok(_) => {
                copied_files += 1;
                *category_counts.entry(category).or_insert(0) += 1;
            }
            Err(err) => failures.push(classification_failure(
                &source_file,
                format!("复制文件失败：{err}"),
            )),
        }
    }

    Ok(ClassificationSummary {
        source_path: source_path.to_string_lossy().into_owned(),
        output_path: output_path.to_string_lossy().into_owned(),
        total_files,
        copied_files,
        failed_files: failures.len(),
        category_counts: runtime_categories
            .into_iter()
            .map(|category| CategoryCount {
                count: *category_counts.get(&category).unwrap_or(&0),
                category,
            })
            .collect(),
        failures,
    })
}

fn decide_classification(path: &Path, settings: &ClassificationSettings) -> ClassificationDecision {
    if !has_supported_extension(path) {
        return ClassificationDecision::Other;
    }

    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return ClassificationDecision::Other;
    };
    let stem = stem.to_lowercase();
    let mut hits = Vec::new();
    let mut seen = HashSet::new();

    for category in ordinary_categories(settings) {
        if category.keywords.iter().any(|keyword| {
            let keyword = keyword.to_lowercase();
            !keyword.is_empty() && stem.contains(&keyword)
        }) && seen.insert(category.name.as_str())
        {
            hits.push(category.name.clone());
        }
    }

    match hits.len() {
        0 => ClassificationDecision::Other,
        1 => ClassificationDecision::Category(hits.remove(0)),
        _ => ClassificationDecision::NeedsReview,
    }
}

fn validate_source_folder(path: &Path) -> Result<(), AppError> {
    let metadata = fs::metadata(path).map_err(|err| {
        batch_input_error(
            ErrorCode::InvalidCommandArgument,
            "源文件夹不存在。",
            err.to_string(),
            path,
        )
    })?;

    if !metadata.is_dir() {
        return Err(batch_input_error(
            ErrorCode::InvalidCommandArgument,
            "源路径不是文件夹。",
            "source path is not a directory",
            path,
        ));
    }

    fs::read_dir(path).map_err(|err| {
        let code = if err.kind() == io::ErrorKind::PermissionDenied {
            ErrorCode::PermissionDenied
        } else {
            ErrorCode::FileReadFailed
        };
        batch_input_error(code, "源文件夹不可读取。", err.to_string(), path)
    })?;

    Ok(())
}

fn create_output_directory(
    source_path: &Path,
    timestamp: NaiveDateTime,
) -> Result<PathBuf, AppError> {
    let parent = source_path.parent().unwrap_or_else(|| Path::new("."));
    let base_name = format!("{OUTPUT_PREFIX} {}", timestamp.format("%Y-%m-%d %H%M"));

    for sequence in 1usize.. {
        let dirname = if sequence == 1 {
            base_name.clone()
        } else {
            format!("{base_name} ({sequence})")
        };
        let candidate = parent.join(dirname);
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(AppError {
                    code: ErrorCode::OutputDirectoryCreateFailed,
                    category: ErrorCategory::Output,
                    user_message: "创建分类输出目录失败。".into(),
                    technical_detail: Some(err.to_string()),
                    retryable: false,
                    file_path: Some(candidate.to_string_lossy().into_owned()),
                    stage: None,
                });
            }
        }
    }

    Err(AppError::internal("output directory sequence overflow"))
}

fn scan_files(
    directory: &Path,
    files: &mut Vec<PathBuf>,
    failures: &mut Vec<ClassificationFailure>,
) {
    let mut entries = match fs::read_dir(directory) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(err) => {
            failures.push(classification_failure(
                directory,
                format!("读取目录失败：{err}"),
            ));
            return;
        }
    };
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if is_hidden_or_system(&path) {
            continue;
        }

        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(err) => {
                failures.push(classification_failure(
                    &path,
                    format!("读取文件信息失败：{err}"),
                ));
                continue;
            }
        };

        let file_type = metadata.file_type();
        if should_skip_scan_entry(file_type.is_symlink(), is_windows_reparse_point(&metadata)) {
            continue;
        }

        if metadata.is_dir() {
            scan_files(&path, files, failures);
        } else if metadata.is_file() {
            files.push(path);
        }
    }
}

fn has_supported_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase()),
        Some(extension)
            if matches!(
                extension.as_str(),
                "docx" | "doc" | "pdf" | "png" | "jpg" | "jpeg"
            )
    )
}

fn ordinary_categories(
    settings: &ClassificationSettings,
) -> impl Iterator<Item = &crate::models::ClassificationCategory> {
    settings
        .categories
        .iter()
        .filter(|category| category.system_kind.is_none())
}

fn runtime_categories(settings: &ClassificationSettings) -> Vec<String> {
    let mut categories = Vec::new();
    let mut seen = HashSet::new();

    for category in ordinary_categories(settings) {
        if !category.name.is_empty() && seen.insert(category.name.clone()) {
            categories.push(category.name.clone());
        }
    }

    for required in [OTHER_CATEGORY, NEEDS_REVIEW_CATEGORY] {
        if seen.insert(required.to_string()) {
            categories.push(required.to_string());
        }
    }

    categories
}

fn decision_category(decision: ClassificationDecision) -> String {
    match decision {
        ClassificationDecision::Category(category) => category,
        ClassificationDecision::Other => OTHER_CATEGORY.into(),
        ClassificationDecision::NeedsReview => NEEDS_REVIEW_CATEGORY.into(),
    }
}

fn unique_target_path(category_dir: &Path, source_file: &Path) -> Option<PathBuf> {
    let file_name = source_file.file_name()?.to_str()?;
    let first_candidate = category_dir.join(file_name);
    if !first_candidate.exists() {
        return Some(first_candidate);
    }

    let stem = source_file.file_stem()?.to_str()?;
    let extension = source_file
        .extension()
        .and_then(|extension| extension.to_str());
    for sequence in 2usize.. {
        let candidate_name = match extension {
            Some(extension) => format!("{stem} ({sequence}).{extension}"),
            None => format!("{stem} ({sequence})"),
        };
        let candidate = category_dir.join(candidate_name);
        if !candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

fn classification_failure(path: &Path, reason: impl Into<String>) -> ClassificationFailure {
    ClassificationFailure {
        source_path: path.to_string_lossy().into_owned(),
        reason: reason.into(),
    }
}

fn batch_input_error(
    code: ErrorCode,
    user_message: impl Into<String>,
    detail: impl Into<String>,
    path: &Path,
) -> AppError {
    AppError {
        code,
        category: ErrorCategory::Input,
        user_message: user_message.into(),
        technical_detail: Some(detail.into()),
        retryable: false,
        file_path: Some(path.to_string_lossy().into_owned()),
        stage: None,
    }
}

fn is_hidden_or_system(path: &Path) -> bool {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
    {
        return true;
    }

    is_windows_hidden_or_system(path)
}

fn should_skip_scan_entry(is_symlink: bool, is_reparse_point: bool) -> bool {
    is_symlink || is_reparse_point
}

#[cfg(windows)]
fn is_windows_hidden_or_system(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;

    fs::metadata(path)
        .map(|metadata| {
            let attributes = metadata.file_attributes();
            attributes & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
        })
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn is_windows_hidden_or_system(_path: &Path) -> bool {
    false
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ClassificationCategory, ClassificationSettings, SystemClassificationKind};
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn decision_uses_supported_extensions_stem_only_and_case_insensitive_keywords() {
        let settings = settings_with_categories(&[
            ("Alpha", &["alpha"][..]),
            ("Beta", &["BETA"][..]),
            ("Extension", &["pdf"][..]),
        ]);

        assert_eq!(
            decide_classification(Path::new("Quarterly ALPHA.PDF"), &settings),
            ClassificationDecision::Category("Alpha".into())
        );
        assert_eq!(
            decide_classification(Path::new("unknown.docx"), &settings),
            ClassificationDecision::Other
        );
        assert_eq!(
            decide_classification(Path::new("alpha.txt"), &settings),
            ClassificationDecision::Other
        );
        assert_eq!(
            decide_classification(Path::new("untitled.pdf"), &settings),
            ClassificationDecision::Other
        );
        assert_eq!(
            decide_classification(Path::new("alpha beta.jpeg"), &settings),
            ClassificationDecision::NeedsReview
        );
    }

    #[test]
    fn classify_recursively_skips_hidden_files_and_does_not_preserve_source_hierarchy() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::create_dir_all(source.join(".hidden-dir")).unwrap();
        write_file(source.join("root alpha.pdf"));
        write_file(source.join("nested").join("nested beta.docx"));
        write_file(source.join(".hidden.pdf"));
        write_file(source.join(".hidden-dir").join("alpha.pdf"));

        let summary = classify_folder_at(
            &source,
            &settings_with_categories(&[("Alpha", &["alpha"][..]), ("Beta", &["beta"][..])]),
            fixed_time(),
        )
        .unwrap();

        assert_eq!(summary.total_files, 2);
        assert_eq!(summary.copied_files, 2);
        assert_eq!(summary.failed_files, 0);
        let output = PathBuf::from(&summary.output_path);
        assert!(output.join("Alpha").join("root alpha.pdf").is_file());
        assert!(output.join("Beta").join("nested beta.docx").is_file());
        assert!(!output.join("Beta").join("nested").exists());
        assert!(!output.join("Alpha").join(".hidden.pdf").exists());
    }

    #[test]
    fn output_directory_conflicts_append_sequence_without_overwriting() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(&source).unwrap();
        write_file(source.join("alpha.pdf"));
        fs::create_dir(temp.path().join("Rustitler 分类输出 2026-07-03 1530")).unwrap();
        fs::create_dir(temp.path().join("Rustitler 分类输出 2026-07-03 1530 (2)")).unwrap();

        let summary = classify_folder_at(
            &source,
            &settings_with_categories(&[("Alpha", &["alpha"][..])]),
            fixed_time(),
        )
        .unwrap();

        assert!(summary
            .output_path
            .ends_with("Rustitler 分类输出 2026-07-03 1530 (3)"));
        assert!(Path::new(&summary.output_path).is_dir());
    }

    #[test]
    fn target_filename_conflicts_append_sequence_and_preserve_extension_case() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(source.join("a")).unwrap();
        fs::create_dir_all(source.join("b")).unwrap();
        write_file(source.join("a").join("Notice.PDF"));
        write_file(source.join("b").join("Notice.PDF"));

        let summary = classify_folder_at(
            &source,
            &settings_with_categories(&[("Notice", &["notice"][..])]),
            fixed_time(),
        )
        .unwrap();

        let output = PathBuf::from(summary.output_path).join("Notice");
        assert!(output.join("Notice.PDF").is_file());
        assert!(output.join("Notice (2).PDF").is_file());
    }

    #[test]
    fn invalid_nul_category_is_batch_failure_and_does_not_create_output_directory() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(&source).unwrap();
        write_file(source.join("good.pdf"));
        write_file(source.join("bad.pdf"));
        let settings =
            settings_with_categories(&[("Good", &["good"][..]), ("bad\0category", &["bad"][..])]);

        let error = classify_folder_at(&source, &settings, fixed_time()).unwrap_err();

        assert_eq!(error.code, crate::errors::ErrorCode::InvalidSettings);
        assert_no_output_directories(temp.path());
    }

    #[test]
    fn invalid_empty_keyword_settings_fail_before_output_directory_creation() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(&source).unwrap();
        write_file(source.join("alpha.pdf"));
        let settings = ClassificationSettings {
            categories: vec![ClassificationCategory {
                name: "Alpha".into(),
                keywords: vec!["  ".into()],
                system_kind: None,
            }],
        };

        let error = classify_folder_at(&source, &settings, fixed_time()).unwrap_err();

        assert_eq!(error.code, crate::errors::ErrorCode::InvalidSettings);
        assert_no_output_directories(temp.path());
    }

    #[test]
    fn path_traversal_category_settings_fail_before_output_directory_creation() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(&source).unwrap();
        write_file(source.join("alpha.pdf"));
        let settings = settings_with_categories(&[("..", &["alpha"][..])]);

        let error = classify_folder_at(&source, &settings, fixed_time()).unwrap_err();

        assert_eq!(error.code, crate::errors::ErrorCode::InvalidSettings);
        assert_no_output_directories(temp.path());
    }

    #[test]
    fn unsafe_category_names_are_cleaned_before_creating_output_paths() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(&source).unwrap();
        write_file(source.join("danger.pdf"));
        let settings = settings_with_categories(&[("../escaped", &["danger"][..])]);

        let summary = classify_folder_at(&source, &settings, fixed_time()).unwrap();
        let output = PathBuf::from(&summary.output_path);
        let counts = counts_by_category(&summary);

        assert_eq!(counts[".._escaped"], 1);
        assert!(output.join(".._escaped").join("danger.pdf").is_file());
        assert!(!temp.path().join("escaped").exists());
    }

    #[test]
    fn summary_counts_include_runtime_system_categories_even_when_settings_omit_them() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(&source).unwrap();
        write_file(source.join("alpha.pdf"));
        write_file(source.join("alpha beta.pdf"));
        write_file(source.join("loose.xlsx"));

        let summary = classify_folder_at(
            &source,
            &settings_with_categories(&[("Alpha", &["alpha"][..]), ("Beta", &["beta"][..])]),
            fixed_time(),
        )
        .unwrap();
        let counts = counts_by_category(&summary);

        assert_eq!(summary.total_files, 3);
        assert_eq!(summary.copied_files, 3);
        assert_eq!(counts["Alpha"], 1);
        assert_eq!(counts["待确认"], 1);
        assert_eq!(counts["其他"], 1);
        assert_eq!(counts["Beta"], 0);
    }

    #[test]
    fn recursive_scan_skips_symlinked_files_and_directories_without_counting_them() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&outside).unwrap();
        write_file(source.join("alpha.pdf"));
        write_file(outside.join("beta.pdf"));

        let file_link_created =
            try_create_file_symlink(&source.join("alpha.pdf"), &source.join("alpha-link.pdf"));
        if !try_create_dir_link(&outside, &source.join("outside-link")) {
            return;
        }

        let summary = classify_folder_at(
            &source,
            &settings_with_categories(&[("Alpha", &["alpha"][..]), ("Beta", &["beta"][..])]),
            fixed_time(),
        )
        .unwrap();
        let output = PathBuf::from(&summary.output_path);
        let counts = counts_by_category(&summary);

        assert_eq!(summary.total_files, 1);
        assert_eq!(summary.copied_files, 1);
        assert_eq!(counts["Alpha"], 1);
        assert_eq!(counts["Beta"], 0);
        assert!(output.join("Alpha").join("alpha.pdf").is_file());
        if file_link_created {
            assert!(!output.join("Alpha").join("alpha-link.pdf").exists());
        }
        assert!(!output.join("Beta").join("beta.pdf").exists());
    }

    #[test]
    fn scan_entry_policy_skips_symlink_or_reparse_entries() {
        assert!(should_skip_scan_entry(true, false));
        assert!(should_skip_scan_entry(false, true));
        assert!(!should_skip_scan_entry(false, false));
    }

    #[test]
    fn acceptance_sample_places_six_files_into_expected_categories() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("输入文件夹");
        fs::create_dir_all(source.join("子目录")).unwrap();
        write_file(source.join("关于预算调整的请示.pdf"));
        write_file(source.join("工作总结报告.docx"));
        write_file(source.join("会议通知.png"));
        write_file(source.join("技术标准.txt"));
        write_file(source.join("通知报告.pdf"));
        write_file(source.join("子目录").join("未知材料.xlsx"));
        let settings = settings_with_categories(&[
            ("请示", &["请示"][..]),
            ("报告", &["报告"][..]),
            ("通知", &["通知"][..]),
            ("标准", &["标准"][..]),
        ]);

        let summary = classify_folder_at(&source, &settings, fixed_time()).unwrap();
        let output = PathBuf::from(&summary.output_path);
        let counts = counts_by_category(&summary);

        assert!(output.join("请示").join("关于预算调整的请示.pdf").is_file());
        assert!(output.join("报告").join("工作总结报告.docx").is_file());
        assert!(output.join("通知").join("会议通知.png").is_file());
        assert!(output.join("待确认").join("通知报告.pdf").is_file());
        assert!(output.join("其他").join("技术标准.txt").is_file());
        assert!(output.join("其他").join("未知材料.xlsx").is_file());
        assert_eq!(summary.total_files, 6);
        assert_eq!(summary.copied_files, 6);
        assert_eq!(summary.failed_files, 0);
        assert_eq!(counts["请示"], 1);
        assert_eq!(counts["报告"], 1);
        assert_eq!(counts["通知"], 1);
        assert_eq!(counts["标准"], 0);
        assert_eq!(counts["待确认"], 1);
        assert_eq!(counts["其他"], 2);
    }

    #[test]
    fn invalid_source_paths_are_batch_level_errors() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("missing");
        let file = temp.path().join("file.pdf");
        write_file(&file);

        let missing_error =
            classify_folder(&missing, &ClassificationSettings::default()).unwrap_err();
        let file_error = classify_folder(&file, &ClassificationSettings::default()).unwrap_err();

        assert_eq!(
            missing_error.file_path.as_deref(),
            Some(missing.to_string_lossy().as_ref())
        );
        assert_eq!(
            file_error.file_path.as_deref(),
            Some(file.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn classify_module_stays_isolated_from_title_pipeline_modules() {
        let source = include_str!("classify.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        for forbidden in ["ex", "scor", "ren", "hist"]
            .into_iter()
            .zip(["tract", "ing", "ame", "ory"])
            .map(|(left, right)| format!("crate::{}{}", left, right))
        {
            assert!(
                !source.contains(&forbidden),
                "found forbidden dependency {forbidden}"
            );
        }
        let forbidden_method = ["with", "_stage"].concat();
        assert!(!source.contains(&forbidden_method));
        for forbidden in [
            ("req", "west"),
            ("ur", "eq"),
            ("open", "ai"),
            ("chat", "gpt"),
            ("cl", "oud"),
            ("net", "work"),
            ("http", "://"),
            ("https", "://"),
        ]
        .into_iter()
        .map(|(left, right)| format!("{left}{right}"))
        {
            assert!(
                !source.to_ascii_lowercase().contains(&forbidden),
                "found forbidden AI/cloud/network symbol {forbidden}"
            );
        }
    }

    fn settings_with_categories(categories: &[(&str, &[&str])]) -> ClassificationSettings {
        ClassificationSettings {
            categories: categories
                .iter()
                .map(|(name, keywords)| ClassificationCategory {
                    name: (*name).into(),
                    keywords: keywords.iter().map(|keyword| (*keyword).into()).collect(),
                    system_kind: None,
                })
                .chain([
                    ClassificationCategory {
                        name: "其他".into(),
                        keywords: vec![],
                        system_kind: Some(SystemClassificationKind::Other),
                    },
                    ClassificationCategory {
                        name: "待确认".into(),
                        keywords: vec![],
                        system_kind: Some(SystemClassificationKind::NeedsReview),
                    },
                ])
                .collect(),
        }
    }

    fn counts_by_category(
        summary: &crate::models::ClassificationSummary,
    ) -> HashMap<String, usize> {
        summary
            .category_counts
            .iter()
            .map(|count| (count.category.clone(), count.count))
            .collect()
    }

    fn fixed_time() -> NaiveDateTime {
        NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2026, 7, 3).unwrap(),
            NaiveTime::from_hms_opt(15, 30, 0).unwrap(),
        )
    }

    fn write_file(path: impl AsRef<Path>) {
        fs::write(path, b"fixture").unwrap();
    }

    #[cfg(unix)]
    fn try_create_file_symlink(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn try_create_file_symlink(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }

    #[cfg(unix)]
    fn try_create_dir_link(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn try_create_dir_link(target: &Path, link: &Path) -> bool {
        if std::os::windows::fs::symlink_dir(target, link).is_ok() {
            return true;
        }

        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .status()
            .is_ok_and(|status| status.success())
    }

    fn assert_no_output_directories(parent: &Path) {
        let output_dirs = fs::read_dir(parent)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(OUTPUT_PREFIX))
            })
            .collect::<Vec<_>>();
        assert!(
            output_dirs.is_empty(),
            "unexpected output dirs: {output_dirs:?}"
        );
    }
}
