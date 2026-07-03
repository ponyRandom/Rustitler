use crate::errors::{AppError, ErrorCategory, ErrorCode};
use crate::models::{
    ClassificationCategory, ClassificationSettings, KeywordRule, RegexRule, Settings,
    DEFAULT_MAX_TITLE_CHARS, SETTINGS_VERSION,
};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const MAX_RULES: usize = 100;
const MIN_SENSITIVITY: f32 = 0.0;
const MAX_SENSITIVITY: f32 = 2.0;
const MIN_TITLE_CHAR_LIMIT: u16 = 10;
const MAX_TITLE_CHAR_LIMIT: u16 = 120;
const RESERVED_CLASSIFICATION_CATEGORY_NAMES: [&str; 2] = ["其他", "待确认"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub id: String,
    pub captured_at: String,
    pub settings: Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportExportSettings {
    version: u16,
    auto_output_threshold: u8,
    layout_sensitivity: f32,
    position_sensitivity: f32,
    keyword_sensitivity: f32,
    text_quality_sensitivity: f32,
    ocr_conservatism: f32,
    #[serde(default = "default_import_export_max_title_chars")]
    max_title_chars: u16,
    keyword_rules: Vec<KeywordRule>,
    regex_rules: Vec<RegexRule>,
    debug_mode: bool,
}

fn default_import_export_max_title_chars() -> u16 {
    DEFAULT_MAX_TITLE_CHARS
}

impl From<&Settings> for ImportExportSettings {
    fn from(settings: &Settings) -> Self {
        Self {
            version: settings.version,
            auto_output_threshold: settings.auto_output_threshold,
            layout_sensitivity: settings.layout_sensitivity,
            position_sensitivity: settings.position_sensitivity,
            keyword_sensitivity: settings.keyword_sensitivity,
            text_quality_sensitivity: settings.text_quality_sensitivity,
            ocr_conservatism: settings.ocr_conservatism,
            max_title_chars: settings.max_title_chars,
            keyword_rules: settings.keyword_rules.clone(),
            regex_rules: settings.regex_rules.clone(),
            debug_mode: settings.debug_mode,
        }
    }
}

impl From<ImportExportSettings> for Settings {
    fn from(settings: ImportExportSettings) -> Self {
        Self {
            version: settings.version,
            auto_output_threshold: settings.auto_output_threshold,
            layout_sensitivity: settings.layout_sensitivity,
            position_sensitivity: settings.position_sensitivity,
            keyword_sensitivity: settings.keyword_sensitivity,
            text_quality_sensitivity: settings.text_quality_sensitivity,
            ocr_conservatism: settings.ocr_conservatism,
            max_title_chars: settings.max_title_chars,
            keyword_rules: settings.keyword_rules,
            regex_rules: settings.regex_rules,
            debug_mode: settings.debug_mode,
            classification_settings: ClassificationSettings::default(),
        }
    }
}

pub fn settings_path(app_data_dir: impl AsRef<Path>) -> PathBuf {
    app_data_dir.as_ref().join("settings.json")
}

pub fn load_settings(app_data_dir: impl AsRef<Path>) -> Result<Settings, AppError> {
    let path = settings_path(app_data_dir);

    if !path.exists() {
        let settings = Settings::default();
        write_settings_file(&path, &settings)?;
        return Ok(settings);
    }

    let settings = normalize_settings(&read_settings_file(&path)?)?;
    validate_settings(&settings)?;
    Ok(settings)
}

pub fn save_settings(
    app_data_dir: impl AsRef<Path>,
    settings: &Settings,
) -> Result<Settings, AppError> {
    let settings = normalize_settings(settings)?;
    validate_settings(&settings)?;
    let path = settings_path(app_data_dir);
    write_settings_file(&path, &settings)?;
    Ok(settings)
}

pub fn import_settings(path: impl AsRef<Path>) -> Result<Settings, AppError> {
    let settings = normalize_settings(&read_import_export_settings_file(path.as_ref())?)?;
    validate_settings(&settings)?;
    Ok(settings)
}

pub fn export_settings(settings: &Settings, path: impl AsRef<Path>) -> Result<(), AppError> {
    let export_settings = ImportExportSettings::from(settings);
    let validation_settings: Settings = export_settings.clone().into();
    validate_settings(&validation_settings)?;
    write_import_export_settings_file(path.as_ref(), &export_settings)
}

pub fn reset_settings(app_data_dir: impl AsRef<Path>) -> Result<Settings, AppError> {
    let settings = Settings::default();
    save_settings(app_data_dir, &settings)
}

pub fn create_settings_snapshot(settings: &Settings) -> SettingsSnapshot {
    SettingsSnapshot {
        id: Uuid::new_v4().to_string(),
        captured_at: Utc::now().to_rfc3339(),
        settings: settings.clone(),
    }
}

pub fn validate_settings(settings: &Settings) -> Result<(), AppError> {
    if settings.version != SETTINGS_VERSION {
        return Err(settings_error(format!(
            "设置版本不兼容：当前支持版本为 {SETTINGS_VERSION}。"
        )));
    }

    if settings.auto_output_threshold > 100 {
        return Err(settings_error("自动输出阈值必须在 0-100 之间。"));
    }

    validate_sensitivity("版式敏感度", settings.layout_sensitivity)?;
    validate_sensitivity("位置敏感度", settings.position_sensitivity)?;
    validate_sensitivity("关键词敏感度", settings.keyword_sensitivity)?;
    validate_sensitivity("文本质量敏感度", settings.text_quality_sensitivity)?;
    validate_sensitivity("OCR 保守度敏感度", settings.ocr_conservatism)?;
    if !(MIN_TITLE_CHAR_LIMIT..=MAX_TITLE_CHAR_LIMIT).contains(&settings.max_title_chars) {
        return Err(settings_error(format!(
            "标题最大字数必须在 {MIN_TITLE_CHAR_LIMIT}-{MAX_TITLE_CHAR_LIMIT} 之间。"
        )));
    }
    validate_rule_count(settings.keyword_rules.len(), "关键词规则")?;
    validate_rule_count(settings.regex_rules.len(), "正则规则")?;

    for rule in &settings.keyword_rules {
        if rule.keyword.trim().is_empty() {
            return Err(settings_error("关键词规则不能为空。"));
        }
    }

    for RegexRule { pattern, .. } in &settings.regex_rules {
        if pattern.trim().is_empty() {
            return Err(settings_error("正则规则不能为空。"));
        }

        Regex::new(pattern).map_err(|err| settings_error(format!("正则规则无法编译：{err}")))?;
    }

    normalize_classification_settings(&settings.classification_settings)?;

    Ok(())
}

pub(crate) fn normalize_classification_settings_for_runtime(
    settings: &ClassificationSettings,
) -> Result<ClassificationSettings, AppError> {
    normalize_classification_settings(settings)
}

fn normalize_settings(settings: &Settings) -> Result<Settings, AppError> {
    let mut normalized = settings.clone();
    normalized.classification_settings =
        normalize_classification_settings(&settings.classification_settings)?;
    Ok(normalized)
}

fn normalize_classification_settings(
    settings: &ClassificationSettings,
) -> Result<ClassificationSettings, AppError> {
    let mut seen_names = HashSet::new();
    let mut seen_keywords = HashSet::new();
    let mut categories = Vec::with_capacity(settings.categories.len());

    for category in &settings.categories {
        if category.name.contains('\0') {
            return Err(settings_error(
                "Classification category name cannot contain NUL.",
            ));
        }
        let name = clean_classification_value(&category.name);
        if category.system_kind.is_none() && name.is_empty() {
            return Err(settings_error("分类名称不能为空。"));
        }
        if is_unsafe_classification_path_component(&name) {
            return Err(settings_error(
                "Classification category name cannot be a path traversal component.",
            ));
        }
        if category.system_kind.is_none() && is_reserved_classification_category_name(&name) {
            return Err(settings_error(
                "Classification category name is reserved for runtime buckets.",
            ));
        }
        let name_key = classification_identity_key(&name);
        if !name.is_empty() && !seen_names.insert(name_key) {
            return Err(settings_error("分类名称重复。"));
        }

        let keywords: Vec<String> = category
            .keywords
            .iter()
            .map(|keyword| clean_classification_value(keyword))
            .filter(|keyword| !keyword.is_empty())
            .collect();

        if category.system_kind.is_none() && keywords.is_empty() {
            return Err(settings_error("普通分类至少需要一个关键词。"));
        }

        for keyword in &keywords {
            if !seen_keywords.insert(classification_identity_key(keyword)) {
                return Err(settings_error("分类关键词重复。"));
            }
        }

        categories.push(ClassificationCategory {
            name,
            keywords,
            system_kind: category.system_kind.clone(),
        });
    }

    Ok(ClassificationSettings { categories })
}

fn clean_classification_value(value: &str) -> String {
    let replaced: String = value
        .chars()
        .map(|ch| if is_path_illegal_char(ch) { '_' } else { ch })
        .collect();

    replaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_path_illegal_char(ch: char) -> bool {
    matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        || (ch.is_control() && !ch.is_whitespace())
}

fn is_unsafe_classification_path_component(value: &str) -> bool {
    value == "." || value == ".."
}

fn is_reserved_classification_category_name(value: &str) -> bool {
    let key = classification_identity_key(value);
    RESERVED_CLASSIFICATION_CATEGORY_NAMES
        .iter()
        .any(|reserved| classification_identity_key(reserved) == key)
}

fn classification_identity_key(value: &str) -> String {
    value.to_lowercase()
}

fn validate_sensitivity(label: &str, value: f32) -> Result<(), AppError> {
    if !value.is_finite() || !(MIN_SENSITIVITY..=MAX_SENSITIVITY).contains(&value) {
        return Err(settings_error(format!(
            "{label}必须在 {MIN_SENSITIVITY}-{MAX_SENSITIVITY} 之间。"
        )));
    }

    Ok(())
}

fn validate_rule_count(count: usize, label: &str) -> Result<(), AppError> {
    if count > MAX_RULES {
        return Err(settings_error(format!("{label}最多支持 {MAX_RULES} 条。")));
    }

    Ok(())
}

fn read_settings_file(path: &Path) -> Result<Settings, AppError> {
    let contents = fs::read_to_string(path).map_err(|err| {
        settings_error(format!("读取设置文件失败：{err}")).with_path(path.display().to_string())
    })?;

    serde_json::from_str(&contents).map_err(|err| {
        settings_error(format!("解析设置文件失败：{err}")).with_path(path.display().to_string())
    })
}

fn write_settings_file(path: &Path, settings: &Settings) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            settings_error(format!("创建设置目录失败：{err}"))
                .with_path(parent.display().to_string())
        })?;
    }

    let temp_path = path.with_extension("json.tmp");
    let contents = serde_json::to_string_pretty(settings)
        .map_err(|err| settings_error(format!("序列化设置失败：{err}")))?;

    fs::write(&temp_path, contents).map_err(|err| {
        settings_error(format!("写入设置临时文件失败：{err}"))
            .with_path(temp_path.display().to_string())
    })?;
    fs::rename(&temp_path, path).map_err(|err| {
        settings_error(format!("替换设置文件失败：{err}")).with_path(path.display().to_string())
    })?;

    Ok(())
}

fn read_import_export_settings_file(path: &Path) -> Result<Settings, AppError> {
    let contents = fs::read_to_string(path).map_err(|err| {
        settings_error(format!("读取设置文件失败：{err}")).with_path(path.display().to_string())
    })?;

    let settings: ImportExportSettings = serde_json::from_str(&contents).map_err(|err| {
        settings_error(format!("解析设置文件失败：{err}")).with_path(path.display().to_string())
    })?;

    Ok(settings.into())
}

fn write_import_export_settings_file(
    path: &Path,
    settings: &ImportExportSettings,
) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            settings_error(format!("创建设置目录失败：{err}"))
                .with_path(parent.display().to_string())
        })?;
    }

    let temp_path = path.with_extension("json.tmp");
    let contents = serde_json::to_string_pretty(settings)
        .map_err(|err| settings_error(format!("序列化设置失败：{err}")))?;

    fs::write(&temp_path, contents).map_err(|err| {
        settings_error(format!("写入设置临时文件失败：{err}"))
            .with_path(temp_path.display().to_string())
    })?;
    fs::rename(&temp_path, path).map_err(|err| {
        settings_error(format!("替换设置文件失败：{err}")).with_path(path.display().to_string())
    })?;

    Ok(())
}

fn settings_error(message: impl Into<String>) -> AppError {
    AppError {
        code: ErrorCode::InvalidSettings,
        category: ErrorCategory::Settings,
        user_message: message.into(),
        technical_detail: None,
        retryable: false,
        file_path: None,
        stage: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        ClassificationCategory, ClassificationSettings, KeywordRule, RegexRule, Settings,
        SystemClassificationKind, SETTINGS_VERSION,
    };

    #[test]
    fn settings_path_uses_app_data_dir() {
        let dir = tempfile::tempdir().unwrap();

        let path = settings_path(dir.path());

        assert_eq!(path, dir.path().join("settings.json"));
    }

    #[test]
    fn load_settings_creates_default_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();

        let settings = load_settings(dir.path()).unwrap();

        assert_eq!(settings.auto_output_threshold, 70);
        assert_eq!(settings.max_title_chars, 45);
        assert!(!settings.debug_mode);
        assert_eq!(
            settings
                .classification_settings
                .categories
                .iter()
                .map(|category| (category.name.as_str(), category.keywords.as_slice()))
                .collect::<Vec<_>>(),
            vec![
                ("请示", &["请示".to_string()][..]),
                ("报告", &["报告".to_string()][..]),
                ("通知", &["通知".to_string()][..]),
                ("标准", &["标准".to_string()][..]),
            ]
        );
        assert!(settings_path(dir.path()).exists());
        let json = std::fs::read_to_string(settings_path(dir.path())).unwrap();
        assert!(json.contains("\"autoOutputThreshold\": 70"));
        assert!(json.contains("\"maxTitleChars\": 45"));
        assert!(json.contains("\"classificationSettings\""));
    }

    #[test]
    fn save_settings_validates_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            auto_output_threshold: 65,
            layout_sensitivity: 1.5,
            ..Settings::default()
        };

        let saved = save_settings(dir.path(), &settings).unwrap();
        let loaded = load_settings(dir.path()).unwrap();

        assert_eq!(saved.auto_output_threshold, 65);
        assert_eq!(loaded.auto_output_threshold, 65);
        assert_eq!(loaded.layout_sensitivity, 1.5);
        assert_eq!(loaded.max_title_chars, 45);
        assert!(!dir.path().join("settings.json.tmp").exists());
    }

    #[test]
    fn load_settings_accepts_old_files_without_classification_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path(dir.path());
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(
            &path,
            r#"{
              "version": 1,
              "autoOutputThreshold": 70,
              "layoutSensitivity": 1.0,
              "positionSensitivity": 1.0,
              "keywordSensitivity": 1.0,
              "textQualitySensitivity": 1.0,
              "ocrConservatism": 1.0,
              "maxTitleChars": 45,
              "keywordRules": [],
              "regexRules": [],
              "debugMode": false
            }"#,
        )
        .unwrap();

        let loaded = load_settings(dir.path()).unwrap();

        assert_eq!(
            loaded
                .classification_settings
                .categories
                .iter()
                .map(|category| category.name.as_str())
                .collect::<Vec<_>>(),
            vec!["请示", "报告", "通知", "标准"]
        );
    }

    #[test]
    fn save_settings_persists_cleaned_classification_settings() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            classification_settings: ClassificationSettings {
                categories: vec![
                    ClassificationCategory {
                        name: "  请<示>\t\t分类  ".into(),
                        keywords: vec!["  请/示\n\n关键  ".into()],
                        system_kind: None,
                    },
                    ClassificationCategory {
                        name: " 其他 ".into(),
                        keywords: vec![],
                        system_kind: Some(SystemClassificationKind::Other),
                    },
                ],
            },
            ..Settings::default()
        };

        let saved = save_settings(dir.path(), &settings).unwrap();
        let loaded = load_settings(dir.path()).unwrap();

        assert_eq!(
            settings.classification_settings.categories[0].name,
            "  请<示>\t\t分类  "
        );
        assert_eq!(
            saved.classification_settings.categories[0].name,
            "请_示_ 分类"
        );
        assert_eq!(
            saved.classification_settings.categories[0].keywords,
            vec!["请_示 关键".to_string()]
        );
        assert_eq!(
            loaded.classification_settings,
            saved.classification_settings
        );
        let json = std::fs::read_to_string(settings_path(dir.path())).unwrap();
        assert!(json.contains("请_示_ 分类"));
        assert!(!json.contains("请<示>"));
    }

    #[test]
    fn validate_rejects_empty_ordinary_classification_names_and_keywords_after_cleaning() {
        let empty_name = Settings {
            classification_settings: ClassificationSettings {
                categories: vec![ClassificationCategory {
                    name: " \t\n ".into(),
                    keywords: vec!["valid".into()],
                    system_kind: None,
                }],
            },
            ..Settings::default()
        };
        let empty_keywords = Settings {
            classification_settings: ClassificationSettings {
                categories: vec![ClassificationCategory {
                    name: "Valid".into(),
                    keywords: vec![" \t ".into(), "\n".into()],
                    system_kind: None,
                }],
            },
            ..Settings::default()
        };

        assert!(validate_settings(&empty_name)
            .unwrap_err()
            .user_message
            .contains("分类"));
        assert!(validate_settings(&empty_keywords)
            .unwrap_err()
            .user_message
            .contains("关键词"));
    }

    #[test]
    fn validate_rejects_duplicate_classification_names_after_cleaning() {
        let settings = Settings {
            classification_settings: ClassificationSettings {
                categories: vec![
                    ClassificationCategory {
                        name: "A<B".into(),
                        keywords: vec!["a".into()],
                        system_kind: None,
                    },
                    ClassificationCategory {
                        name: " A:B ".into(),
                        keywords: vec!["b".into()],
                        system_kind: None,
                    },
                ],
            },
            ..Settings::default()
        };

        let error = validate_settings(&settings).unwrap_err();

        assert!(error.user_message.contains("分类"));
        assert!(error.user_message.contains("重复"));
    }

    #[test]
    fn validate_rejects_duplicate_classification_keywords_after_cleaning() {
        let settings = Settings {
            classification_settings: ClassificationSettings {
                categories: vec![
                    ClassificationCategory {
                        name: "A".into(),
                        keywords: vec!["dup<key".into()],
                        system_kind: None,
                    },
                    ClassificationCategory {
                        name: "B".into(),
                        keywords: vec![" dup:key ".into()],
                        system_kind: None,
                    },
                ],
            },
            ..Settings::default()
        };

        let error = validate_settings(&settings).unwrap_err();

        assert!(error.user_message.contains("关键词"));
        assert!(error.user_message.contains("重复"));
    }

    #[test]
    fn validate_rejects_duplicate_classification_keywords_within_same_category() {
        let settings = Settings {
            classification_settings: ClassificationSettings {
                categories: vec![ClassificationCategory {
                    name: "A".into(),
                    keywords: vec!["dup<key".into(), " dup:key ".into()],
                    system_kind: None,
                }],
            },
            ..Settings::default()
        };

        let error = validate_settings(&settings).unwrap_err();

        assert!(error.user_message.contains("关键词"));
        assert!(error.user_message.contains("重复"));
    }

    #[test]
    fn normalize_classification_settings_for_runtime_cleans_names_without_writing() {
        let settings = ClassificationSettings {
            categories: vec![ClassificationCategory {
                name: "../Escaped".into(),
                keywords: vec![" danger ".into()],
                system_kind: None,
            }],
        };

        let normalized = normalize_classification_settings_for_runtime(&settings).unwrap();

        assert_eq!(normalized.categories[0].name, ".._Escaped");
        assert_eq!(normalized.categories[0].keywords, vec!["danger"]);
    }

    #[test]
    fn normalize_classification_settings_for_runtime_rejects_path_traversal_names() {
        let settings = ClassificationSettings {
            categories: vec![ClassificationCategory {
                name: "..".into(),
                keywords: vec!["danger".into()],
                system_kind: None,
            }],
        };

        let error = normalize_classification_settings_for_runtime(&settings).unwrap_err();

        assert_eq!(error.code, ErrorCode::InvalidSettings);
    }

    #[test]
    fn normalize_classification_settings_for_runtime_rejects_reserved_ordinary_category_names() {
        for reserved_name in ["其他", "待确认"] {
            let settings = ClassificationSettings {
                categories: vec![ClassificationCategory {
                    name: reserved_name.into(),
                    keywords: vec!["keyword".into()],
                    system_kind: None,
                }],
            };

            let error = normalize_classification_settings_for_runtime(&settings).unwrap_err();

            assert_eq!(error.code, ErrorCode::InvalidSettings);
        }
    }

    #[test]
    fn normalize_classification_settings_for_runtime_rejects_case_folded_duplicates() {
        let duplicate_names = ClassificationSettings {
            categories: vec![
                ClassificationCategory {
                    name: "Report".into(),
                    keywords: vec!["report".into()],
                    system_kind: None,
                },
                ClassificationCategory {
                    name: "report".into(),
                    keywords: vec!["summary".into()],
                    system_kind: None,
                },
            ],
        };
        let duplicate_keywords = ClassificationSettings {
            categories: vec![
                ClassificationCategory {
                    name: "Alpha".into(),
                    keywords: vec!["Alpha".into()],
                    system_kind: None,
                },
                ClassificationCategory {
                    name: "Beta".into(),
                    keywords: vec!["alpha".into()],
                    system_kind: None,
                },
            ],
        };

        assert_eq!(
            normalize_classification_settings_for_runtime(&duplicate_names)
                .unwrap_err()
                .code,
            ErrorCode::InvalidSettings
        );
        assert_eq!(
            normalize_classification_settings_for_runtime(&duplicate_keywords)
                .unwrap_err()
                .code,
            ErrorCode::InvalidSettings
        );
    }

    #[test]
    fn validate_allows_missing_system_classification_categories() {
        let settings = Settings {
            classification_settings: ClassificationSettings {
                categories: vec![ClassificationCategory {
                    name: "Only Ordinary".into(),
                    keywords: vec!["ordinary".into()],
                    system_kind: None,
                }],
            },
            ..Settings::default()
        };

        validate_settings(&settings).unwrap();
    }

    #[test]
    fn validate_rejects_invalid_threshold() {
        let settings = Settings {
            auto_output_threshold: 101,
            ..Settings::default()
        };

        let error = validate_settings(&settings).unwrap_err();

        assert_eq!(error.code, ErrorCode::InvalidSettings);
        assert!(error.user_message.contains("阈值"));
    }

    #[test]
    fn validate_rejects_invalid_sensitivity() {
        let settings = Settings {
            ocr_conservatism: 2.1,
            ..Settings::default()
        };

        let error = validate_settings(&settings).unwrap_err();

        assert!(error.user_message.contains("敏感度"));
    }

    #[test]
    fn validate_rejects_invalid_title_char_limit() {
        let too_small = Settings {
            max_title_chars: 9,
            ..Settings::default()
        };
        let too_large = Settings {
            max_title_chars: 121,
            ..Settings::default()
        };

        assert!(validate_settings(&too_small)
            .unwrap_err()
            .user_message
            .contains("标题最大字数"));
        assert!(validate_settings(&too_large)
            .unwrap_err()
            .user_message
            .contains("标题最大字数"));
    }

    #[test]
    fn validate_rejects_empty_or_too_many_keyword_rules() {
        let empty_keyword = Settings {
            keyword_rules: vec![KeywordRule {
                keyword: "   ".into(),
                score_delta: 5,
            }],
            ..Settings::default()
        };

        let too_many = Settings {
            keyword_rules: (0..=MAX_RULES)
                .map(|i| KeywordRule {
                    keyword: format!("关键词{i}"),
                    score_delta: 1,
                })
                .collect(),
            ..Settings::default()
        };

        assert!(validate_settings(&empty_keyword).is_err());
        assert!(validate_settings(&too_many).is_err());
    }

    #[test]
    fn validate_rejects_invalid_regex_and_version() {
        let invalid_regex = Settings {
            regex_rules: vec![RegexRule {
                pattern: "(".into(),
                score_delta: 5,
            }],
            ..Settings::default()
        };

        let invalid_version = Settings {
            version: SETTINGS_VERSION + 1,
            ..Settings::default()
        };

        assert!(validate_settings(&invalid_regex)
            .unwrap_err()
            .user_message
            .contains("正则"));
        assert!(validate_settings(&invalid_version)
            .unwrap_err()
            .user_message
            .contains("版本"));
    }

    #[test]
    fn import_settings_reuses_validation_chain() {
        let dir = tempfile::tempdir().unwrap();
        let import_path = dir.path().join("import.json");
        let settings = Settings {
            regex_rules: vec![RegexRule {
                pattern: "^合同".into(),
                score_delta: 8,
            }],
            ..Settings::default()
        };
        std::fs::write(
            &import_path,
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let imported = import_settings(&import_path).unwrap();

        assert_eq!(imported.regex_rules[0].pattern, "^合同");
    }

    #[test]
    fn export_settings_writes_valid_json_to_target_path() {
        let dir = tempfile::tempdir().unwrap();
        let export_path = dir.path().join("nested").join("settings-export.json");

        export_settings(&Settings::default(), &export_path).unwrap();

        let exported: Settings =
            serde_json::from_str(&std::fs::read_to_string(export_path).unwrap()).unwrap();
        assert_eq!(exported.auto_output_threshold, 70);
    }

    #[test]
    fn import_settings_ignores_classification_settings_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let import_path = dir.path().join("import-with-classification.json");
        std::fs::write(
            &import_path,
            r#"{
              "version": 1,
              "autoOutputThreshold": 66,
              "layoutSensitivity": 1.0,
              "positionSensitivity": 1.0,
              "keywordSensitivity": 1.0,
              "textQualitySensitivity": 1.0,
              "ocrConservatism": 1.0,
              "maxTitleChars": 45,
              "keywordRules": [],
              "regexRules": [],
              "debugMode": true,
              "classificationSettings": {
                "categories": [
                  {
                    "name": "Imported Category",
                    "keywords": ["imported"],
                    "systemKind": null
                  }
                ]
              }
            }"#,
        )
        .unwrap();

        let imported = import_settings(&import_path).unwrap();

        assert_eq!(imported.auto_output_threshold, 66);
        assert!(imported.debug_mode);
        assert_eq!(
            imported.classification_settings,
            ClassificationSettings::default()
        );
    }

    #[test]
    fn export_settings_omits_classification_settings() {
        let dir = tempfile::tempdir().unwrap();
        let export_path = dir.path().join("settings-export.json");
        let settings = Settings {
            classification_settings: ClassificationSettings {
                categories: vec![ClassificationCategory {
                    name: "Local Only".into(),
                    keywords: vec!["local".into()],
                    system_kind: None,
                }],
            },
            ..Settings::default()
        };

        export_settings(&settings, &export_path).unwrap();

        let exported: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(export_path).unwrap()).unwrap();
        assert_eq!(exported["autoOutputThreshold"], 70);
        assert!(exported.get("classificationSettings").is_none());
    }

    #[test]
    fn export_settings_ignores_invalid_local_classification_settings() {
        let dir = tempfile::tempdir().unwrap();
        let export_path = dir.path().join("settings-export.json");
        let settings = Settings {
            auto_output_threshold: 64,
            classification_settings: ClassificationSettings {
                categories: vec![ClassificationCategory {
                    name: " ".into(),
                    keywords: vec![" ".into()],
                    system_kind: None,
                }],
            },
            ..Settings::default()
        };

        export_settings(&settings, &export_path).unwrap();

        let exported: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(export_path).unwrap()).unwrap();
        assert_eq!(exported["autoOutputThreshold"], 64);
        assert!(exported.get("classificationSettings").is_none());
    }

    #[test]
    fn reset_settings_persists_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            auto_output_threshold: 33,
            ..Settings::default()
        };
        save_settings(dir.path(), &settings).unwrap();

        let reset = reset_settings(dir.path()).unwrap();
        let loaded = load_settings(dir.path()).unwrap();

        assert_eq!(reset.auto_output_threshold, 70);
        assert_eq!(loaded.auto_output_threshold, 70);
    }

    #[test]
    fn create_settings_snapshot_clones_current_settings() {
        let mut settings = Settings {
            auto_output_threshold: 80,
            ..Settings::default()
        };

        let snapshot = create_settings_snapshot(&settings);
        settings.auto_output_threshold = 20;

        assert!(!snapshot.id.is_empty());
        assert!(!snapshot.captured_at.is_empty());
        assert_eq!(snapshot.settings.auto_output_threshold, 80);
    }
}
