use crate::errors::{AppError, ErrorCategory, ErrorCode, ProcessingStage};
use crate::extract::{BatchTempDir, ExtractRequest};
use crate::history::{self, BatchRecord, DuplicateMatch, FileResultRecord, OutputKind};
use crate::models::{
    BatchEvent, BatchId, BatchState, BatchStatus, BatchSummary, ExtractedDocument, FileJob,
    FileJobId, FileJobView, FileStatus, FileType, PendingReason, ScoreDecision, ScoringProfile,
    ScoringResult, Settings, SourceFingerprint,
};
use crate::{ingest, rename, scoring, settings};
use chrono::Utc;
use rusqlite::Connection;
use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use uuid::Uuid;

#[derive(Clone)]
pub struct BatchScheduler {
    inner: Arc<Mutex<HashMap<String, RuntimeBatch>>>,
    config: BatchSchedulerConfig,
}

impl Default for BatchScheduler {
    fn default() -> Self {
        Self::with_config(BatchSchedulerConfig::default())
    }
}

#[derive(Debug, Clone)]
pub struct BatchSchedulerConfig {
    pub max_parallel_files: usize,
    pub max_parallel_ocr: usize,
    pub max_parallel_doc_conversion: usize,
}

impl Default for BatchSchedulerConfig {
    fn default() -> Self {
        Self {
            max_parallel_files: 4,
            max_parallel_ocr: 1,
            max_parallel_doc_conversion: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatchRunResult {
    pub batch_id: BatchId,
    pub summary: BatchSummary,
}

pub struct BatchSchedulerServices<'a> {
    pub history: &'a dyn HistoryStore,
    pub extractor: &'a dyn Extractor,
    pub output: &'a dyn OutputCreator,
    pub events: &'a dyn EventSink,
}

pub trait HistoryStore: Send + Sync {
    fn find_duplicate(
        &self,
        fingerprint: &SourceFingerprint,
    ) -> Result<Option<DuplicateMatch>, AppError>;

    fn save_settings_snapshot(&self, snapshot: &settings::SettingsSnapshot)
        -> Result<(), AppError>;

    fn create_batch(&self, record: &BatchRecord) -> Result<(), AppError>;

    fn record_file_result(&self, record: &FileResultRecord) -> Result<(), AppError>;

    fn record_undo_for_output(
        &self,
        file_job_id: &FileJobId,
        output_path: &Path,
    ) -> Result<(), AppError>;
}

impl HistoryStore for Mutex<Connection> {
    fn find_duplicate(
        &self,
        fingerprint: &SourceFingerprint,
    ) -> Result<Option<DuplicateMatch>, AppError> {
        history::find_duplicate_by_fingerprint(&self.lock().unwrap(), fingerprint)
    }

    fn save_settings_snapshot(
        &self,
        snapshot: &settings::SettingsSnapshot,
    ) -> Result<(), AppError> {
        history::save_settings_snapshot(&self.lock().unwrap(), snapshot)
    }

    fn create_batch(&self, record: &BatchRecord) -> Result<(), AppError> {
        history::create_batch(&self.lock().unwrap(), record)
    }

    fn record_file_result(&self, record: &FileResultRecord) -> Result<(), AppError> {
        history::record_file_result(&self.lock().unwrap(), record)
    }

    fn record_undo_for_output(
        &self,
        file_job_id: &FileJobId,
        output_path: &Path,
    ) -> Result<(), AppError> {
        history::record_undo_for_output(&self.lock().unwrap(), file_job_id, output_path)
    }
}

pub trait Extractor: Send + Sync {
    fn extract(
        &self,
        request: &ExtractRequest,
        work_dir: &Path,
    ) -> Result<ExtractedDocument, AppError>;
}

pub trait OutputCreator: Send + Sync {
    fn create_auto_output(&self, source_path: &Path, title: &str) -> Result<PathBuf, AppError>;
}

pub struct DefaultOutputCreator;

impl OutputCreator for DefaultOutputCreator {
    fn create_auto_output(&self, source_path: &Path, title: &str) -> Result<PathBuf, AppError> {
        rename::create_output_copy(source_path, title)
    }
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: BatchEvent);
}

pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _event: BatchEvent) {}
}

#[derive(Debug, Clone)]
struct RuntimeBatch {
    state: BatchState,
    cancel_token: Arc<AtomicBool>,
}

impl BatchScheduler {
    pub fn with_config(config: BatchSchedulerConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    pub fn start_batch_with_services(
        &self,
        input_paths: Vec<PathBuf>,
        settings: Settings,
        services: &BatchSchedulerServices<'_>,
    ) -> Result<BatchRunResult, AppError> {
        if input_paths.is_empty() {
            return Err(empty_input_error());
        }

        let batch_id = BatchId(Uuid::new_v4().to_string());
        let settings_snapshot = settings::create_settings_snapshot(&settings);
        services
            .history
            .save_settings_snapshot(&settings_snapshot)?;

        let mut jobs = ingest::scan_inputs(&batch_id, &input_paths, |fingerprint| {
            services.history.find_duplicate(fingerprint)
        })?;
        let created_at = Utc::now().to_rfc3339();
        let initial_summary = summary_from_jobs(&jobs);
        let initial_state = BatchState {
            batch_id: batch_id.clone(),
            created_at: created_at.clone(),
            status: BatchStatus::Running,
            settings_snapshot_id: settings_snapshot.id.clone(),
            files: jobs.iter().map(FileJobView::from).collect(),
            summary: initial_summary.clone(),
        };

        services.history.create_batch(&BatchRecord {
            batch_id: batch_id.clone(),
            created_at: created_at.clone(),
            status: BatchStatus::Running,
            settings_snapshot_id: settings_snapshot.id.clone(),
            summary: initial_summary,
        })?;
        let cancel_token = self.insert_state(initial_state);

        services.events.emit(BatchEvent::BatchStarted {
            batch_id: batch_id.clone(),
            created_at: created_at.clone(),
            total_files: jobs.len(),
        });
        for job in &jobs {
            services.events.emit(BatchEvent::FileQueued {
                batch_id: batch_id.clone(),
                file: FileJobView::from(job),
            });
        }

        let temp_dir = BatchTempDir::new(&batch_id.0)?;
        self.process_jobs(
            &mut jobs,
            &settings,
            services,
            temp_dir.path(),
            cancel_token,
        );

        let final_status = if self.is_cancel_requested(&batch_id) {
            BatchStatus::Cancelled
        } else {
            BatchStatus::Completed
        };
        self.set_final_state(&batch_id, final_status.clone());
        let final_state = self
            .get_batch_state(&batch_id)?
            .ok_or_else(|| AppError::internal("batch state disappeared during finalization"))?;

        services.history.create_batch(&BatchRecord {
            batch_id: batch_id.clone(),
            created_at,
            status: final_status.clone(),
            settings_snapshot_id: settings_snapshot.id,
            summary: final_state.summary.clone(),
        })?;

        match final_status {
            BatchStatus::Cancelled => services.events.emit(BatchEvent::BatchCancelled {
                batch_id: batch_id.clone(),
                summary: final_state.summary.clone(),
            }),
            BatchStatus::Completed => services.events.emit(BatchEvent::BatchCompleted {
                batch_id: batch_id.clone(),
                summary: final_state.summary.clone(),
            }),
            BatchStatus::Running | BatchStatus::Failed => {}
        }

        Ok(BatchRunResult {
            batch_id,
            summary: final_state.summary,
        })
    }

    pub fn cancel_batch(&self, batch_id: &BatchId) -> Result<(), AppError> {
        let mut batches = self.inner.lock().unwrap();
        let Some(batch) = batches.get_mut(&batch_id.0) else {
            return Err(AppError::internal("batch not found").with_stage(ProcessingStage::History));
        };

        batch.cancel_token.store(true, Ordering::SeqCst);
        batch.state.status = BatchStatus::Cancelled;
        Ok(())
    }

    pub fn get_batch_state(&self, batch_id: &BatchId) -> Result<Option<BatchState>, AppError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .get(&batch_id.0)
            .map(|runtime| runtime.state.clone()))
    }

    fn insert_state(&self, state: BatchState) -> Arc<AtomicBool> {
        let cancel_token = Arc::new(AtomicBool::new(false));
        self.inner.lock().unwrap().insert(
            state.batch_id.0.clone(),
            RuntimeBatch {
                state,
                cancel_token: Arc::clone(&cancel_token),
            },
        );
        cancel_token
    }

    fn process_jobs(
        &self,
        jobs: &mut [FileJob],
        settings: &Settings,
        services: &BatchSchedulerServices<'_>,
        work_dir: &Path,
        cancel_token: Arc<AtomicBool>,
    ) {
        let profile = ScoringProfile::from(settings);
        let mut queued_jobs = VecDeque::new();

        for job in jobs.iter_mut() {
            if cancel_token.load(Ordering::SeqCst) {
                self.cancel_job(job, services);
                continue;
            }

            match job.status {
                FileStatus::Skipped => self.handle_skipped_job(job, services),
                FileStatus::Pending => self.handle_existing_pending_job(job, services),
                FileStatus::Queued => queued_jobs.push_back(job.clone()),
                FileStatus::Analyzing
                | FileStatus::OutputCreated
                | FileStatus::Failed
                | FileStatus::Undoable
                | FileStatus::Cancelled => {}
            }
        }

        if queued_jobs.is_empty() {
            return;
        }

        let queue = Arc::new(Mutex::new(queued_jobs));
        let ocr_limiter = Arc::new(Semaphore::new(self.config.max_parallel_ocr.max(1)));
        let doc_limiter = Arc::new(Semaphore::new(
            self.config.max_parallel_doc_conversion.max(1),
        ));
        let worker_count = self
            .config
            .max_parallel_files
            .max(1)
            .min(queue.lock().unwrap().len());

        thread::scope(|scope| {
            for _ in 0..worker_count {
                let queue = Arc::clone(&queue);
                let ocr_limiter = Arc::clone(&ocr_limiter);
                let doc_limiter = Arc::clone(&doc_limiter);
                let profile = profile.clone();
                let scheduler = self.clone();
                let cancel_token = Arc::clone(&cancel_token);

                scope.spawn(move || loop {
                    let Some(mut job) = queue.lock().unwrap().pop_front() else {
                        return;
                    };

                    if cancel_token.load(Ordering::SeqCst) {
                        scheduler.cancel_job(&mut job, services);
                        continue;
                    }

                    let _permit = limiter_for_job(&job, &ocr_limiter, &doc_limiter);
                    if cancel_token.load(Ordering::SeqCst) {
                        scheduler.cancel_job(&mut job, services);
                        continue;
                    }

                    scheduler.process_queued_job(&mut job, &profile, services, work_dir);
                });
            }
        });
    }

    fn process_queued_job(
        &self,
        job: &mut FileJob,
        profile: &ScoringProfile,
        services: &BatchSchedulerServices<'_>,
        work_dir: &Path,
    ) {
        job.status = FileStatus::Analyzing;
        self.sync_job(job);
        emit_progress(services, job, ProcessingStage::Extract, Some(0.0));

        let request = ExtractRequest {
            batch_id: job.batch_id.clone(),
            file_job_id: job.file_job_id.clone(),
            file_type: job.file_type.clone(),
            source_path: PathBuf::from(&job.source_path),
        };
        let extracted = match catch_unwind(AssertUnwindSafe(|| {
            services.extractor.extract(&request, work_dir)
        })) {
            Ok(Ok(extracted)) => extracted,
            Ok(Err(error)) => {
                self.fail_job(job, services, error, None);
                return;
            }
            Err(panic) => {
                let error = extraction_panic_error(&request, panic);
                self.fail_job(job, services, error, None);
                return;
            }
        };
        let extract_method = extracted.extract_method.clone();
        services.events.emit(BatchEvent::FileExtracted {
            batch_id: job.batch_id.clone(),
            file_job_id: job.file_job_id.clone(),
            extract_method,
        });

        emit_progress(services, job, ProcessingStage::Score, Some(0.0));
        let scoring_result = scoring::score_document(extracted, profile.clone());
        services.events.emit(BatchEvent::FileScored {
            batch_id: job.batch_id.clone(),
            file_job_id: job.file_job_id.clone(),
            result: scoring_result.clone(),
        });

        match scoring_result.decision {
            ScoreDecision::AutoOutput => {
                self.create_auto_output(job, services, scoring_result);
            }
            ScoreDecision::Pending => {
                self.mark_low_confidence_pending(job, services, scoring_result);
            }
            ScoreDecision::Failed => {
                self.mark_no_trusted_title_pending(job, services, scoring_result);
            }
        }
    }

    fn create_auto_output(
        &self,
        job: &mut FileJob,
        services: &BatchSchedulerServices<'_>,
        scoring_result: ScoringResult,
    ) {
        let Some(title) = scoring_result.final_title.clone() else {
            self.mark_no_trusted_title_pending(job, services, scoring_result);
            return;
        };

        emit_progress(services, job, ProcessingStage::Rename, Some(0.0));
        let output_path = match services
            .output
            .create_auto_output(Path::new(&job.source_path), &title)
        {
            Ok(path) => path,
            Err(error) => {
                self.fail_job(job, services, error, Some(scoring_result));
                return;
            }
        };

        job.status = FileStatus::OutputCreated;
        job.recognized_title = Some(title);
        job.confidence = Some(scoring_result.confidence);
        job.output_path = Some(output_path.display().to_string());
        job.failure_reason = None;
        job.pending_reason = None;
        self.sync_job(job);

        services.events.emit(BatchEvent::FileOutputCreated {
            batch_id: job.batch_id.clone(),
            file_job_id: job.file_job_id.clone(),
            output_path: output_path.display().to_string(),
        });
        emit_progress(services, job, ProcessingStage::History, Some(0.0));

        if let Err(error) = self.record_job_result(
            job,
            services,
            Some(scoring_result),
            None,
            Some(OutputKind::Auto),
        ) {
            self.fail_job(job, services, error, None);
            return;
        }

        if let Err(error) = services
            .history
            .record_undo_for_output(&job.file_job_id, &output_path)
        {
            self.fail_job(job, services, error, None);
        }
    }

    fn mark_low_confidence_pending(
        &self,
        job: &mut FileJob,
        services: &BatchSchedulerServices<'_>,
        scoring_result: ScoringResult,
    ) {
        job.status = FileStatus::Pending;
        job.recognized_title = scoring_result.final_title.clone();
        job.confidence = Some(scoring_result.confidence);
        job.failure_reason = Some("置信度低于自动输出阈值，等待手动确认。".into());
        job.pending_reason = Some(PendingReason::LowConfidence);
        self.sync_job(job);
        services.events.emit(BatchEvent::FilePending {
            batch_id: job.batch_id.clone(),
            file_job_id: job.file_job_id.clone(),
            reason: PendingReason::LowConfidence,
            suggestion: scoring_result.final_title.clone(),
        });
        emit_progress(services, job, ProcessingStage::History, Some(0.0));
        let _ = self.record_job_result(job, services, Some(scoring_result), None, None);
    }

    fn mark_no_trusted_title_pending(
        &self,
        job: &mut FileJob,
        services: &BatchSchedulerServices<'_>,
        scoring_result: ScoringResult,
    ) {
        job.status = FileStatus::Pending;
        job.recognized_title = scoring_result.final_title.clone();
        job.confidence = Some(scoring_result.confidence);
        job.failure_reason = Some("未找到可信标题，等待手动确认。".into());
        job.pending_reason = Some(PendingReason::LowConfidence);
        self.sync_job(job);
        services.events.emit(BatchEvent::FilePending {
            batch_id: job.batch_id.clone(),
            file_job_id: job.file_job_id.clone(),
            reason: PendingReason::LowConfidence,
            suggestion: scoring_result.final_title.clone(),
        });
        emit_progress(services, job, ProcessingStage::History, Some(0.0));
        let _ = self.record_job_result(job, services, Some(scoring_result), None, None);
    }

    fn handle_skipped_job(&self, job: &mut FileJob, services: &BatchSchedulerServices<'_>) {
        services.events.emit(BatchEvent::FileSkipped {
            batch_id: job.batch_id.clone(),
            file_job_id: job.file_job_id.clone(),
            reason: job
                .failure_reason
                .clone()
                .unwrap_or_else(|| "文件已跳过。".into()),
        });
        emit_progress(services, job, ProcessingStage::History, Some(0.0));
        let _ = self.record_job_result(job, services, None, None, None);
        self.sync_job(job);
    }

    fn handle_existing_pending_job(
        &self,
        job: &mut FileJob,
        services: &BatchSchedulerServices<'_>,
    ) {
        let reason = job
            .pending_reason
            .clone()
            .unwrap_or(PendingReason::LowConfidence);
        services.events.emit(BatchEvent::FilePending {
            batch_id: job.batch_id.clone(),
            file_job_id: job.file_job_id.clone(),
            reason,
            suggestion: job.recognized_title.clone(),
        });
        emit_progress(services, job, ProcessingStage::History, Some(0.0));
        let _ = self.record_job_result(job, services, None, None, None);
        self.sync_job(job);
    }

    fn cancel_job(&self, job: &mut FileJob, services: &BatchSchedulerServices<'_>) {
        if matches!(
            job.status,
            FileStatus::Queued | FileStatus::Analyzing | FileStatus::Pending
        ) {
            job.status = FileStatus::Cancelled;
            job.failure_reason = Some("批次已取消，文件未继续处理。".into());
            self.sync_job(job);
            emit_progress(services, job, ProcessingStage::History, Some(0.0));
            let _ = self.record_job_result(job, services, None, None, None);
        }
    }

    fn fail_job(
        &self,
        job: &mut FileJob,
        services: &BatchSchedulerServices<'_>,
        error: AppError,
        scoring_result: Option<ScoringResult>,
    ) {
        job.status = FileStatus::Failed;
        job.failure_reason = Some(error.user_message.clone());
        job.pending_reason = None;
        self.sync_job(job);
        services.events.emit(BatchEvent::FileFailed {
            batch_id: job.batch_id.clone(),
            file_job_id: job.file_job_id.clone(),
            error: error.clone(),
        });
        emit_progress(services, job, ProcessingStage::History, Some(0.0));
        let _ = self.record_job_result(job, services, scoring_result, Some(error), None);
    }

    fn record_job_result(
        &self,
        job: &FileJob,
        services: &BatchSchedulerServices<'_>,
        scoring_result: Option<ScoringResult>,
        error: Option<AppError>,
        output_kind: Option<OutputKind>,
    ) -> Result<(), AppError> {
        services.history.record_file_result(&FileResultRecord {
            file: FileJobView::from(job),
            source_fingerprint: job.fingerprint.clone(),
            scoring_result,
            error,
            output_kind,
        })
    }

    fn sync_job(&self, job: &FileJob) {
        let mut batches = self.inner.lock().unwrap();
        let Some(runtime) = batches.get_mut(&job.batch_id.0) else {
            return;
        };

        if let Some(file) = runtime
            .state
            .files
            .iter_mut()
            .find(|file| file.file_job_id == job.file_job_id)
        {
            *file = FileJobView::from(job);
        }
        runtime.state.summary = summary_from_views(&runtime.state.files);
    }

    fn is_cancel_requested(&self, batch_id: &BatchId) -> bool {
        self.inner
            .lock()
            .unwrap()
            .get(&batch_id.0)
            .map(|runtime| runtime.cancel_token.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    fn set_final_state(&self, batch_id: &BatchId, status: BatchStatus) {
        let mut batches = self.inner.lock().unwrap();
        let Some(runtime) = batches.get_mut(&batch_id.0) else {
            return;
        };

        runtime.state.status = status;
        runtime.state.summary = summary_from_views(&runtime.state.files);
    }
}

fn emit_progress(
    services: &BatchSchedulerServices<'_>,
    job: &FileJob,
    stage: ProcessingStage,
    progress: Option<f32>,
) {
    services.events.emit(BatchEvent::FileProgress {
        batch_id: job.batch_id.clone(),
        file_job_id: job.file_job_id.clone(),
        stage,
        progress,
    });
}

struct Semaphore {
    state: Mutex<usize>,
    available: Condvar,
}

impl Semaphore {
    fn new(limit: usize) -> Self {
        Self {
            state: Mutex::new(limit),
            available: Condvar::new(),
        }
    }

    fn acquire(&self) -> SemaphorePermit<'_> {
        let mut remaining = self.state.lock().unwrap();
        while *remaining == 0 {
            remaining = self.available.wait(remaining).unwrap();
        }
        *remaining -= 1;
        SemaphorePermit { semaphore: self }
    }

    fn release(&self) {
        *self.state.lock().unwrap() += 1;
        self.available.notify_one();
    }
}

struct SemaphorePermit<'a> {
    semaphore: &'a Semaphore,
}

impl Drop for SemaphorePermit<'_> {
    fn drop(&mut self) {
        self.semaphore.release();
    }
}

fn limiter_for_job<'a>(
    job: &FileJob,
    ocr_limiter: &'a Semaphore,
    doc_limiter: &'a Semaphore,
) -> Option<SemaphorePermit<'a>> {
    match job.file_type {
        FileType::Doc => Some(doc_limiter.acquire()),
        FileType::Png | FileType::Jpg | FileType::Jpeg => Some(ocr_limiter.acquire()),
        FileType::Pdf
            if job.file_name.to_ascii_lowercase().contains("scan")
                || job.file_name.contains("扫描") =>
        {
            Some(ocr_limiter.acquire())
        }
        FileType::Docx | FileType::Pdf | FileType::Unsupported => None,
    }
}

fn summary_from_jobs(jobs: &[FileJob]) -> BatchSummary {
    let views = jobs.iter().map(FileJobView::from).collect::<Vec<_>>();
    summary_from_views(&views)
}

fn summary_from_views(files: &[FileJobView]) -> BatchSummary {
    BatchSummary {
        total: files.len(),
        output_created: files
            .iter()
            .filter(|file| {
                matches!(
                    file.status,
                    FileStatus::OutputCreated | FileStatus::Undoable
                )
            })
            .count(),
        pending: files
            .iter()
            .filter(|file| file.status == FileStatus::Pending)
            .count(),
        skipped: files
            .iter()
            .filter(|file| file.status == FileStatus::Skipped)
            .count(),
        failed: files
            .iter()
            .filter(|file| file.status == FileStatus::Failed)
            .count(),
        cancelled: files
            .iter()
            .filter(|file| file.status == FileStatus::Cancelled)
            .count(),
    }
}

fn empty_input_error() -> AppError {
    AppError {
        code: ErrorCode::FileReadFailed,
        category: ErrorCategory::Input,
        user_message: "请选择至少一个文件或文件夹。".into(),
        technical_detail: None,
        retryable: false,
        file_path: None,
        stage: Some(ProcessingStage::Ingest),
    }
}

fn extraction_panic_error(request: &ExtractRequest, panic: Box<dyn Any + Send>) -> AppError {
    let (code, user_message) = match request.file_type {
        FileType::Pdf => (ErrorCode::PdfExtractFailed, "PDF 提取失败"),
        FileType::Doc | FileType::Docx => {
            (ErrorCode::WordExtractFailed, "无法提取 Word 文档文本。")
        }
        FileType::Png | FileType::Jpg | FileType::Jpeg => {
            (ErrorCode::OcrEngineFailed, "无法执行图片 OCR。")
        }
        FileType::Unsupported => (
            ErrorCode::UnsupportedFormat,
            "不支持的文件格式，无法提取内容。",
        ),
    };

    AppError {
        code,
        category: ErrorCategory::Extraction,
        user_message: user_message.into(),
        technical_detail: Some(format!(
            "extractor panicked while processing '{}': {}",
            request.source_path.display(),
            panic_payload_message(panic.as_ref())
        )),
        retryable: true,
        file_path: Some(request.source_path.display().to_string()),
        stage: Some(ProcessingStage::Extract),
    }
}

fn panic_payload_message(panic: &(dyn Any + Send)) -> String {
    if let Some(message) = panic.downcast_ref::<&str>() {
        return (*message).into();
    }
    if let Some(message) = panic.downcast_ref::<String>() {
        return message.clone();
    }
    "unknown panic payload".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::{AppError, ErrorCategory, ErrorCode, ProcessingStage};
    use crate::history::{BatchRecord, DuplicateMatch, FileResultRecord, OutputKind};
    use crate::models::{
        BatchEvent, BatchId, BatchStatus, ExtractMethod, ExtractedDocument, ExtractedPage,
        FileJobId, FileStatus, FileType, LayoutBlock, NormalizedBox, ParagraphBlock, PendingReason,
        Settings, SourceFingerprint, SourceUnit,
    };
    use crate::settings::SettingsSnapshot;
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn start_batch_initializes_state_history_and_queue_events() {
        let dir = tempfile::tempdir().unwrap();
        let skipped = create_file(dir.path(), "notes.txt", b"plain");
        let services = TestServices::default();
        let scheduler = BatchScheduler::default();

        let result = scheduler
            .start_batch_with_services(vec![skipped], Settings::default(), &services.refs())
            .unwrap();
        let state = scheduler
            .get_batch_state(&result.batch_id)
            .unwrap()
            .expect("state should exist");
        let events = services.events.events();
        let history = services.history.snapshot();

        assert_eq!(state.batch_id, result.batch_id);
        assert_eq!(state.status, BatchStatus::Completed);
        assert_eq!(state.summary.total, 1);
        assert_eq!(state.summary.skipped, 1);
        assert_eq!(history.settings_snapshots.len(), 1);
        assert_eq!(
            history.batches.first().unwrap().status,
            BatchStatus::Running
        );
        assert_eq!(
            history.batches.last().unwrap().status,
            BatchStatus::Completed
        );
        assert!(matches!(events[0], BatchEvent::BatchStarted { .. }));
        assert!(matches!(events[1], BatchEvent::FileQueued { .. }));
        assert!(events
            .iter()
            .any(|event| matches!(event, BatchEvent::FileSkipped { .. })));
        assert!(matches!(
            events.last().unwrap(),
            BatchEvent::BatchCompleted { .. }
        ));
    }

    #[test]
    fn start_batch_rejects_empty_path_list() {
        let services = TestServices::default();
        let scheduler = BatchScheduler::default();

        let err = scheduler
            .start_batch_with_services(Vec::new(), Settings::default(), &services.refs())
            .expect_err("empty input must fail");

        assert_eq!(err.category, ErrorCategory::Input);
        assert_eq!(err.stage, Some(ProcessingStage::Ingest));
    }

    #[test]
    fn start_batch_processes_auto_pending_skipped_and_duplicate_files() {
        let dir = tempfile::tempdir().unwrap();
        let auto = create_file(dir.path(), "auto.pdf", b"pdf");
        let pending = create_file(dir.path(), "pending.docx", b"docx");
        let skipped = create_file(dir.path(), "unsupported.xlsx", b"xlsx");
        let duplicate = create_file(dir.path(), "duplicate.pdf", b"same");
        let services = TestServices::default();
        services
            .extractor
            .insert("auto.pdf", Ok(high_confidence_pdf_document()));
        services
            .extractor
            .insert("pending.docx", Ok(low_confidence_word_document()));
        services
            .extractor
            .insert("duplicate.pdf", Ok(high_confidence_pdf_document()));
        services.history.mark_duplicate_path("duplicate.pdf");
        let scheduler = BatchScheduler::default();

        let result = scheduler
            .start_batch_with_services(
                vec![auto, pending, skipped, duplicate],
                Settings::default(),
                &services.refs(),
            )
            .unwrap();

        let state = scheduler
            .get_batch_state(&result.batch_id)
            .unwrap()
            .expect("state should exist");
        let history = services.history.snapshot();
        let events = services.events.events();

        assert_eq!(state.summary.total, 4);
        assert_eq!(state.summary.output_created, 2);
        assert_eq!(state.summary.pending, 1);
        assert_eq!(state.summary.skipped, 1);
        assert_eq!(
            state
                .files
                .iter()
                .find(|file| file.file_name == "auto.pdf")
                .unwrap()
                .status,
            FileStatus::OutputCreated
        );
        assert_eq!(
            state
                .files
                .iter()
                .find(|file| file.file_name == "pending.docx")
                .unwrap()
                .pending_reason,
            Some(PendingReason::LowConfidence)
        );
        assert_eq!(
            sorted(services.extractor.calls()),
            vec![
                "auto.pdf".to_string(),
                "duplicate.pdf".to_string(),
                "pending.docx".to_string()
            ]
        );
        assert_eq!(history.file_results.len(), 4);
        assert_eq!(history.undo_records.len(), 2);
        assert!(history
            .file_results
            .iter()
            .any(|record| record.output_kind == Some(OutputKind::Auto)));
        let duplicate_file = state
            .files
            .iter()
            .find(|file| file.file_name == "duplicate.pdf")
            .unwrap();
        assert_eq!(duplicate_file.status, FileStatus::OutputCreated);
        assert_eq!(duplicate_file.pending_reason, None);
        assert_eq!(duplicate_file.failure_reason, None);
        assert!(duplicate_file
            .duplicate_warning
            .as_deref()
            .unwrap()
            .contains("old-batch"));
        assert!(events
            .iter()
            .any(|event| matches!(event, BatchEvent::FileExtracted { .. })));
        assert!(events
            .iter()
            .any(|event| matches!(event, BatchEvent::FileScored { .. })));
        assert!(events
            .iter()
            .any(|event| matches!(event, BatchEvent::FileOutputCreated { .. })));
        assert!(!events.iter().any(|event| matches!(
            event,
            BatchEvent::FilePending {
                reason: PendingReason::DuplicateSuspected,
                ..
            }
        )));
    }

    #[test]
    fn file_failure_does_not_stop_batch() {
        let dir = tempfile::tempdir().unwrap();
        let broken = create_file(dir.path(), "broken.pdf", b"bad");
        let good = create_file(dir.path(), "good.pdf", b"good");
        let services = TestServices::default();
        services.extractor.insert(
            "broken.pdf",
            Err(app_error(
                ErrorCode::PdfExtractFailed,
                ErrorCategory::Extraction,
                "PDF 提取失败",
                ProcessingStage::Extract,
            )),
        );
        services
            .extractor
            .insert("good.pdf", Ok(high_confidence_pdf_document()));
        let scheduler = BatchScheduler::default();

        let result = scheduler
            .start_batch_with_services(vec![broken, good], Settings::default(), &services.refs())
            .unwrap();
        let state = scheduler
            .get_batch_state(&result.batch_id)
            .unwrap()
            .expect("state should exist");
        let events = services.events.events();

        assert_eq!(state.summary.failed, 1);
        assert_eq!(state.summary.output_created, 1);
        assert_eq!(
            sorted(services.extractor.calls()),
            vec!["broken.pdf".to_string(), "good.pdf".to_string()]
        );
        assert!(events
            .iter()
            .any(|event| matches!(event, BatchEvent::FileFailed { .. })));
        assert!(matches!(
            events.last().unwrap(),
            BatchEvent::BatchCompleted { .. }
        ));
    }

    #[test]
    fn extractor_panic_fails_file_without_stopping_batch() {
        let dir = tempfile::tempdir().unwrap();
        let broken = create_file(dir.path(), "broken.pdf", b"bad");
        let good = create_file(dir.path(), "good.pdf", b"good");
        let services = TestServices::default();
        services.extractor.panic_on("broken.pdf");
        services
            .extractor
            .insert("good.pdf", Ok(high_confidence_pdf_document()));
        let scheduler = BatchScheduler::default();

        let result = scheduler
            .start_batch_with_services(vec![broken, good], Settings::default(), &services.refs())
            .unwrap();
        let state = scheduler
            .get_batch_state(&result.batch_id)
            .unwrap()
            .expect("state should exist");
        let failed = state
            .files
            .iter()
            .find(|file| file.file_name == "broken.pdf")
            .unwrap();

        assert_eq!(state.summary.failed, 1);
        assert_eq!(state.summary.output_created, 1);
        assert_eq!(failed.status, FileStatus::Failed);
        assert_eq!(failed.failure_reason.as_deref(), Some("PDF 提取失败"));
        assert!(matches!(
            services.events.events().last().unwrap(),
            BatchEvent::BatchCompleted { .. }
        ));
    }

    #[test]
    fn cancel_batch_updates_state_and_emits_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let first = create_file(dir.path(), "first.pdf", b"one");
        let second = create_file(dir.path(), "second.pdf", b"two");
        let services = TestServices::default();
        services
            .extractor
            .insert("first.pdf", Ok(high_confidence_pdf_document()));
        services
            .extractor
            .insert("second.pdf", Ok(high_confidence_pdf_document()));
        let scheduler = BatchScheduler::with_config(BatchSchedulerConfig {
            max_parallel_files: 1,
            ..BatchSchedulerConfig::default()
        });
        services
            .extractor
            .cancel_after_first_file(scheduler.clone());

        let result = scheduler
            .start_batch_with_services(vec![first, second], Settings::default(), &services.refs())
            .unwrap();
        let state = scheduler
            .get_batch_state(&result.batch_id)
            .unwrap()
            .expect("state should exist");
        let events = services.events.events();

        assert_eq!(state.status, BatchStatus::Cancelled);
        assert_eq!(state.summary.cancelled, 1);
        assert_eq!(services.extractor.calls(), vec!["first.pdf"]);
        assert!(matches!(
            events.last().unwrap(),
            BatchEvent::BatchCancelled { .. }
        ));
    }

    #[test]
    fn worker_pool_limits_general_ocr_and_doc_concurrency() {
        let dir = tempfile::tempdir().unwrap();
        let paths = vec![
            create_file(dir.path(), "a.pdf", b"pdf"),
            create_file(dir.path(), "b.pdf", b"pdf"),
            create_file(dir.path(), "c.pdf", b"pdf"),
            create_file(dir.path(), "image-1.png", b"png"),
            create_file(dir.path(), "image-2.jpg", b"jpg"),
            create_file(dir.path(), "legacy-1.doc", b"doc"),
            create_file(dir.path(), "legacy-2.doc", b"doc"),
        ];
        let services = TestServices::default();
        services.extractor.set_delay(Duration::from_millis(30));
        let scheduler = BatchScheduler::with_config(BatchSchedulerConfig {
            max_parallel_files: 3,
            max_parallel_ocr: 1,
            max_parallel_doc_conversion: 1,
        });

        scheduler
            .start_batch_with_services(paths, Settings::default(), &services.refs())
            .unwrap();

        assert!(services.extractor.max_active_all() > 1);
        assert!(services.extractor.max_active_all() <= 3);
        assert_eq!(services.extractor.max_active_ocr(), 1);
        assert_eq!(services.extractor.max_active_doc(), 1);
    }

    #[derive(Default)]
    struct TestServices {
        history: FakeHistoryStore,
        extractor: FakeExtractor,
        output: FakeOutputCreator,
        events: CapturingEventSink,
    }

    impl TestServices {
        fn refs(&self) -> BatchSchedulerServices<'_> {
            BatchSchedulerServices {
                history: &self.history,
                extractor: &self.extractor,
                output: &self.output,
                events: &self.events,
            }
        }
    }

    #[derive(Default)]
    struct FakeHistoryStore {
        inner: Mutex<FakeHistoryState>,
    }

    #[derive(Default, Clone)]
    struct FakeHistoryState {
        settings_snapshots: Vec<SettingsSnapshot>,
        batches: Vec<BatchRecord>,
        file_results: Vec<FileResultRecord>,
        undo_records: Vec<(FileJobId, String)>,
        duplicate_path_fragments: HashSet<String>,
    }

    impl FakeHistoryStore {
        fn snapshot(&self) -> FakeHistoryState {
            self.inner.lock().unwrap().clone()
        }

        fn mark_duplicate_path(&self, path_fragment: &str) {
            self.inner
                .lock()
                .unwrap()
                .duplicate_path_fragments
                .insert(path_fragment.into());
        }
    }

    impl HistoryStore for FakeHistoryStore {
        fn find_duplicate(
            &self,
            fingerprint: &SourceFingerprint,
        ) -> Result<Option<DuplicateMatch>, AppError> {
            let state = self.inner.lock().unwrap();
            if state
                .duplicate_path_fragments
                .iter()
                .any(|fragment| fingerprint.normalized_path.contains(fragment))
            {
                Ok(Some(DuplicateMatch {
                    batch_id: BatchId("old-batch".into()),
                    file_job_id: FileJobId("old-file".into()),
                    output_path: Some("/old/output.pdf".into()),
                }))
            } else {
                Ok(None)
            }
        }

        fn save_settings_snapshot(&self, snapshot: &SettingsSnapshot) -> Result<(), AppError> {
            self.inner
                .lock()
                .unwrap()
                .settings_snapshots
                .push(snapshot.clone());
            Ok(())
        }

        fn create_batch(&self, record: &BatchRecord) -> Result<(), AppError> {
            self.inner.lock().unwrap().batches.push(record.clone());
            Ok(())
        }

        fn record_file_result(&self, record: &FileResultRecord) -> Result<(), AppError> {
            self.inner.lock().unwrap().file_results.push(record.clone());
            Ok(())
        }

        fn record_undo_for_output(
            &self,
            file_job_id: &FileJobId,
            output_path: &Path,
        ) -> Result<(), AppError> {
            self.inner
                .lock()
                .unwrap()
                .undo_records
                .push((file_job_id.clone(), output_path.display().to_string()));
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeExtractor {
        documents: Mutex<HashMap<String, Result<ExtractedDocument, AppError>>>,
        panic_files: Mutex<HashSet<String>>,
        calls: Mutex<Vec<String>>,
        cancel_scheduler: Mutex<Option<BatchScheduler>>,
        delay: Mutex<Option<Duration>>,
        tracker: ConcurrencyTracker,
    }

    impl FakeExtractor {
        fn insert(&self, file_name: &str, result: Result<ExtractedDocument, AppError>) {
            self.documents
                .lock()
                .unwrap()
                .insert(file_name.into(), result);
        }

        fn panic_on(&self, file_name: &str) {
            self.panic_files.lock().unwrap().insert(file_name.into());
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }

        fn cancel_after_first_file(&self, scheduler: BatchScheduler) {
            *self.cancel_scheduler.lock().unwrap() = Some(scheduler);
        }

        fn set_delay(&self, delay: Duration) {
            *self.delay.lock().unwrap() = Some(delay);
        }

        fn max_active_all(&self) -> usize {
            self.tracker.max_all.load(Ordering::SeqCst)
        }

        fn max_active_ocr(&self) -> usize {
            self.tracker.max_ocr.load(Ordering::SeqCst)
        }

        fn max_active_doc(&self) -> usize {
            self.tracker.max_doc.load(Ordering::SeqCst)
        }
    }

    impl Extractor for FakeExtractor {
        fn extract(
            &self,
            request: &crate::extract::ExtractRequest,
            _work_dir: &Path,
        ) -> Result<ExtractedDocument, AppError> {
            let file_name = request
                .source_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            self.calls.lock().unwrap().push(file_name.clone());
            let _guard = self.tracker.enter(&request.file_type);

            if self.panic_files.lock().unwrap().contains(&file_name) {
                panic!("extractor panic for {file_name}");
            }

            if let Some(scheduler) = self.cancel_scheduler.lock().unwrap().clone() {
                scheduler.cancel_batch(&request.batch_id).unwrap();
            }

            if let Some(delay) = *self.delay.lock().unwrap() {
                std::thread::sleep(delay);
            }

            self.documents
                .lock()
                .unwrap()
                .get(&file_name)
                .cloned()
                .unwrap_or_else(|| Ok(high_confidence_pdf_document()))
        }
    }

    #[derive(Default)]
    struct ConcurrencyTracker {
        active_all: AtomicUsize,
        max_all: AtomicUsize,
        active_ocr: AtomicUsize,
        max_ocr: AtomicUsize,
        active_doc: AtomicUsize,
        max_doc: AtomicUsize,
    }

    impl ConcurrencyTracker {
        fn enter(&self, file_type: &FileType) -> ConcurrencyGuard<'_> {
            increment_and_record(&self.active_all, &self.max_all);
            let category = match file_type {
                FileType::Doc => {
                    increment_and_record(&self.active_doc, &self.max_doc);
                    ConcurrencyCategory::Doc
                }
                FileType::Png | FileType::Jpg | FileType::Jpeg => {
                    increment_and_record(&self.active_ocr, &self.max_ocr);
                    ConcurrencyCategory::Ocr
                }
                _ => ConcurrencyCategory::General,
            };

            ConcurrencyGuard {
                tracker: self,
                category,
            }
        }
    }

    struct ConcurrencyGuard<'a> {
        tracker: &'a ConcurrencyTracker,
        category: ConcurrencyCategory,
    }

    impl Drop for ConcurrencyGuard<'_> {
        fn drop(&mut self) {
            self.tracker.active_all.fetch_sub(1, Ordering::SeqCst);
            match self.category {
                ConcurrencyCategory::Ocr => {
                    self.tracker.active_ocr.fetch_sub(1, Ordering::SeqCst);
                }
                ConcurrencyCategory::Doc => {
                    self.tracker.active_doc.fetch_sub(1, Ordering::SeqCst);
                }
                ConcurrencyCategory::General => {}
            }
        }
    }

    enum ConcurrencyCategory {
        General,
        Ocr,
        Doc,
    }

    fn increment_and_record(active: &AtomicUsize, max: &AtomicUsize) {
        let current = active.fetch_add(1, Ordering::SeqCst) + 1;
        let mut previous = max.load(Ordering::SeqCst);
        while current > previous {
            match max.compare_exchange(previous, current, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => break,
                Err(next) => previous = next,
            }
        }
    }

    #[derive(Default)]
    struct FakeOutputCreator {
        created: Mutex<Vec<(String, String)>>,
    }

    impl OutputCreator for FakeOutputCreator {
        fn create_auto_output(&self, source_path: &Path, title: &str) -> Result<PathBuf, AppError> {
            self.created
                .lock()
                .unwrap()
                .push((source_path.display().to_string(), title.to_string()));
            let output_dir = source_path.parent().unwrap().join("Rustitler 输出");
            fs::create_dir_all(&output_dir).unwrap();
            let output_path = output_dir.join(format!(
                "{title}.{}",
                source_path.extension().unwrap().to_string_lossy()
            ));
            fs::write(&output_path, b"output").unwrap();
            Ok(output_path)
        }
    }

    #[derive(Default)]
    struct CapturingEventSink {
        events: Mutex<Vec<BatchEvent>>,
    }

    impl CapturingEventSink {
        fn events(&self) -> Vec<BatchEvent> {
            self.events.lock().unwrap().clone()
        }
    }

    impl EventSink for CapturingEventSink {
        fn emit(&self, event: BatchEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    fn create_file(dir: &Path, name: &str, contents: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, contents).unwrap();
        path
    }

    fn sorted(mut values: Vec<String>) -> Vec<String> {
        values.sort();
        values
    }

    fn high_confidence_pdf_document() -> ExtractedDocument {
        ExtractedDocument {
            source_type: FileType::Pdf,
            extract_method: ExtractMethod::PdfNativeLiteparse,
            pages: vec![ExtractedPage {
                page_index: 0,
                width: 100.0,
                height: 100.0,
                unit: SourceUnit::PdfPoint,
                blocks: vec![LayoutBlock {
                    text: "关于年度工作会议的通知".into(),
                    bbox: NormalizedBox {
                        x0: 0.20,
                        y0: 0.08,
                        x1: 0.80,
                        y1: 0.16,
                    },
                    raw_bbox: None,
                    font_size: Some(24.0),
                    bold: Some(true),
                    ocr_confidence: None,
                    line_index: Some(0),
                }],
            }],
            paragraphs: vec![],
            diagnostics_ref: None,
        }
    }

    fn low_confidence_word_document() -> ExtractedDocument {
        ExtractedDocument {
            source_type: FileType::Docx,
            extract_method: ExtractMethod::WordUndoc,
            pages: vec![],
            paragraphs: vec![ParagraphBlock {
                text: "普通标题".into(),
                paragraph_index: 0,
            }],
            diagnostics_ref: None,
        }
    }

    fn app_error(
        code: ErrorCode,
        category: ErrorCategory,
        message: &str,
        stage: ProcessingStage,
    ) -> AppError {
        AppError {
            code,
            category,
            user_message: message.into(),
            technical_detail: None,
            retryable: true,
            file_path: None,
            stage: Some(stage),
        }
    }
}
