use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    UnsupportedFormat,
    FileReadFailed,
    PermissionDenied,
    PdfExtractFailed,
    PdfOcrFallbackFailed,
    OcrEngineFailed,
    DocConvertFailed,
    WordExtractFailed,
    NoTrustedTitle,
    ConfidenceBelowThreshold,
    DuplicateSuspected,
    OutputDirectoryCreateFailed,
    FileCopyFailed,
    SanitizedNameEmpty,
    UndoOutputMissing,
    UndoOutputModified,
    InvalidSettings,
    InvalidCommandArgument,
    Cancelled,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCategory {
    Input,
    Extraction,
    Scoring,
    Output,
    History,
    Settings,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProcessingStage {
    Ingest,
    Extract,
    Score,
    Rename,
    History,
    Undo,
}

#[derive(Debug, Clone, Serialize, Deserialize, Error)]
#[serde(rename_all = "camelCase")]
#[error("{user_message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub category: ErrorCategory,
    pub user_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technical_detail: Option<String>,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<ProcessingStage>,
}

impl AppError {
    pub fn internal(detail: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Internal,
            category: ErrorCategory::System,
            user_message: "内部错误，请查看诊断日志。".into(),
            technical_detail: Some(detail.into()),
            retryable: false,
            file_path: None,
            stage: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    pub fn with_stage(mut self, stage: ProcessingStage) -> Self {
        self.stage = Some(stage);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_error_serializes() {
        let e = AppError::internal("test detail");
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"code\":\"internal\""));
        assert!(json.contains("test detail"));
    }

    #[test]
    fn with_path_and_stage() {
        let e = AppError::internal("x")
            .with_path("/tmp/foo.pdf")
            .with_stage(ProcessingStage::Extract);
        assert_eq!(e.file_path.as_deref(), Some("/tmp/foo.pdf"));
        assert_eq!(e.stage, Some(ProcessingStage::Extract));
    }
}
