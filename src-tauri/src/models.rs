use crate::errors::{AppError, ProcessingStage};
use serde::{Deserialize, Serialize};

// ── Typed IDs ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BatchId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileJobId(pub String);

// ── File classification ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileType {
    Docx,
    Doc,
    Pdf,
    Png,
    Jpg,
    Jpeg,
    Unsupported,
}

impl FileType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_ascii_lowercase().as_str() {
            "docx" => Self::Docx,
            "doc" => Self::Doc,
            "pdf" => Self::Pdf,
            "png" => Self::Png,
            "jpg" => Self::Jpg,
            "jpeg" => Self::Jpeg,
            _ => Self::Unsupported,
        }
    }

    pub fn is_supported(&self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
    Queued,
    Analyzing,
    OutputCreated,
    Pending,
    Skipped,
    Failed,
    Undoable,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
}

// ── Source fingerprint ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFingerprint {
    pub normalized_path: String,
    pub size_bytes: u64,
    pub modified_time: String, // ISO-8601 UTC
}

// ── Extraction types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtractMethod {
    PdfNativeLiteparse,
    WordUndoc,
    DocConvertedUndoc,
    ImageOcrTesseract,
    PdfOcrFallbackTesseract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceUnit {
    PdfPoint,
    Pixel,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutBlock {
    pub text: String,
    pub bbox: NormalizedBox,
    pub raw_bbox: Option<RawBox>,
    pub font_size: Option<f32>,
    pub bold: Option<bool>,
    pub ocr_confidence: Option<f32>,
    pub line_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedPage {
    pub page_index: usize,
    pub width: f32,
    pub height: f32,
    pub unit: SourceUnit,
    pub blocks: Vec<LayoutBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParagraphBlock {
    pub text: String,
    pub paragraph_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedDocument {
    pub source_type: FileType,
    pub extract_method: ExtractMethod,
    pub pages: Vec<ExtractedPage>,
    pub paragraphs: Vec<ParagraphBlock>,
    pub diagnostics_ref: Option<String>,
}

// ── Scoring types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeywordRule {
    pub keyword: String,
    pub score_delta: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegexRule {
    pub pattern: String,
    pub score_delta: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoringProfile {
    pub auto_output_threshold: u8,
    pub layout_sensitivity: f32,
    pub position_sensitivity: f32,
    pub keyword_sensitivity: f32,
    pub text_quality_sensitivity: f32,
    pub ocr_conservatism: f32,
    pub keyword_rules: Vec<KeywordRule>,
    pub regex_rules: Vec<RegexRule>,
}

impl Default for ScoringProfile {
    fn default() -> Self {
        Self {
            auto_output_threshold: 70,
            layout_sensitivity: 1.0,
            position_sensitivity: 1.0,
            keyword_sensitivity: 1.0,
            text_quality_sensitivity: 1.0,
            ocr_conservatism: 1.0,
            keyword_rules: vec![
                KeywordRule {
                    keyword: "关于".into(),
                    score_delta: 5,
                },
                KeywordRule {
                    keyword: "通知".into(),
                    score_delta: 5,
                },
                KeywordRule {
                    keyword: "报告".into(),
                    score_delta: 5,
                },
                KeywordRule {
                    keyword: "方案".into(),
                    score_delta: 5,
                },
                KeywordRule {
                    keyword: "制度".into(),
                    score_delta: 5,
                },
                KeywordRule {
                    keyword: "合同".into(),
                    score_delta: 5,
                },
                KeywordRule {
                    keyword: "函".into(),
                    score_delta: 3,
                },
            ],
            regex_rules: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CandidateSource {
    PdfLayout,
    WordParagraph,
    OcrBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryScores {
    pub layout: i16,
    pub position: i16,
    pub keyword: i16,
    pub text_quality: i16,
    pub penalty: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleDetail {
    pub rule_name: String,
    pub category: String,
    pub delta: i16,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateTitle {
    pub text: String,
    pub source: CandidateSource,
    pub page_index: Option<usize>,
    pub paragraph_index: Option<usize>,
    pub score: u8,
    pub category_scores: CategoryScores,
    pub rule_details: Vec<RuleDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScoreDecision {
    AutoOutput,
    Pending,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoringResult {
    pub final_title: Option<String>,
    pub confidence: u8,
    pub candidates: Vec<CandidateTitle>,
    pub decision: ScoreDecision,
}

// ── Settings types ─────────────────────────────────────────────────────────

pub const SETTINGS_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub version: u16,
    pub auto_output_threshold: u8,
    pub layout_sensitivity: f32,
    pub position_sensitivity: f32,
    pub keyword_sensitivity: f32,
    pub text_quality_sensitivity: f32,
    pub ocr_conservatism: f32,
    pub keyword_rules: Vec<KeywordRule>,
    pub regex_rules: Vec<RegexRule>,
    pub debug_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let profile = ScoringProfile::default();

        Self {
            version: SETTINGS_VERSION,
            auto_output_threshold: profile.auto_output_threshold,
            layout_sensitivity: profile.layout_sensitivity,
            position_sensitivity: profile.position_sensitivity,
            keyword_sensitivity: profile.keyword_sensitivity,
            text_quality_sensitivity: profile.text_quality_sensitivity,
            ocr_conservatism: profile.ocr_conservatism,
            keyword_rules: profile.keyword_rules,
            regex_rules: profile.regex_rules,
            debug_mode: false,
        }
    }
}

impl From<&Settings> for ScoringProfile {
    fn from(settings: &Settings) -> Self {
        Self {
            auto_output_threshold: settings.auto_output_threshold,
            layout_sensitivity: settings.layout_sensitivity,
            position_sensitivity: settings.position_sensitivity,
            keyword_sensitivity: settings.keyword_sensitivity,
            text_quality_sensitivity: settings.text_quality_sensitivity,
            ocr_conservatism: settings.ocr_conservatism,
            keyword_rules: settings.keyword_rules.clone(),
            regex_rules: settings.regex_rules.clone(),
        }
    }
}

// ── IPC view types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PendingReason {
    LowConfidence,
    ExtractionFailed,
    OcrFailed,
    DocConvertFailed,
    UnsupportedFormat,
    DuplicateSuspected,
    IoError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileJobView {
    pub file_job_id: FileJobId,
    pub batch_id: BatchId,
    pub source_path: String,
    pub file_name: String,
    pub file_type: FileType,
    pub status: FileStatus,
    pub recognized_title: Option<String>,
    pub confidence: Option<u8>,
    pub output_path: Option<String>,
    pub failure_reason: Option<String>,
    pub pending_reason: Option<PendingReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileJob {
    pub file_job_id: FileJobId,
    pub batch_id: BatchId,
    pub source_path: String,
    pub source_parent_path: Option<String>,
    pub file_name: String,
    pub file_type: FileType,
    pub status: FileStatus,
    pub fingerprint: SourceFingerprint,
    pub recognized_title: Option<String>,
    pub confidence: Option<u8>,
    pub output_path: Option<String>,
    pub failure_reason: Option<String>,
    pub pending_reason: Option<PendingReason>,
}

impl From<&FileJob> for FileJobView {
    fn from(job: &FileJob) -> Self {
        Self {
            file_job_id: job.file_job_id.clone(),
            batch_id: job.batch_id.clone(),
            source_path: job.source_path.clone(),
            file_name: job.file_name.clone(),
            file_type: job.file_type.clone(),
            status: job.status.clone(),
            recognized_title: job.recognized_title.clone(),
            confidence: job.confidence,
            output_path: job.output_path.clone(),
            failure_reason: job.failure_reason.clone(),
            pending_reason: job.pending_reason.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSummary {
    pub total: usize,
    pub output_created: usize,
    pub pending: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchState {
    pub batch_id: BatchId,
    pub created_at: String,
    pub status: BatchStatus,
    pub settings_snapshot_id: String,
    pub files: Vec<FileJobView>,
    pub summary: BatchSummary,
}

// ── History types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryBatchRow {
    pub batch_id: BatchId,
    pub created_at: String,
    pub status: BatchStatus,
    pub settings_snapshot_id: String,
    pub summary: BatchSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryBatchPage {
    pub batches: Vec<HistoryBatchRow>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFileResult {
    pub file: FileJobView,
    pub source_fingerprint: SourceFingerprint,
    pub scoring_result: Option<ScoringResult>,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryBatchDetail {
    pub batch_id: BatchId,
    pub created_at: String,
    pub status: BatchStatus,
    pub settings_snapshot_id: String,
    pub summary: BatchSummary,
    pub files: Vec<HistoryFileResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoResult {
    pub deleted: usize,
    pub skipped_missing: usize,
    pub skipped_modified: usize,
}

// ── Events ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BatchEvent {
    BatchStarted {
        #[serde(rename = "batchId")]
        batch_id: BatchId,
        #[serde(rename = "createdAt")]
        created_at: String,
        #[serde(rename = "totalFiles")]
        total_files: usize,
    },
    FileQueued {
        #[serde(rename = "batchId")]
        batch_id: BatchId,
        file: FileJobView,
    },
    FileProgress {
        #[serde(rename = "batchId")]
        batch_id: BatchId,
        #[serde(rename = "fileJobId")]
        file_job_id: FileJobId,
        stage: ProcessingStage,
        #[serde(skip_serializing_if = "Option::is_none")]
        progress: Option<f32>,
    },
    FileExtracted {
        #[serde(rename = "batchId")]
        batch_id: BatchId,
        #[serde(rename = "fileJobId")]
        file_job_id: FileJobId,
        #[serde(rename = "extractMethod")]
        extract_method: ExtractMethod,
    },
    FileScored {
        #[serde(rename = "batchId")]
        batch_id: BatchId,
        #[serde(rename = "fileJobId")]
        file_job_id: FileJobId,
        result: ScoringResult,
    },
    FileOutputCreated {
        #[serde(rename = "batchId")]
        batch_id: BatchId,
        #[serde(rename = "fileJobId")]
        file_job_id: FileJobId,
        #[serde(rename = "outputPath")]
        output_path: String,
    },
    FilePending {
        #[serde(rename = "batchId")]
        batch_id: BatchId,
        #[serde(rename = "fileJobId")]
        file_job_id: FileJobId,
        reason: PendingReason,
        #[serde(skip_serializing_if = "Option::is_none")]
        suggestion: Option<String>,
    },
    FileSkipped {
        #[serde(rename = "batchId")]
        batch_id: BatchId,
        #[serde(rename = "fileJobId")]
        file_job_id: FileJobId,
        reason: String,
    },
    FileFailed {
        #[serde(rename = "batchId")]
        batch_id: BatchId,
        #[serde(rename = "fileJobId")]
        file_job_id: FileJobId,
        error: AppError,
    },
    BatchCompleted {
        #[serde(rename = "batchId")]
        batch_id: BatchId,
        summary: BatchSummary,
    },
    BatchCancelled {
        #[serde(rename = "batchId")]
        batch_id: BatchId,
        summary: BatchSummary,
    },
    BatchError {
        #[serde(rename = "batchId")]
        batch_id: BatchId,
        error: AppError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::{AppError, ErrorCategory, ErrorCode};
    use serde_json::json;

    #[test]
    fn file_type_from_extension() {
        assert_eq!(FileType::from_extension("pdf"), FileType::Pdf);
        assert_eq!(FileType::from_extension("PDF"), FileType::Pdf);
        assert_eq!(FileType::from_extension("docx"), FileType::Docx);
        assert_eq!(FileType::from_extension("doc"), FileType::Doc);
        assert_eq!(FileType::from_extension("png"), FileType::Png);
        assert_eq!(FileType::from_extension("jpg"), FileType::Jpg);
        assert_eq!(FileType::from_extension("jpeg"), FileType::Jpeg);
        assert_eq!(FileType::from_extension("xlsx"), FileType::Unsupported);
        assert!(FileType::Pdf.is_supported());
        assert!(!FileType::Unsupported.is_supported());
    }

    #[test]
    fn scoring_profile_defaults() {
        let p = ScoringProfile::default();
        assert_eq!(p.auto_output_threshold, 70);
        assert!(p.keyword_rules.iter().any(|r| r.keyword == "通知"));
    }

    #[test]
    fn models_serialize_round_trip() {
        let id = BatchId("batch-001".into());
        let json = serde_json::to_string(&id).unwrap();
        let back: BatchId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn ipc_structs_serialize_with_camel_case_fields() {
        let view = FileJobView {
            file_job_id: FileJobId("file-001".into()),
            batch_id: BatchId("batch-001".into()),
            source_path: "/input/合同.pdf".into(),
            file_name: "合同.pdf".into(),
            file_type: FileType::Pdf,
            status: FileStatus::Queued,
            recognized_title: Some("劳动合同".into()),
            confidence: Some(88),
            output_path: None,
            failure_reason: None,
            pending_reason: None,
        };

        let value = serde_json::to_value(view).unwrap();

        assert_eq!(value["fileJobId"], "file-001");
        assert_eq!(value["batchId"], "batch-001");
        assert_eq!(value["sourcePath"], "/input/合同.pdf");
        assert_eq!(value["fileType"], "pdf");
        assert_eq!(value["recognizedTitle"], "劳动合同");
        assert!(value.get("file_job_id").is_none());
    }

    #[test]
    fn settings_default_serialization_snapshot() {
        let settings = Settings::default();
        let value = serde_json::to_value(settings).unwrap();

        assert_eq!(
            value,
            json!({
                "version": 1,
                "autoOutputThreshold": 70,
                "layoutSensitivity": 1.0,
                "positionSensitivity": 1.0,
                "keywordSensitivity": 1.0,
                "textQualitySensitivity": 1.0,
                "ocrConservatism": 1.0,
                "keywordRules": [
                    { "keyword": "关于", "scoreDelta": 5 },
                    { "keyword": "通知", "scoreDelta": 5 },
                    { "keyword": "报告", "scoreDelta": 5 },
                    { "keyword": "方案", "scoreDelta": 5 },
                    { "keyword": "制度", "scoreDelta": 5 },
                    { "keyword": "合同", "scoreDelta": 5 },
                    { "keyword": "函", "scoreDelta": 3 }
                ],
                "regexRules": [],
                "debugMode": false
            })
        );
    }

    #[test]
    fn batch_event_serialization_snapshot() {
        let event = BatchEvent::FileScored {
            batch_id: BatchId("batch-001".into()),
            file_job_id: FileJobId("file-001".into()),
            result: scoring_result_fixture(),
        };

        let value = serde_json::to_value(event).unwrap();

        assert_eq!(value["type"], "FileScored");
        assert_eq!(value["batchId"], "batch-001");
        assert_eq!(value["fileJobId"], "file-001");
        assert_eq!(value["result"]["finalTitle"], "劳动合同");
        assert_eq!(
            value["result"]["candidates"][0]["categoryScores"]["textQuality"],
            12
        );
        assert!(value.get("file_job_id").is_none());
    }

    #[test]
    fn batch_state_and_history_round_trip() {
        let batch_id = BatchId("batch-001".into());
        let file_job_id = FileJobId("file-001".into());
        let file = file_job_view_fixture(batch_id.clone(), file_job_id.clone());
        let summary = BatchSummary {
            total: 1,
            output_created: 1,
            pending: 0,
            skipped: 0,
            failed: 0,
            cancelled: 0,
        };
        let state = BatchState {
            batch_id: batch_id.clone(),
            created_at: "2026-06-26T10:00:00Z".into(),
            status: BatchStatus::Completed,
            settings_snapshot_id: "settings-001".into(),
            files: vec![file.clone()],
            summary: summary.clone(),
        };
        let detail = HistoryBatchDetail {
            batch_id: batch_id.clone(),
            created_at: "2026-06-26T10:00:00Z".into(),
            status: BatchStatus::Completed,
            settings_snapshot_id: "settings-001".into(),
            summary,
            files: vec![HistoryFileResult {
                file,
                source_fingerprint: SourceFingerprint {
                    normalized_path: "/input/合同.pdf".into(),
                    size_bytes: 2048,
                    modified_time: "2026-06-26T09:00:00Z".into(),
                },
                scoring_result: Some(scoring_result_fixture()),
                error: None,
            }],
        };

        let state_json = serde_json::to_string(&state).unwrap();
        let detail_json = serde_json::to_string(&detail).unwrap();
        let state_back: BatchState = serde_json::from_str(&state_json).unwrap();
        let detail_back: HistoryBatchDetail = serde_json::from_str(&detail_json).unwrap();

        assert_eq!(state_back.batch_id, batch_id);
        assert_eq!(detail_back.files[0].source_fingerprint.size_bytes, 2048);
    }

    #[test]
    fn error_serialization_matches_ipc_shape() {
        let error = AppError {
            code: ErrorCode::PdfExtractFailed,
            category: ErrorCategory::Extraction,
            user_message: "PDF 提取失败".into(),
            technical_detail: Some("liteparse error".into()),
            retryable: true,
            file_path: Some("/input/a.pdf".into()),
            stage: Some(ProcessingStage::Extract),
        };

        let value = serde_json::to_value(error).unwrap();

        assert_eq!(value["code"], "pdfExtractFailed");
        assert_eq!(value["userMessage"], "PDF 提取失败");
        assert_eq!(value["technicalDetail"], "liteparse error");
        assert_eq!(value["filePath"], "/input/a.pdf");
        assert_eq!(value["stage"], "extract");
        assert!(value.get("user_message").is_none());
    }

    fn file_job_view_fixture(batch_id: BatchId, file_job_id: FileJobId) -> FileJobView {
        FileJobView {
            file_job_id,
            batch_id,
            source_path: "/input/合同.pdf".into(),
            file_name: "合同.pdf".into(),
            file_type: FileType::Pdf,
            status: FileStatus::OutputCreated,
            recognized_title: Some("劳动合同".into()),
            confidence: Some(91),
            output_path: Some("/input/Rustitler 输出/劳动合同.pdf".into()),
            failure_reason: None,
            pending_reason: None,
        }
    }

    fn scoring_result_fixture() -> ScoringResult {
        ScoringResult {
            final_title: Some("劳动合同".into()),
            confidence: 91,
            candidates: vec![CandidateTitle {
                text: "劳动合同".into(),
                source: CandidateSource::PdfLayout,
                page_index: Some(0),
                paragraph_index: None,
                score: 91,
                category_scores: CategoryScores {
                    layout: 30,
                    position: 25,
                    keyword: 8,
                    text_quality: 12,
                    penalty: -2,
                },
                rule_details: vec![RuleDetail {
                    rule_name: "keyword-default".into(),
                    category: "keyword".into(),
                    delta: 8,
                    description: "命中默认关键词".into(),
                }],
            }],
            decision: ScoreDecision::AutoOutput,
        }
    }
}
