use crate::batch_scheduler::{
    BatchScheduler, BatchSchedulerServices, DefaultOutputCreator, EventSink, Extractor,
};
use crate::errors::{AppError, ErrorCategory, ErrorCode, ProcessingStage};
use crate::extract::ExtractRequest;
#[cfg(feature = "extraction-deps")]
use crate::extract::{self, ExtractionServices, SofficeDocConverter};
#[cfg(all(feature = "extraction-deps", not(feature = "extraction-ocr")))]
use crate::extract::{OcrExtractor, OcrPage, OcrPageInput};
use crate::history::{self, FileResultRecord, OutputKind};
use crate::models::{
    BatchEvent, BatchId, BatchState, ExtractedDocument, FileJobId, FileJobView, FileStatus,
    HistoryBatchDetail, HistoryBatchPage, Settings, UndoResult,
};
use crate::packaging::RuntimeAssets;
use crate::{rename, settings};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const MAX_HISTORY_PAGE_LIMIT: usize = 100;
const BATCH_EVENT_NAME: &str = "batch-event";

#[derive(Clone)]
pub struct AppState {
    app_data_dir: PathBuf,
    runtime_assets: Option<RuntimeAssets>,
    scheduler: BatchScheduler,
    event_emitter: Arc<dyn CommandEventEmitter>,
}

impl AppState {
    pub fn new(
        app_data_dir: impl Into<PathBuf>,
        event_emitter: Arc<dyn CommandEventEmitter>,
    ) -> Result<Self, AppError> {
        let app_data_dir = app_data_dir.into();
        history::open_history(&app_data_dir)?;
        Ok(Self {
            app_data_dir,
            runtime_assets: None,
            scheduler: BatchScheduler::default(),
            event_emitter,
        })
    }

    pub fn with_runtime_assets(mut self, runtime_assets: RuntimeAssets) -> Self {
        self.runtime_assets = Some(runtime_assets);
        self
    }

    pub fn app_data_dir(&self) -> &Path {
        &self.app_data_dir
    }

    pub fn runtime_assets(&self) -> Option<&RuntimeAssets> {
        self.runtime_assets.as_ref()
    }

    #[cfg(test)]
    fn emitted_events(&self) -> Vec<BatchEvent> {
        self.event_emitter.recorded_events()
    }
}

pub trait CommandEventEmitter: Send + Sync {
    fn emit_batch_event(&self, event: &BatchEvent) -> Result<(), AppError>;

    fn recorded_events(&self) -> Vec<BatchEvent> {
        vec![]
    }
}

pub struct TauriEventEmitter {
    app: tauri::AppHandle,
}

impl TauriEventEmitter {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl CommandEventEmitter for TauriEventEmitter {
    fn emit_batch_event(&self, event: &BatchEvent) -> Result<(), AppError> {
        use tauri::Emitter;

        self.app.emit(BATCH_EVENT_NAME, event).map_err(|err| {
            command_error(
                format!("发布批处理事件失败：{err}"),
                ProcessingStage::History,
            )
        })
    }
}

struct CommandEventSink<'a> {
    emitter: &'a dyn CommandEventEmitter,
}

impl EventSink for CommandEventSink<'_> {
    fn emit(&self, event: BatchEvent) {
        let _ = self.emitter.emit_batch_event(&event);
    }
}

#[derive(Clone, Default)]
pub struct DefaultExtractor {
    runtime_assets: Option<RuntimeAssets>,
}

impl DefaultExtractor {
    pub fn new(runtime_assets: Option<RuntimeAssets>) -> Self {
        Self { runtime_assets }
    }

    #[cfg(all(test, feature = "extraction-ocr"))]
    fn ocr_tessdata_dir_for_tests(&self) -> PathBuf {
        crate::packaging::resolve_tessdata_dir(self.runtime_assets.as_ref())
    }
}

impl Extractor for DefaultExtractor {
    fn extract(
        &self,
        request: &ExtractRequest,
        work_dir: &Path,
    ) -> Result<ExtractedDocument, AppError> {
        default_extract(request, work_dir, self.runtime_assets.as_ref())
    }
}

#[cfg(feature = "extraction-deps")]
fn default_extract(
    request: &ExtractRequest,
    work_dir: &Path,
    runtime_assets: Option<&RuntimeAssets>,
) -> Result<ExtractedDocument, AppError> {
    let docx = extract::UndocDocxTextExtractor;
    let doc_converter = SofficeDocConverter::discover_with_assets(runtime_assets);
    let pdf = extract::LiteparsePdfTextExtractor;
    let rasterizer = extract::LiteparsePdfRasterizer;
    let ocr = default_ocr_extractor(runtime_assets);
    let services = ExtractionServices {
        docx: &docx,
        doc_converter: &doc_converter,
        pdf: &pdf,
        rasterizer: &rasterizer,
        ocr: &ocr,
    };

    extract::extract_document_with_services(request, &services, work_dir)
}

#[cfg(feature = "extraction-ocr")]
fn default_ocr_extractor(runtime_assets: Option<&RuntimeAssets>) -> extract::TesseractOcrExtractor {
    extract::TesseractOcrExtractor::new(crate::packaging::resolve_tessdata_dir(runtime_assets))
}

#[cfg(all(feature = "extraction-deps", not(feature = "extraction-ocr")))]
fn default_ocr_extractor(_runtime_assets: Option<&RuntimeAssets>) -> UnavailableOcrExtractor {
    UnavailableOcrExtractor
}

#[cfg(not(feature = "extraction-deps"))]
fn default_extract(
    request: &ExtractRequest,
    _work_dir: &Path,
    _runtime_assets: Option<&RuntimeAssets>,
) -> Result<ExtractedDocument, AppError> {
    Err(AppError {
        code: ErrorCode::Internal,
        category: ErrorCategory::Extraction,
        user_message: "当前构建未启用文档提取依赖。".into(),
        technical_detail: Some("enable the extraction-deps feature for runtime extraction".into()),
        retryable: false,
        file_path: Some(request.source_path.display().to_string()),
        stage: Some(ProcessingStage::Extract),
    })
}

#[cfg(all(feature = "extraction-deps", not(feature = "extraction-ocr")))]
struct UnavailableOcrExtractor;

#[cfg(all(feature = "extraction-deps", not(feature = "extraction-ocr")))]
impl OcrExtractor for UnavailableOcrExtractor {
    fn extract_pages(&self, _pages: &[OcrPageInput]) -> Result<Vec<OcrPage>, AppError> {
        Err(AppError {
            code: ErrorCode::OcrEngineFailed,
            category: ErrorCategory::Extraction,
            user_message: "当前构建未启用 OCR 引擎。".into(),
            technical_detail: Some("OCR runtime packaging is handled by packaging-offline".into()),
            retryable: true,
            file_path: None,
            stage: Some(ProcessingStage::Extract),
        })
    }
}

#[tauri::command]
pub fn start_batch(
    state: tauri::State<'_, AppState>,
    paths: Vec<String>,
    settings_snapshot: Settings,
) -> Result<BatchId, AppError> {
    start_batch_impl(&state, paths, settings_snapshot)
}

pub fn start_batch_impl(
    state: &AppState,
    paths: Vec<String>,
    settings_snapshot: Settings,
) -> Result<BatchId, AppError> {
    if paths.is_empty() {
        return Err(command_error(
            "请选择至少一个文件或文件夹。",
            ProcessingStage::Ingest,
        ));
    }
    settings::validate_settings(&settings_snapshot)?;

    let input_paths = paths.into_iter().map(PathBuf::from).collect::<Vec<_>>();
    let history = Mutex::new(history::open_history(&state.app_data_dir)?);
    let extractor = DefaultExtractor::new(state.runtime_assets.clone());
    let output = DefaultOutputCreator;
    let events = CommandEventSink {
        emitter: state.event_emitter.as_ref(),
    };
    let services = BatchSchedulerServices {
        history: &history,
        extractor: &extractor,
        output: &output,
        events: &events,
    };
    let result =
        state
            .scheduler
            .start_batch_with_services(input_paths, settings_snapshot, &services)?;

    Ok(result.batch_id)
}

#[tauri::command]
pub fn cancel_batch(state: tauri::State<'_, AppState>, batch_id: BatchId) -> Result<(), AppError> {
    cancel_batch_impl(&state, batch_id)
}

pub fn cancel_batch_impl(state: &AppState, batch_id: BatchId) -> Result<(), AppError> {
    state.scheduler.cancel_batch(&batch_id)
}

#[tauri::command]
pub fn get_batch_state(
    state: tauri::State<'_, AppState>,
    batch_id: BatchId,
) -> Result<Option<BatchState>, AppError> {
    get_batch_state_impl(&state, batch_id)
}

pub fn get_batch_state_impl(
    state: &AppState,
    batch_id: BatchId,
) -> Result<Option<BatchState>, AppError> {
    state.scheduler.get_batch_state(&batch_id)
}

#[tauri::command]
pub fn confirm_pending_output(
    state: tauri::State<'_, AppState>,
    file_job_id: FileJobId,
    edited_name_stem: String,
) -> Result<FileJobView, AppError> {
    confirm_pending_output_impl(&state, file_job_id, edited_name_stem)
}

pub fn confirm_pending_output_impl(
    state: &AppState,
    file_job_id: FileJobId,
    edited_name_stem: impl AsRef<str>,
) -> Result<FileJobView, AppError> {
    let conn = history::open_history(&state.app_data_dir)?;
    let existing = history::get_history_file_result(&conn, &file_job_id)?
        .ok_or_else(|| command_error("找不到待确认的文件记录。", ProcessingStage::History))?;
    let edited_name_stem = validate_edited_name_stem(
        edited_name_stem.as_ref(),
        Path::new(&existing.file.source_path),
    )?;

    if existing.file.status != FileStatus::Pending {
        return Err(command_error(
            "只能确认待处理状态的文件。",
            ProcessingStage::Rename,
        ));
    }

    let output_path = rename::create_manual_output_copy(
        Path::new(&existing.file.source_path),
        &edited_name_stem,
    )?;
    let mut updated = existing.file.clone();
    updated.status = FileStatus::OutputCreated;
    updated.recognized_title = Some(edited_name_stem);
    updated.output_path = Some(output_path.display().to_string());
    updated.failure_reason = None;
    updated.pending_reason = None;

    history::record_file_result(
        &conn,
        &FileResultRecord {
            file: updated.clone(),
            source_fingerprint: existing.source_fingerprint,
            scoring_result: existing.scoring_result,
            error: None,
            output_kind: Some(OutputKind::Manual),
        },
    )?;
    history::record_undo_for_output(&conn, &file_job_id, &output_path)?;
    history::refresh_batch_summary(&conn, &updated.batch_id)?;

    state
        .event_emitter
        .emit_batch_event(&BatchEvent::FileOutputCreated {
            batch_id: updated.batch_id.clone(),
            file_job_id,
            output_path: output_path.display().to_string(),
        })?;

    Ok(updated)
}

fn validate_edited_name_stem(input: &str, source_path: &Path) -> Result<String, AppError> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return Err(command_error(
            "文件名主体不能为空。",
            ProcessingStage::Rename,
        ));
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err(command_error(
            "文件名主体不能包含路径分隔符。",
            ProcessingStage::Rename,
        ));
    }
    if Path::new(trimmed)
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(command_error(
            "文件名主体只能包含文件名，不能包含路径。",
            ProcessingStage::Rename,
        ));
    }
    let source_extension = source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    let edited_extension = Path::new(trimmed)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    if source_extension.is_some() && edited_extension == source_extension {
        return Err(command_error(
            "文件名主体不能包含扩展名。",
            ProcessingStage::Rename,
        ));
    }

    Ok(trimmed.to_string())
}

#[tauri::command]
pub fn undo_batch(
    state: tauri::State<'_, AppState>,
    batch_id: BatchId,
) -> Result<UndoResult, AppError> {
    undo_batch_impl(&state, batch_id)
}

pub fn undo_batch_impl(state: &AppState, batch_id: BatchId) -> Result<UndoResult, AppError> {
    let conn = history::open_history(&state.app_data_dir)?;
    history::undo_batch_outputs(&conn, &batch_id)
}

#[tauri::command]
pub fn list_history(
    state: tauri::State<'_, AppState>,
    offset: usize,
    limit: usize,
) -> Result<HistoryBatchPage, AppError> {
    list_history_impl(&state, offset, limit)
}

pub fn list_history_impl(
    state: &AppState,
    offset: usize,
    limit: usize,
) -> Result<HistoryBatchPage, AppError> {
    if limit == 0 || limit > MAX_HISTORY_PAGE_LIMIT {
        return Err(command_error(
            format!("分页 limit 必须在 1-{MAX_HISTORY_PAGE_LIMIT} 之间。"),
            ProcessingStage::History,
        ));
    }
    let conn = history::open_history(&state.app_data_dir)?;
    history::list_history(&conn, offset, limit)
}

#[tauri::command]
pub fn get_history_batch(
    state: tauri::State<'_, AppState>,
    batch_id: BatchId,
) -> Result<Option<HistoryBatchDetail>, AppError> {
    get_history_batch_impl(&state, batch_id)
}

pub fn get_history_batch_impl(
    state: &AppState,
    batch_id: BatchId,
) -> Result<Option<HistoryBatchDetail>, AppError> {
    let conn = history::open_history(&state.app_data_dir)?;
    history::get_history_batch(&conn, &batch_id)
}

#[tauri::command]
pub fn load_settings(state: tauri::State<'_, AppState>) -> Result<Settings, AppError> {
    load_settings_impl(&state)
}

pub fn load_settings_impl(state: &AppState) -> Result<Settings, AppError> {
    settings::load_settings(&state.app_data_dir)
}

#[tauri::command]
pub fn save_settings(
    state: tauri::State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, AppError> {
    save_settings_impl(&state, settings)
}

pub fn save_settings_impl(state: &AppState, settings: Settings) -> Result<Settings, AppError> {
    settings::save_settings(&state.app_data_dir, &settings)
}

#[tauri::command]
pub fn import_settings(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<Settings, AppError> {
    import_settings_impl(&state, path)
}

pub fn import_settings_impl(_state: &AppState, path: String) -> Result<Settings, AppError> {
    settings::import_settings(path)
}

#[tauri::command]
pub fn export_settings(state: tauri::State<'_, AppState>, path: String) -> Result<(), AppError> {
    export_settings_impl(&state, path)
}

pub fn export_settings_impl(state: &AppState, path: String) -> Result<(), AppError> {
    let current = settings::load_settings(&state.app_data_dir)?;
    settings::export_settings(&current, path)
}

#[tauri::command]
pub fn reset_settings(state: tauri::State<'_, AppState>) -> Result<Settings, AppError> {
    reset_settings_impl(&state)
}

pub fn reset_settings_impl(state: &AppState) -> Result<Settings, AppError> {
    settings::reset_settings(&state.app_data_dir)
}

fn command_error(message: impl Into<String>, stage: ProcessingStage) -> AppError {
    AppError {
        code: ErrorCode::InvalidCommandArgument,
        category: ErrorCategory::Input,
        user_message: message.into(),
        technical_detail: None,
        retryable: false,
        file_path: None,
        stage: Some(stage),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::{ErrorCategory, ErrorCode};
    use crate::history::{BatchRecord, FileResultRecord};
    use crate::models::{
        BatchStatus, BatchSummary, FileStatus, FileType, PendingReason, SourceFingerprint,
    };
    use crate::settings::create_settings_snapshot;
    use std::fs;

    #[test]
    fn start_batch_rejects_empty_paths_before_work_starts() {
        let state = test_state();

        let error = start_batch_impl(&state, vec![], Settings::default()).unwrap_err();

        assert_eq!(error.code, ErrorCode::InvalidCommandArgument);
        assert_eq!(error.stage, Some(ProcessingStage::Ingest));
        assert!(state.emitted_events().is_empty());
    }

    #[test]
    fn start_batch_processes_unsupported_file_and_exposes_state_history_and_events() {
        let state = test_state();
        let source = state.app_data_dir.join("input").join("notes.txt");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"plain text").unwrap();

        let batch_id = start_batch_impl(
            &state,
            vec![source.display().to_string()],
            Settings::default(),
        )
        .unwrap();

        let state_snapshot = get_batch_state_impl(&state, batch_id.clone())
            .unwrap()
            .expect("batch state should be retained");
        assert_eq!(state_snapshot.summary.total, 1);
        assert_eq!(state_snapshot.summary.skipped, 1);

        let history_page = list_history_impl(&state, 0, 10).unwrap();
        assert_eq!(history_page.total, 1);
        let detail = get_history_batch_impl(&state, batch_id.clone())
            .unwrap()
            .expect("history detail should exist");
        assert_eq!(detail.files[0].file.status, FileStatus::Skipped);

        let events = state.emitted_events();
        assert!(matches!(
            &events[0],
            BatchEvent::BatchStarted {
                batch_id: event_batch_id,
                total_files: 1,
                ..
            } if event_batch_id == &batch_id
        ));
        assert!(events.iter().any(|event| {
            matches!(
                event,
                BatchEvent::FileSkipped {
                    batch_id: event_batch_id,
                    file_job_id,
                    ..
                } if event_batch_id == &batch_id && !file_job_id.0.is_empty()
            )
        }));
        assert!(matches!(
            events.last().unwrap(),
            BatchEvent::BatchCompleted {
                batch_id: event_batch_id,
                ..
            } if event_batch_id == &batch_id
        ));
    }

    #[test]
    fn list_history_validates_pagination_arguments() {
        let state = test_state();

        let zero_limit = list_history_impl(&state, 0, 0).unwrap_err();
        let too_large = list_history_impl(&state, 0, MAX_HISTORY_PAGE_LIMIT + 1).unwrap_err();

        assert_eq!(zero_limit.code, ErrorCode::InvalidCommandArgument);
        assert_eq!(too_large.code, ErrorCode::InvalidCommandArgument);
    }

    #[test]
    fn edited_name_stem_must_be_only_a_file_name_stem() {
        let source = Path::new("/input/old.pdf");
        assert_eq!(
            validate_edited_name_stem(" 手动标题 ", source).unwrap(),
            "手动标题"
        );
        assert_eq!(
            validate_edited_name_stem("3.15会议纪要", source).unwrap(),
            "3.15会议纪要"
        );

        for invalid in [
            "",
            "   ",
            "../标题",
            "nested/标题",
            "nested\\标题",
            ".",
            "..",
            "标题.pdf",
        ] {
            let error = validate_edited_name_stem(invalid, source).unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidCommandArgument);
            assert_eq!(error.stage, Some(ProcessingStage::Rename));
        }
    }

    #[test]
    fn confirm_pending_output_copies_file_updates_history_and_emits_event() {
        let state = test_state();
        let source = state.app_data_dir.join("input").join("old.pdf");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"document").unwrap();
        let batch_id = BatchId("batch-manual".into());
        let file_job_id = FileJobId("file-manual".into());
        seed_pending_history(&state, &batch_id, &file_job_id, &source);

        let updated =
            confirm_pending_output_impl(&state, file_job_id.clone(), " 手动|标题 ").unwrap();

        assert_eq!(updated.status, FileStatus::OutputCreated);
        assert_eq!(updated.recognized_title.as_deref(), Some("手动|标题"));
        let output_path = updated.output_path.clone().unwrap();
        assert_eq!(fs::read(&output_path).unwrap(), b"document");
        assert!(output_path.ends_with("手动_标题.pdf"));

        let conn = history::open_history(&state.app_data_dir).unwrap();
        let duplicate = history::find_duplicate_by_fingerprint(&conn, &fingerprint_for(&source))
            .unwrap()
            .unwrap();
        assert_eq!(duplicate.file_job_id, file_job_id);
        let detail = history::get_history_batch(&conn, &batch_id)
            .unwrap()
            .unwrap();
        assert_eq!(detail.summary.output_created, 1);
        assert_eq!(detail.summary.pending, 0);

        let undo = undo_batch_impl(&state, batch_id.clone()).unwrap();
        assert_eq!(undo.deleted, 1);
        assert!(!std::path::Path::new(&output_path).exists());

        let events = state.emitted_events();
        assert!(matches!(
            &events[0],
            BatchEvent::FileOutputCreated {
                batch_id: event_batch_id,
                file_job_id: event_file_job_id,
                output_path: event_output_path,
            } if event_batch_id == &batch_id
                && event_file_job_id == &FileJobId("file-manual".into())
                && event_output_path == &output_path
        ));
    }

    #[test]
    fn settings_commands_reuse_settings_validation_and_io() {
        let state = test_state();
        let imported_path = state.app_data_dir.join("import.json");
        let exported_path = state.app_data_dir.join("export").join("settings.json");
        let settings = Settings {
            auto_output_threshold: 63,
            ..Settings::default()
        };
        fs::write(
            &imported_path,
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let imported = import_settings_impl(&state, imported_path.display().to_string()).unwrap();
        assert_eq!(imported.auto_output_threshold, 63);

        let saved = save_settings_impl(&state, imported).unwrap();
        assert_eq!(saved.auto_output_threshold, 63);
        assert_eq!(
            load_settings_impl(&state).unwrap().auto_output_threshold,
            63
        );

        export_settings_impl(&state, exported_path.display().to_string()).unwrap();
        assert!(exported_path.exists());

        let reset = reset_settings_impl(&state).unwrap();
        assert_eq!(
            reset.auto_output_threshold,
            Settings::default().auto_output_threshold
        );
    }

    #[test]
    fn save_settings_returns_structured_validation_error() {
        let state = test_state();
        let invalid = Settings {
            auto_output_threshold: 101,
            ..Settings::default()
        };

        let error = save_settings_impl(&state, invalid).unwrap_err();

        assert_eq!(error.code, ErrorCode::InvalidSettings);
        assert_eq!(error.category, ErrorCategory::Settings);
    }

    #[cfg(feature = "extraction-ocr")]
    #[test]
    fn default_extractor_uses_tesseract_ocr_with_runtime_assets() {
        let assets = RuntimeAssets::new("/bundle/resources");
        let extractor = DefaultExtractor::new(Some(assets.clone()));

        assert_eq!(
            extractor.ocr_tessdata_dir_for_tests(),
            assets.resource_dir().join("tessdata")
        );
    }

    fn test_state() -> AppState {
        let temp_dir = tempfile::tempdir().unwrap();
        let app_data_dir = temp_dir.keep();
        AppState::new(app_data_dir, Arc::new(RecordingEventEmitter::default())).unwrap()
    }

    fn seed_pending_history(
        state: &AppState,
        batch_id: &BatchId,
        file_job_id: &FileJobId,
        source: &std::path::Path,
    ) {
        let conn = history::open_history(&state.app_data_dir).unwrap();
        let snapshot = create_settings_snapshot(&Settings::default());
        history::save_settings_snapshot(&conn, &snapshot).unwrap();
        history::create_batch(
            &conn,
            &BatchRecord {
                batch_id: batch_id.clone(),
                created_at: "2026-06-27T10:00:00Z".into(),
                status: BatchStatus::Completed,
                settings_snapshot_id: snapshot.id,
                summary: BatchSummary {
                    total: 1,
                    output_created: 0,
                    pending: 1,
                    skipped: 0,
                    failed: 0,
                    cancelled: 0,
                },
            },
        )
        .unwrap();
        history::record_file_result(
            &conn,
            &FileResultRecord {
                file: FileJobView {
                    file_job_id: file_job_id.clone(),
                    batch_id: batch_id.clone(),
                    source_path: source.display().to_string(),
                    file_name: "old.pdf".into(),
                    file_type: FileType::Pdf,
                    status: FileStatus::Pending,
                    recognized_title: Some("旧建议".into()),
                    confidence: Some(41),
                    output_path: None,
                    failure_reason: Some("低置信度".into()),
                    pending_reason: Some(PendingReason::LowConfidence),
                },
                source_fingerprint: fingerprint_for(source),
                scoring_result: None,
                error: None,
                output_kind: None,
            },
        )
        .unwrap();
    }

    fn fingerprint_for(path: &std::path::Path) -> SourceFingerprint {
        SourceFingerprint {
            normalized_path: path.display().to_string(),
            size_bytes: fs::metadata(path).unwrap().len(),
            modified_time: "2026-06-27T10:00:00Z".into(),
        }
    }

    #[derive(Default)]
    struct RecordingEventEmitter {
        events: Mutex<Vec<BatchEvent>>,
    }

    impl CommandEventEmitter for RecordingEventEmitter {
        fn emit_batch_event(&self, event: &BatchEvent) -> Result<(), AppError> {
            self.events.lock().unwrap().push(event.clone());
            Ok(())
        }

        fn recorded_events(&self) -> Vec<BatchEvent> {
            self.events.lock().unwrap().clone()
        }
    }
}
