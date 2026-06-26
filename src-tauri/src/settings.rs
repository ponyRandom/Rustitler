use crate::errors::{AppError, ErrorCategory, ErrorCode};
use crate::models::{RegexRule, Settings, SETTINGS_VERSION};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const MAX_RULES: usize = 100;
const MIN_SENSITIVITY: f32 = 0.0;
const MAX_SENSITIVITY: f32 = 2.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub id: String,
    pub captured_at: String,
    pub settings: Settings,
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

    let settings = read_settings_file(&path)?;
    validate_settings(&settings)?;
    Ok(settings)
}

pub fn save_settings(
    app_data_dir: impl AsRef<Path>,
    settings: &Settings,
) -> Result<Settings, AppError> {
    validate_settings(settings)?;
    let path = settings_path(app_data_dir);
    write_settings_file(&path, settings)?;
    Ok(settings.clone())
}

pub fn import_settings(path: impl AsRef<Path>) -> Result<Settings, AppError> {
    let settings = read_settings_file(path.as_ref())?;
    validate_settings(&settings)?;
    Ok(settings)
}

pub fn export_settings(settings: &Settings, path: impl AsRef<Path>) -> Result<(), AppError> {
    validate_settings(settings)?;
    write_settings_file(path.as_ref(), settings)
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

    Ok(())
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
    use crate::models::{KeywordRule, RegexRule, Settings, SETTINGS_VERSION};

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
        assert!(!settings.debug_mode);
        assert!(settings_path(dir.path()).exists());
        let json = std::fs::read_to_string(settings_path(dir.path())).unwrap();
        assert!(json.contains("\"autoOutputThreshold\": 70"));
    }

    #[test]
    fn save_settings_validates_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings::default();
        settings.auto_output_threshold = 65;
        settings.layout_sensitivity = 1.5;

        let saved = save_settings(dir.path(), &settings).unwrap();
        let loaded = load_settings(dir.path()).unwrap();

        assert_eq!(saved.auto_output_threshold, 65);
        assert_eq!(loaded.auto_output_threshold, 65);
        assert_eq!(loaded.layout_sensitivity, 1.5);
        assert!(!dir.path().join("settings.json.tmp").exists());
    }

    #[test]
    fn validate_rejects_invalid_threshold() {
        let mut settings = Settings::default();
        settings.auto_output_threshold = 101;

        let error = validate_settings(&settings).unwrap_err();

        assert_eq!(error.code, ErrorCode::InvalidSettings);
        assert!(error.user_message.contains("阈值"));
    }

    #[test]
    fn validate_rejects_invalid_sensitivity() {
        let mut settings = Settings::default();
        settings.ocr_conservatism = 2.1;

        let error = validate_settings(&settings).unwrap_err();

        assert!(error.user_message.contains("敏感度"));
    }

    #[test]
    fn validate_rejects_empty_or_too_many_keyword_rules() {
        let mut empty_keyword = Settings::default();
        empty_keyword.keyword_rules = vec![KeywordRule {
            keyword: "   ".into(),
            score_delta: 5,
        }];

        let mut too_many = Settings::default();
        too_many.keyword_rules = (0..=MAX_RULES)
            .map(|i| KeywordRule {
                keyword: format!("关键词{i}"),
                score_delta: 1,
            })
            .collect();

        assert!(validate_settings(&empty_keyword).is_err());
        assert!(validate_settings(&too_many).is_err());
    }

    #[test]
    fn validate_rejects_invalid_regex_and_version() {
        let mut invalid_regex = Settings::default();
        invalid_regex.regex_rules = vec![RegexRule {
            pattern: "(".into(),
            score_delta: 5,
        }];

        let mut invalid_version = Settings::default();
        invalid_version.version = SETTINGS_VERSION + 1;

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
        let mut settings = Settings::default();
        settings.regex_rules = vec![RegexRule {
            pattern: "^合同".into(),
            score_delta: 8,
        }];
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
    fn reset_settings_persists_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings::default();
        settings.auto_output_threshold = 33;
        save_settings(dir.path(), &settings).unwrap();

        let reset = reset_settings(dir.path()).unwrap();
        let loaded = load_settings(dir.path()).unwrap();

        assert_eq!(reset.auto_output_threshold, 70);
        assert_eq!(loaded.auto_output_threshold, 70);
    }

    #[test]
    fn create_settings_snapshot_clones_current_settings() {
        let mut settings = Settings::default();
        settings.auto_output_threshold = 80;

        let snapshot = create_settings_snapshot(&settings);
        settings.auto_output_threshold = 20;

        assert!(!snapshot.id.is_empty());
        assert!(!snapshot.captured_at.is_empty());
        assert_eq!(snapshot.settings.auto_output_threshold, 80);
    }
}
