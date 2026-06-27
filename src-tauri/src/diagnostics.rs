use crate::errors::{AppError, ErrorCategory, ErrorCode, ProcessingStage};
use crate::models::{BatchId, BatchStatus, ExtractedDocument, FileJobId, Settings};
use chrono::{Local, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const DEFAULT_MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
const LOG_FILE_NAME: &str = "rustitler.log";

#[derive(Debug, Clone)]
pub struct DiagnosticsOptions {
    pub max_log_bytes: u64,
    pub current_date: String,
}

impl Default for DiagnosticsOptions {
    fn default() -> Self {
        Self {
            max_log_bytes: DEFAULT_MAX_LOG_BYTES,
            current_date: Local::now().date_naive().to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostics {
    app_data_dir: PathBuf,
    options: DiagnosticsOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSummary {
    pub total: usize,
    pub output_created: usize,
    pub pending: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogRecord {
    timestamp: String,
    scope: &'static str,
    event: String,
    batch_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stage: Option<ProcessingStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<BatchStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_files: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<LogSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<ErrorCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_file_name: Option<String>,
}

impl Diagnostics {
    pub fn new(app_data_dir: impl AsRef<Path>) -> Result<Self, AppError> {
        Self::with_options(app_data_dir, DiagnosticsOptions::default())
    }

    pub fn with_options(
        app_data_dir: impl AsRef<Path>,
        options: DiagnosticsOptions,
    ) -> Result<Self, AppError> {
        let diagnostics = Self {
            app_data_dir: app_data_dir.as_ref().to_path_buf(),
            options,
        };
        diagnostics.ensure_log_dir()?;
        Ok(diagnostics)
    }

    pub fn log_batch_started(
        &self,
        batch_id: &BatchId,
        total_files: usize,
    ) -> Result<(), AppError> {
        self.write_log(&LogRecord {
            timestamp: Utc::now().to_rfc3339(),
            scope: "batch",
            event: "batchStarted".into(),
            batch_id: batch_id.0.clone(),
            file_job_id: None,
            stage: None,
            status: None,
            total_files: Some(total_files),
            summary: None,
            error_code: None,
            output_file_name: None,
        })
    }

    pub fn log_batch_finished(
        &self,
        batch_id: &BatchId,
        status: BatchStatus,
        summary: LogSummary,
    ) -> Result<(), AppError> {
        let event = match status {
            BatchStatus::Completed => "batchCompleted",
            BatchStatus::Cancelled => "batchCancelled",
            BatchStatus::Failed => "batchFailed",
            BatchStatus::Running => "batchRunning",
        };

        self.write_log(&LogRecord {
            timestamp: Utc::now().to_rfc3339(),
            scope: "batch",
            event: event.into(),
            batch_id: batch_id.0.clone(),
            file_job_id: None,
            stage: None,
            status: Some(status),
            total_files: None,
            summary: Some(summary),
            error_code: None,
            output_file_name: None,
        })
    }

    pub fn log_batch_error(&self, batch_id: &BatchId, error: &AppError) -> Result<(), AppError> {
        self.write_log(&LogRecord {
            timestamp: Utc::now().to_rfc3339(),
            scope: "batch",
            event: "batchError".into(),
            batch_id: batch_id.0.clone(),
            file_job_id: None,
            stage: error.stage.clone(),
            status: None,
            total_files: None,
            summary: None,
            error_code: Some(error.code.clone()),
            output_file_name: None,
        })
    }

    pub fn log_file_stage(
        &self,
        batch_id: &BatchId,
        file_job_id: &FileJobId,
        stage: ProcessingStage,
        error_code: Option<ErrorCode>,
        output_path: Option<&str>,
    ) -> Result<(), AppError> {
        self.write_log(&LogRecord {
            timestamp: Utc::now().to_rfc3339(),
            scope: "file",
            event: "fileStage".into(),
            batch_id: batch_id.0.clone(),
            file_job_id: Some(file_job_id.0.clone()),
            stage: Some(stage),
            status: None,
            total_files: None,
            summary: None,
            error_code,
            output_file_name: output_path.and_then(output_file_name),
        })
    }

    pub fn log_file_error(
        &self,
        batch_id: &BatchId,
        file_job_id: &FileJobId,
        error: &AppError,
    ) -> Result<(), AppError> {
        self.write_log(&LogRecord {
            timestamp: Utc::now().to_rfc3339(),
            scope: "file",
            event: "fileError".into(),
            batch_id: batch_id.0.clone(),
            file_job_id: Some(file_job_id.0.clone()),
            stage: error.stage.clone(),
            status: None,
            total_files: None,
            summary: None,
            error_code: Some(error.code.clone()),
            output_file_name: None,
        })
    }

    pub fn save_extracted_document(
        &self,
        settings: &Settings,
        batch_id: &BatchId,
        file_job_id: &FileJobId,
        document: &ExtractedDocument,
    ) -> Result<Option<String>, AppError> {
        if !settings.debug_mode {
            return Ok(None);
        }

        self.save_debug_json(batch_id, file_job_id, "extracted-document", document)
    }

    pub fn save_debug_detail(
        &self,
        settings: &Settings,
        batch_id: &BatchId,
        file_job_id: &FileJobId,
        name: &str,
        value: &Value,
    ) -> Result<Option<String>, AppError> {
        if !settings.debug_mode {
            return Ok(None);
        }

        self.save_debug_json(batch_id, file_job_id, name, value)
    }

    pub fn clear_debug_data(&self) -> Result<(), AppError> {
        let debug_dir = self.debug_dir();
        if debug_dir.exists() {
            fs::remove_dir_all(&debug_dir).map_err(|err| {
                diagnostics_error(format!("清理 Debug 数据失败：{err}"))
                    .with_path(debug_dir.display().to_string())
            })?;
        }

        Ok(())
    }

    fn save_debug_json<T: Serialize>(
        &self,
        batch_id: &BatchId,
        file_job_id: &FileJobId,
        name: &str,
        value: &T,
    ) -> Result<Option<String>, AppError> {
        let file_name = format!("{}.json", sanitize_debug_name(name));
        let dir = self.debug_dir().join(&batch_id.0).join(&file_job_id.0);
        fs::create_dir_all(&dir).map_err(|err| {
            diagnostics_error(format!("创建 Debug 目录失败：{err}"))
                .with_path(dir.display().to_string())
        })?;

        let path = dir.join(&file_name);
        let contents = serde_json::to_string_pretty(value)
            .map_err(|err| diagnostics_error(format!("序列化 Debug 数据失败：{err}")))?;
        fs::write(&path, contents).map_err(|err| {
            diagnostics_error(format!("写入 Debug 数据失败：{err}"))
                .with_path(path.display().to_string())
        })?;

        Ok(Some(format!(
            "debug://{}/{}/{}",
            batch_id.0, file_job_id.0, file_name
        )))
    }

    fn write_log(&self, record: &LogRecord) -> Result<(), AppError> {
        self.rotate_log_if_needed()?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())
            .map_err(|err| diagnostics_error(format!("打开日志文件失败：{err}")))?;
        let line = serde_json::to_string(record)
            .map_err(|err| diagnostics_error(format!("序列化日志失败：{err}")))?;
        writeln!(file, "{line}")
            .map_err(|err| diagnostics_error(format!("写入日志失败：{err}")))?;

        Ok(())
    }

    fn rotate_log_if_needed(&self) -> Result<(), AppError> {
        let log_path = self.log_path();
        if !log_path.exists() {
            return Ok(());
        }

        let metadata = fs::metadata(&log_path)
            .map_err(|err| diagnostics_error(format!("读取日志元数据失败：{err}")))?;
        if metadata.len() >= self.options.max_log_bytes || self.should_rotate_by_date(&log_path)? {
            let rotated = self.next_rotated_log_path();
            fs::rename(&log_path, &rotated).map_err(|err| {
                diagnostics_error(format!("轮转日志失败：{err}"))
                    .with_path(log_path.display().to_string())
            })?;
        }

        Ok(())
    }

    fn should_rotate_by_date(&self, log_path: &Path) -> Result<bool, AppError> {
        let metadata = fs::metadata(log_path)
            .map_err(|err| diagnostics_error(format!("读取日志元数据失败：{err}")))?;
        let modified = metadata
            .modified()
            .map_err(|err| diagnostics_error(format!("读取日志修改时间失败：{err}")))?;
        let modified_date = chrono::DateTime::<Local>::from(modified)
            .date_naive()
            .to_string();

        Ok(modified_date != self.options.current_date)
    }

    fn next_rotated_log_path(&self) -> PathBuf {
        let log_dir = self.log_dir();
        for index in 1.. {
            let candidate = log_dir.join(format!(
                "rustitler-{}-{}.log",
                self.options.current_date, index
            ));
            if !candidate.exists() {
                return candidate;
            }
        }

        unreachable!("unbounded log rotation index loop should always return")
    }

    fn ensure_log_dir(&self) -> Result<(), AppError> {
        fs::create_dir_all(self.log_dir()).map_err(|err| {
            diagnostics_error(format!("创建日志目录失败：{err}"))
                .with_path(self.log_dir().display().to_string())
        })
    }

    fn log_dir(&self) -> PathBuf {
        self.app_data_dir.join("logs")
    }

    fn log_path(&self) -> PathBuf {
        self.log_dir().join(LOG_FILE_NAME)
    }

    fn debug_dir(&self) -> PathBuf {
        self.app_data_dir.join("debug")
    }
}

fn output_file_name(output_path: &str) -> Option<String> {
    Path::new(output_path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
}

fn sanitize_debug_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '-',
        })
        .collect();

    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "debug".into()
    } else {
        trimmed.into()
    }
}

fn diagnostics_error(message: impl Into<String>) -> AppError {
    AppError {
        code: ErrorCode::Internal,
        category: ErrorCategory::System,
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
    use crate::errors::{AppError, ErrorCategory, ErrorCode, ProcessingStage};
    use crate::models::{
        BatchId, BatchStatus, ExtractMethod, ExtractedDocument, FileJobId, FileType, Settings,
    };

    #[test]
    fn creates_log_directory_and_writes_batch_events_as_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let diagnostics = Diagnostics::new(dir.path()).unwrap();
        let summary = LogSummary {
            total: 2,
            output_created: 1,
            pending: 0,
            skipped: 0,
            failed: 1,
            cancelled: 0,
        };

        diagnostics
            .log_batch_started(&BatchId("batch-001".into()), 2)
            .unwrap();
        diagnostics
            .log_batch_finished(
                &BatchId("batch-001".into()),
                BatchStatus::Completed,
                summary,
            )
            .unwrap();

        let contents =
            std::fs::read_to_string(dir.path().join("logs").join("rustitler.log")).unwrap();
        assert!(contents.contains("\"scope\":\"batch\""));
        assert!(contents.contains("\"event\":\"batchStarted\""));
        assert!(contents.contains("\"event\":\"batchCompleted\""));
        assert!(contents.contains("\"batchId\":\"batch-001\""));
    }

    #[test]
    fn file_stage_logs_are_redacted_in_normal_mode() {
        let dir = tempfile::tempdir().unwrap();
        let diagnostics = Diagnostics::new(dir.path()).unwrap();
        let error = AppError {
            code: ErrorCode::PdfExtractFailed,
            category: ErrorCategory::Extraction,
            user_message: "PDF 提取失败".into(),
            technical_detail: Some("全文内容不应进入普通日志".into()),
            retryable: true,
            file_path: Some("/Users/example/secret/合同.pdf".into()),
            stage: Some(ProcessingStage::Extract),
        };

        diagnostics
            .log_file_stage(
                &BatchId("batch-001".into()),
                &FileJobId("file-001".into()),
                ProcessingStage::Extract,
                Some(ErrorCode::PdfExtractFailed),
                Some("/Users/example/out/劳动合同.pdf"),
            )
            .unwrap();
        diagnostics
            .log_file_error(
                &BatchId("batch-001".into()),
                &FileJobId("file-001".into()),
                &error,
            )
            .unwrap();

        let contents =
            std::fs::read_to_string(dir.path().join("logs").join("rustitler.log")).unwrap();
        assert!(contents.contains("\"fileJobId\":\"file-001\""));
        assert!(contents.contains("\"outputFileName\":\"劳动合同.pdf\""));
        assert!(contents.contains("\"errorCode\":\"pdfExtractFailed\""));
        assert!(!contents.contains("/Users/example/secret"));
        assert!(!contents.contains("全文内容不应进入普通日志"));
    }

    #[test]
    fn rotates_logs_when_file_exceeds_size_limit() {
        let dir = tempfile::tempdir().unwrap();
        let options = DiagnosticsOptions {
            max_log_bytes: 20,
            current_date: "2026-06-26".into(),
        };
        let diagnostics = Diagnostics::with_options(dir.path(), options).unwrap();

        diagnostics
            .log_batch_started(&BatchId("batch-001".into()), 1)
            .unwrap();
        diagnostics
            .log_batch_started(&BatchId("batch-002".into()), 1)
            .unwrap();

        let log_dir = dir.path().join("logs");
        let rotated_count = std::fs::read_dir(&log_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("rustitler-")
            })
            .count();

        assert!(rotated_count >= 1);
        assert!(log_dir.join("rustitler.log").exists());
    }

    #[test]
    fn rotates_logs_when_date_changes() {
        let dir = tempfile::tempdir().unwrap();
        let old_log = dir.path().join("logs").join("rustitler.log");
        std::fs::create_dir_all(old_log.parent().unwrap()).unwrap();
        std::fs::write(&old_log, "{\"date\":\"old\"}\n").unwrap();
        let rotation_date = (Local::now().date_naive() + chrono::Duration::days(1)).to_string();

        let diagnostics = Diagnostics::with_options(
            dir.path(),
            DiagnosticsOptions {
                max_log_bytes: 1024 * 1024,
                current_date: rotation_date.clone(),
            },
        )
        .unwrap();

        diagnostics
            .log_batch_started(&BatchId("batch-003".into()), 1)
            .unwrap();

        assert!(dir
            .path()
            .join("logs")
            .join(format!("rustitler-{rotation_date}-1.log"))
            .exists());
    }

    #[test]
    fn debug_save_is_skipped_when_debug_mode_is_off() {
        let dir = tempfile::tempdir().unwrap();
        let diagnostics = Diagnostics::new(dir.path()).unwrap();
        let settings = Settings {
            debug_mode: false,
            ..Settings::default()
        };

        let result = diagnostics
            .save_extracted_document(
                &settings,
                &BatchId("batch-001".into()),
                &FileJobId("file-001".into()),
                &extracted_document_fixture(),
            )
            .unwrap();

        assert!(result.is_none());
        assert!(!dir.path().join("debug").exists());
    }

    #[test]
    fn debug_save_persists_extraction_and_detail_log_with_ref() {
        let dir = tempfile::tempdir().unwrap();
        let diagnostics = Diagnostics::new(dir.path()).unwrap();
        let settings = Settings {
            debug_mode: true,
            ..Settings::default()
        };

        let extraction_ref = diagnostics
            .save_extracted_document(
                &settings,
                &BatchId("batch-001".into()),
                &FileJobId("file-001".into()),
                &extracted_document_fixture(),
            )
            .unwrap()
            .unwrap();
        let detail_ref = diagnostics
            .save_debug_detail(
                &settings,
                &BatchId("batch-001".into()),
                &FileJobId("file-001".into()),
                "ocr",
                &serde_json::json!({"rawText": "完整调试文本"}),
            )
            .unwrap()
            .unwrap();

        assert!(extraction_ref.starts_with("debug://batch-001/file-001/"));
        assert!(detail_ref.ends_with("ocr.json"));
        assert!(dir
            .path()
            .join("debug")
            .join("batch-001")
            .join("file-001")
            .exists());
    }

    #[test]
    fn clear_debug_data_removes_debug_directory() {
        let dir = tempfile::tempdir().unwrap();
        let debug_dir = dir.path().join("debug").join("batch-001");
        std::fs::create_dir_all(&debug_dir).unwrap();
        std::fs::write(debug_dir.join("x.json"), "{}").unwrap();
        let diagnostics = Diagnostics::new(dir.path()).unwrap();

        diagnostics.clear_debug_data().unwrap();

        assert!(!dir.path().join("debug").exists());
    }

    fn extracted_document_fixture() -> ExtractedDocument {
        ExtractedDocument {
            source_type: FileType::Pdf,
            extract_method: ExtractMethod::PdfNativeLiteparse,
            pages: vec![],
            paragraphs: vec![],
            diagnostics_ref: None,
        }
    }
}
