use crate::errors::{AppError, ErrorCategory, ErrorCode, ProcessingStage};
use crate::history::DuplicateMatch;
use crate::models::{
    BatchId, FileJob, FileJobId, FileStatus, FileType, PendingReason, SourceFingerprint,
};
use chrono::{DateTime, Utc};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use uuid::Uuid;

pub fn scan_inputs<F>(
    batch_id: &BatchId,
    input_paths: &[PathBuf],
    mut find_duplicate: F,
) -> Result<Vec<FileJob>, AppError>
where
    F: FnMut(&SourceFingerprint) -> Result<Option<DuplicateMatch>, AppError>,
{
    let mut jobs = Vec::new();

    for input_path in input_paths {
        scan_input_path(batch_id, input_path, None, &mut find_duplicate, &mut jobs)?;
    }

    Ok(jobs)
}

fn scan_input_path<F>(
    batch_id: &BatchId,
    path: &Path,
    source_parent_path: Option<&Path>,
    find_duplicate: &mut F,
    jobs: &mut Vec<FileJob>,
) -> Result<(), AppError>
where
    F: FnMut(&SourceFingerprint) -> Result<Option<DuplicateMatch>, AppError>,
{
    let metadata = fs::metadata(path).map_err(|err| metadata_error(path, err))?;

    if metadata.is_dir() {
        match source_parent_path {
            Some(parent) => jobs.push(skipped_directory_job(batch_id, path, parent, &metadata)?),
            None => scan_folder_first_level(batch_id, path, find_duplicate, jobs)?,
        }
        return Ok(());
    }

    jobs.push(file_job_from_metadata(
        batch_id,
        path,
        source_parent_path,
        &metadata,
        find_duplicate,
    )?);
    Ok(())
}

fn scan_folder_first_level<F>(
    batch_id: &BatchId,
    folder: &Path,
    find_duplicate: &mut F,
    jobs: &mut Vec<FileJob>,
) -> Result<(), AppError>
where
    F: FnMut(&SourceFingerprint) -> Result<Option<DuplicateMatch>, AppError>,
{
    let mut entries = fs::read_dir(folder)
        .map_err(|err| metadata_error(folder, err))?
        .collect::<Result<Vec<_>, io::Error>>()
        .map_err(|err| metadata_error(folder, err))?;

    entries.sort_by(|left, right| {
        let left_path = left.path();
        let right_path = right.path();
        let left_is_dir = left_path.is_dir();
        let right_is_dir = right_path.is_dir();

        left_is_dir
            .cmp(&right_is_dir)
            .then_with(|| left.file_name().cmp(&right.file_name()))
    });

    for entry in entries {
        scan_input_path(batch_id, &entry.path(), Some(folder), find_duplicate, jobs)?;
    }

    Ok(())
}

fn file_job_from_metadata<F>(
    batch_id: &BatchId,
    path: &Path,
    source_parent_path: Option<&Path>,
    metadata: &fs::Metadata,
    find_duplicate: &mut F,
) -> Result<FileJob, AppError>
where
    F: FnMut(&SourceFingerprint) -> Result<Option<DuplicateMatch>, AppError>,
{
    let file_type = file_type_for_path(path);
    let fingerprint = compute_fingerprint(path, metadata)?;

    if !file_type.is_supported() {
        return unsupported_file_job(batch_id, path, source_parent_path, file_type, fingerprint);
    }

    let duplicate = find_duplicate(&fingerprint)?;
    queued_job(
        batch_id,
        path,
        source_parent_path,
        file_type,
        fingerprint,
        duplicate,
    )
}

fn queued_job(
    batch_id: &BatchId,
    path: &Path,
    source_parent_path: Option<&Path>,
    file_type: FileType,
    fingerprint: SourceFingerprint,
    duplicate: Option<DuplicateMatch>,
) -> Result<FileJob, AppError> {
    let (status, pending_reason, failure_reason) = match duplicate {
        Some(duplicate) => (
            FileStatus::Pending,
            Some(PendingReason::DuplicateSuspected),
            Some(duplicate_message(&duplicate)),
        ),
        None => (FileStatus::Queued, None, None),
    };

    Ok(FileJob {
        file_job_id: FileJobId(Uuid::new_v4().to_string()),
        batch_id: batch_id.clone(),
        source_path: path.display().to_string(),
        source_parent_path: source_parent_path.map(|path| path.display().to_string()),
        file_name: file_name_string(path)?,
        file_type,
        status,
        fingerprint,
        recognized_title: None,
        confidence: None,
        output_path: None,
        failure_reason,
        pending_reason,
    })
}

fn unsupported_file_job(
    batch_id: &BatchId,
    path: &Path,
    source_parent_path: Option<&Path>,
    file_type: FileType,
    fingerprint: SourceFingerprint,
) -> Result<FileJob, AppError> {
    Ok(FileJob {
        file_job_id: FileJobId(Uuid::new_v4().to_string()),
        batch_id: batch_id.clone(),
        source_path: path.display().to_string(),
        source_parent_path: source_parent_path.map(|path| path.display().to_string()),
        file_name: file_name_string(path)?,
        file_type,
        status: FileStatus::Skipped,
        fingerprint,
        recognized_title: None,
        confidence: None,
        output_path: None,
        failure_reason: Some("不支持的文件格式，已跳过。".into()),
        pending_reason: Some(PendingReason::UnsupportedFormat),
    })
}

fn skipped_directory_job(
    batch_id: &BatchId,
    path: &Path,
    source_parent_path: &Path,
    metadata: &fs::Metadata,
) -> Result<FileJob, AppError> {
    Ok(FileJob {
        file_job_id: FileJobId(Uuid::new_v4().to_string()),
        batch_id: batch_id.clone(),
        source_path: path.display().to_string(),
        source_parent_path: Some(source_parent_path.display().to_string()),
        file_name: file_name_string(path)?,
        file_type: FileType::Unsupported,
        status: FileStatus::Skipped,
        fingerprint: compute_fingerprint(path, metadata)?,
        recognized_title: None,
        confidence: None,
        output_path: None,
        failure_reason: Some("子文件夹不递归扫描，已作为不处理项跳过。".into()),
        pending_reason: Some(PendingReason::UnsupportedFormat),
    })
}

fn file_type_for_path(path: &Path) -> FileType {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(FileType::from_extension)
        .unwrap_or(FileType::Unsupported)
}

fn compute_fingerprint(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<SourceFingerprint, AppError> {
    Ok(SourceFingerprint {
        normalized_path: normalized_source_path(path)?,
        size_bytes: metadata.len(),
        modified_time: modified_time_string(
            metadata
                .modified()
                .map_err(|err| metadata_error(path, err))?,
        ),
    })
}

fn normalized_source_path(path: &Path) -> Result<String, AppError> {
    fs::canonicalize(path)
        .map(|path| path.display().to_string())
        .map_err(|err| metadata_error(path, err))
}

fn modified_time_string(time: SystemTime) -> String {
    let datetime: DateTime<Utc> = time.into();
    datetime.to_rfc3339()
}

fn file_name_string(path: &Path) -> Result<String, AppError> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| {
            AppError::internal("input path has no file name")
                .with_path(path.display().to_string())
                .with_stage(ProcessingStage::Ingest)
        })
}

fn duplicate_message(duplicate: &DuplicateMatch) -> String {
    match duplicate.output_path.as_deref() {
        Some(output_path) => format!(
            "疑似重复：历史批次 {} 的文件 {} 已输出到 {}。",
            duplicate.batch_id.0, duplicate.file_job_id.0, output_path
        ),
        None => format!(
            "疑似重复：历史批次 {} 的文件 {} 已处理过。",
            duplicate.batch_id.0, duplicate.file_job_id.0
        ),
    }
}

fn metadata_error(path: &Path, err: io::Error) -> AppError {
    let (code, user_message) = if err.kind() == io::ErrorKind::PermissionDenied {
        (
            ErrorCode::PermissionDenied,
            "无法读取输入路径，请检查权限。",
        )
    } else {
        (
            ErrorCode::FileReadFailed,
            "无法读取输入路径，请检查文件是否存在。",
        )
    };

    AppError {
        code,
        category: ErrorCategory::Input,
        user_message: user_message.into(),
        technical_detail: Some(format!("failed to read '{}': {err}", path.display())),
        retryable: true,
        file_path: Some(path.display().to_string()),
        stage: Some(ProcessingStage::Ingest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FileJobId, FileStatus, FileType, PendingReason};
    use std::fs;

    #[test]
    fn scan_inputs_accepts_direct_files_and_first_level_folder_entries() {
        let dir = tempfile::tempdir().unwrap();
        let direct = dir.path().join("直接.PDF");
        fs::write(&direct, b"pdf").unwrap();
        let folder = dir.path().join("folder");
        fs::create_dir_all(&folder).unwrap();
        let first_level = folder.join("报告.docx");
        fs::write(&first_level, b"docx").unwrap();
        fs::create_dir(folder.join("nested")).unwrap();

        let batch_id = BatchId("batch-001".into());
        let jobs = scan_inputs(&batch_id, &[direct.clone(), folder.clone()], |_| Ok(None)).unwrap();

        assert_eq!(jobs.len(), 3);
        assert_eq!(jobs[0].source_path, direct.display().to_string());
        assert_eq!(jobs[0].source_parent_path, None);
        assert_eq!(jobs[0].file_type, FileType::Pdf);
        assert_eq!(jobs[0].status, FileStatus::Queued);
        assert_eq!(
            jobs[1].source_parent_path,
            Some(folder.display().to_string())
        );
        assert_eq!(jobs[1].file_name, "报告.docx");
        assert_eq!(jobs[1].file_type, FileType::Docx);
        assert_eq!(
            jobs[2].source_parent_path,
            Some(folder.display().to_string())
        );
        assert_eq!(jobs[2].status, FileStatus::Skipped);
        assert!(matches!(
            jobs[2].pending_reason.as_ref(),
            Some(PendingReason::UnsupportedFormat)
        ));
        assert!(jobs[2]
            .failure_reason
            .as_deref()
            .unwrap()
            .contains("不递归扫描"));
    }

    #[test]
    fn scan_inputs_classifies_supported_formats_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        let names = ["a.docx", "b.DOC", "c.pdf", "d.PNG", "e.jpg", "f.JPEG"];
        let paths: Vec<PathBuf> = names
            .iter()
            .map(|name| {
                let path = dir.path().join(name);
                fs::write(&path, b"x").unwrap();
                path
            })
            .collect();

        let batch_id = BatchId("batch-001".into());
        let jobs = scan_inputs(&batch_id, &paths, |_| Ok(None)).unwrap();

        assert_eq!(
            jobs.iter()
                .map(|job| job.file_type.clone())
                .collect::<Vec<_>>(),
            vec![
                FileType::Docx,
                FileType::Doc,
                FileType::Pdf,
                FileType::Png,
                FileType::Jpg,
                FileType::Jpeg
            ]
        );
        assert!(jobs.iter().all(|job| job.status == FileStatus::Queued));
    }

    #[test]
    fn scan_inputs_marks_unsupported_files_skipped_with_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("表格.xlsx");
        fs::write(&source, b"sheet").unwrap();

        let batch_id = BatchId("batch-001".into());
        let jobs = scan_inputs(&batch_id, std::slice::from_ref(&source), |_| Ok(None)).unwrap();

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].file_type, FileType::Unsupported);
        assert_eq!(jobs[0].status, FileStatus::Skipped);
        assert!(matches!(
            jobs[0].pending_reason.as_ref(),
            Some(PendingReason::UnsupportedFormat)
        ));
        assert_eq!(jobs[0].fingerprint.size_bytes, 5);
        assert_eq!(
            jobs[0].fingerprint.normalized_path,
            fs::canonicalize(&source).unwrap().display().to_string()
        );
        assert!(!jobs[0].fingerprint.modified_time.is_empty());
    }

    #[test]
    fn scan_inputs_assigns_unique_file_job_ids() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("b.pdf");
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();

        let batch_id = BatchId("batch-001".into());
        let jobs = scan_inputs(&batch_id, &[a, b], |_| Ok(None)).unwrap();

        assert_ne!(jobs[0].file_job_id, jobs[1].file_job_id);
        assert!(jobs.iter().all(|job| job.batch_id == batch_id));
    }

    #[test]
    fn scan_inputs_marks_supported_duplicate_as_pending() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("合同.pdf");
        fs::write(&source, b"same").unwrap();

        let batch_id = BatchId("batch-001".into());
        let jobs = scan_inputs(&batch_id, std::slice::from_ref(&source), |_| {
            Ok(Some(DuplicateMatch {
                batch_id: BatchId("old-batch".into()),
                file_job_id: FileJobId("old-file".into()),
                output_path: Some("/output/合同.pdf".into()),
            }))
        })
        .unwrap();

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, FileStatus::Pending);
        assert!(matches!(
            jobs[0].pending_reason.as_ref(),
            Some(PendingReason::DuplicateSuspected)
        ));
        assert!(jobs[0]
            .failure_reason
            .as_deref()
            .unwrap()
            .contains("old-batch"));
        assert!(jobs[0]
            .failure_reason
            .as_deref()
            .unwrap()
            .contains("/output/合同.pdf"));
    }
}
