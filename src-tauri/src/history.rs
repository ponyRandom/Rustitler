use crate::errors::{AppError, ErrorCategory, ErrorCode, ProcessingStage};
use crate::models::{
    BatchId, BatchStatus, BatchSummary, CandidateSource, CandidateTitle, CategoryScores, FileJobId,
    FileJobView, HistoryBatchDetail, HistoryBatchPage, HistoryBatchRow, HistoryFileResult,
    RuleDetail, ScoringResult, SourceFingerprint, UndoResult,
};
use crate::settings::SettingsSnapshot;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const HISTORY_DB_NAME: &str = "history.sqlite";
const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone)]
pub struct BatchRecord {
    pub batch_id: BatchId,
    pub created_at: String,
    pub status: BatchStatus,
    pub settings_snapshot_id: String,
    pub summary: BatchSummary,
}

#[derive(Debug, Clone)]
pub struct FileResultRecord {
    pub file: FileJobView,
    pub source_fingerprint: SourceFingerprint,
    pub scoring_result: Option<ScoringResult>,
    pub error: Option<AppError>,
    pub output_kind: Option<OutputKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputKind {
    Auto,
    Manual,
}

impl OutputKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateMatch {
    pub batch_id: BatchId,
    pub file_job_id: FileJobId,
    pub output_path: Option<String>,
}

pub fn history_path(app_data_dir: impl AsRef<Path>) -> PathBuf {
    app_data_dir.as_ref().join(HISTORY_DB_NAME)
}

pub fn open_history(app_data_dir: impl AsRef<Path>) -> Result<Connection, AppError> {
    let path = history_path(app_data_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            history_error(format!("创建历史目录失败：{err}"))
                .with_path(parent.display().to_string())
        })?;
    }

    let conn = Connection::open(&path).map_err(|err| {
        history_error(format!("打开历史数据库失败：{err}")).with_path(path.display().to_string())
    })?;
    initialize_schema(&conn)?;
    Ok(conn)
}

fn initialize_schema(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS schema_meta (
            key TEXT PRIMARY KEY,
            value INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS settings_snapshots (
            settings_snapshot_id TEXT PRIMARY KEY,
            captured_at TEXT NOT NULL,
            settings_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS batches (
            batch_id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            status TEXT NOT NULL,
            settings_snapshot_id TEXT NOT NULL,
            total INTEGER NOT NULL,
            output_created INTEGER NOT NULL,
            pending INTEGER NOT NULL,
            skipped INTEGER NOT NULL,
            failed INTEGER NOT NULL,
            cancelled INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS file_results (
            file_job_id TEXT PRIMARY KEY,
            batch_id TEXT NOT NULL,
            source_path TEXT NOT NULL,
            file_name TEXT NOT NULL,
            file_type TEXT NOT NULL,
            status TEXT NOT NULL,
            recognized_title TEXT,
            confidence INTEGER,
            output_path TEXT,
            failure_reason TEXT,
            pending_reason TEXT,
            fingerprint_normalized_path TEXT NOT NULL,
            fingerprint_size_bytes INTEGER NOT NULL,
            fingerprint_modified_time TEXT NOT NULL,
            scoring_final_title TEXT,
            scoring_confidence INTEGER,
            scoring_decision TEXT,
            output_kind TEXT,
            error_json TEXT,
            FOREIGN KEY(batch_id) REFERENCES batches(batch_id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_file_results_batch_id
            ON file_results(batch_id);
        CREATE INDEX IF NOT EXISTS idx_file_results_duplicate
            ON file_results(
                fingerprint_normalized_path,
                fingerprint_size_bytes,
                fingerprint_modified_time,
                output_kind
            );

        CREATE TABLE IF NOT EXISTS candidates (
            candidate_id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_job_id TEXT NOT NULL,
            candidate_order INTEGER NOT NULL,
            text TEXT NOT NULL,
            source TEXT NOT NULL,
            page_index INTEGER,
            paragraph_index INTEGER,
            score INTEGER NOT NULL,
            layout_score INTEGER NOT NULL,
            position_score INTEGER NOT NULL,
            keyword_score INTEGER NOT NULL,
            text_quality_score INTEGER NOT NULL,
            penalty_score INTEGER NOT NULL,
            FOREIGN KEY(file_job_id) REFERENCES file_results(file_job_id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_candidates_file_job_id
            ON candidates(file_job_id, candidate_order);

        CREATE TABLE IF NOT EXISTS rule_details (
            rule_detail_id INTEGER PRIMARY KEY AUTOINCREMENT,
            candidate_id INTEGER NOT NULL,
            rule_order INTEGER NOT NULL,
            rule_name TEXT NOT NULL,
            category TEXT NOT NULL,
            delta INTEGER NOT NULL,
            description TEXT NOT NULL,
            FOREIGN KEY(candidate_id) REFERENCES candidates(candidate_id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_rule_details_candidate_id
            ON rule_details(candidate_id, rule_order);

        CREATE TABLE IF NOT EXISTS undo_records (
            undo_record_id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_job_id TEXT NOT NULL UNIQUE,
            batch_id TEXT NOT NULL,
            output_path TEXT NOT NULL,
            created_size_bytes INTEGER NOT NULL,
            created_modified_time TEXT NOT NULL,
            created_sha256 TEXT NOT NULL,
            status TEXT NOT NULL,
            message TEXT,
            undone_at TEXT,
            FOREIGN KEY(file_job_id) REFERENCES file_results(file_job_id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_undo_records_batch_id
            ON undo_records(batch_id);

        "#,
    )
    .map_err(|err| history_error(format!("初始化历史数据库失败：{err}")))?;

    conn.execute(
        "INSERT INTO schema_meta(key, value)
         VALUES ('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [SCHEMA_VERSION],
    )
    .map_err(|err| history_error(format!("记录历史数据库版本失败：{err}")))?;

    Ok(())
}

pub fn save_settings_snapshot(
    conn: &Connection,
    snapshot: &SettingsSnapshot,
) -> Result<(), AppError> {
    let settings_json = serde_json::to_string(&snapshot.settings)
        .map_err(|err| history_error(format!("序列化设置快照失败：{err}")))?;

    conn.execute(
        "INSERT OR REPLACE INTO settings_snapshots(
            settings_snapshot_id,
            captured_at,
            settings_json
        ) VALUES (?1, ?2, ?3)",
        params![snapshot.id, snapshot.captured_at, settings_json],
    )
    .map_err(|err| history_error(format!("写入设置快照失败：{err}")))?;

    Ok(())
}

pub fn create_batch(conn: &Connection, record: &BatchRecord) -> Result<(), AppError> {
    conn.execute(
        "INSERT OR REPLACE INTO batches(
            batch_id,
            created_at,
            status,
            settings_snapshot_id,
            total,
            output_created,
            pending,
            skipped,
            failed,
            cancelled
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            record.batch_id.0,
            record.created_at,
            enum_to_db_text(&record.status)?,
            record.settings_snapshot_id,
            usize_to_i64(record.summary.total)?,
            usize_to_i64(record.summary.output_created)?,
            usize_to_i64(record.summary.pending)?,
            usize_to_i64(record.summary.skipped)?,
            usize_to_i64(record.summary.failed)?,
            usize_to_i64(record.summary.cancelled)?,
        ],
    )
    .map_err(|err| history_error(format!("写入批次历史失败：{err}")))?;

    Ok(())
}

pub fn record_file_result(conn: &Connection, record: &FileResultRecord) -> Result<(), AppError> {
    delete_scoring_rows(conn, &record.file.file_job_id)?;

    let error_json = record
        .error
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|err| history_error(format!("序列化历史错误信息失败：{err}")))?;
    let pending_reason = record
        .file
        .pending_reason
        .as_ref()
        .map(enum_to_db_text)
        .transpose()?;
    let file_type = enum_to_db_text(&record.file.file_type)?;
    let status = enum_to_db_text(&record.file.status)?;
    let output_kind = record.output_kind.as_ref().map(OutputKind::as_str);
    let scoring_final_title = record
        .scoring_result
        .as_ref()
        .and_then(|result| result.final_title.clone());
    let scoring_confidence = record
        .scoring_result
        .as_ref()
        .map(|result| i64::from(result.confidence));
    let scoring_decision = record
        .scoring_result
        .as_ref()
        .map(|result| enum_to_db_text(&result.decision))
        .transpose()?;

    conn.execute(
        "INSERT OR REPLACE INTO file_results(
            file_job_id,
            batch_id,
            source_path,
            file_name,
            file_type,
            status,
            recognized_title,
            confidence,
            output_path,
            failure_reason,
            pending_reason,
            fingerprint_normalized_path,
            fingerprint_size_bytes,
            fingerprint_modified_time,
            scoring_final_title,
            scoring_confidence,
            scoring_decision,
            output_kind,
            error_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        params![
            record.file.file_job_id.0,
            record.file.batch_id.0,
            record.file.source_path,
            record.file.file_name,
            file_type,
            status,
            record.file.recognized_title,
            record.file.confidence.map(i64::from),
            record.file.output_path,
            record.file.failure_reason,
            pending_reason,
            record.source_fingerprint.normalized_path,
            u64_to_i64(record.source_fingerprint.size_bytes)?,
            record.source_fingerprint.modified_time,
            scoring_final_title,
            scoring_confidence,
            scoring_decision,
            output_kind,
            error_json,
        ],
    )
    .map_err(|err| history_error(format!("写入文件历史失败：{err}")))?;

    if let Some(scoring_result) = &record.scoring_result {
        insert_candidates(conn, &record.file.file_job_id, scoring_result)?;
    }

    Ok(())
}

pub fn list_history(
    conn: &Connection,
    offset: usize,
    limit: usize,
) -> Result<HistoryBatchPage, AppError> {
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM batches", [], |row| row.get(0))
        .map_err(|err| history_error(format!("查询批次总数失败：{err}")))?;
    let mut stmt = conn
        .prepare(
            "SELECT
                batch_id,
                created_at,
                status,
                settings_snapshot_id,
                total,
                output_created,
                pending,
                skipped,
                failed,
                cancelled
             FROM batches
             ORDER BY created_at DESC, batch_id DESC
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(|err| history_error(format!("准备批次列表查询失败：{err}")))?;
    let rows = stmt
        .query_map(
            params![usize_to_i64(limit)?, usize_to_i64(offset)?],
            |row| batch_row_from_row(row),
        )
        .map_err(|err| history_error(format!("查询批次列表失败：{err}")))?;

    let batches = collect_rows(rows)?;

    Ok(HistoryBatchPage {
        batches,
        total: i64_to_usize(total)?,
    })
}

pub fn get_history_batch(
    conn: &Connection,
    batch_id: &BatchId,
) -> Result<Option<HistoryBatchDetail>, AppError> {
    let batch = conn
        .query_row(
            "SELECT
                batch_id,
                created_at,
                status,
                settings_snapshot_id,
                total,
                output_created,
                pending,
                skipped,
                failed,
                cancelled
             FROM batches
             WHERE batch_id = ?1",
            [batch_id.0.as_str()],
            batch_row_from_row,
        )
        .optional()
        .map_err(|err| history_error(format!("查询批次详情失败：{err}")))?;

    let Some(batch) = batch else {
        return Ok(None);
    };

    let mut stmt = conn
        .prepare(
            "SELECT
                file_job_id,
                batch_id,
                source_path,
                file_name,
                file_type,
                status,
                recognized_title,
                confidence,
                output_path,
                failure_reason,
                pending_reason,
                fingerprint_normalized_path,
                fingerprint_size_bytes,
                fingerprint_modified_time,
                scoring_final_title,
                scoring_confidence,
                scoring_decision,
                error_json
             FROM file_results
             WHERE batch_id = ?1
             ORDER BY rowid ASC",
        )
        .map_err(|err| history_error(format!("准备文件历史查询失败：{err}")))?;
    let rows = stmt
        .query_map([batch_id.0.as_str()], |row| file_result_from_row(conn, row))
        .map_err(|err| history_error(format!("查询文件历史失败：{err}")))?;
    let files = collect_rows(rows)?;

    Ok(Some(HistoryBatchDetail {
        batch_id: batch.batch_id,
        created_at: batch.created_at,
        status: batch.status,
        settings_snapshot_id: batch.settings_snapshot_id,
        summary: batch.summary,
        files,
    }))
}

pub fn find_duplicate_by_fingerprint(
    conn: &Connection,
    fingerprint: &SourceFingerprint,
) -> Result<Option<DuplicateMatch>, AppError> {
    conn.query_row(
        "SELECT batch_id, file_job_id, output_path
         FROM file_results
         WHERE fingerprint_normalized_path = ?1
           AND fingerprint_size_bytes = ?2
           AND fingerprint_modified_time = ?3
           AND output_kind IN ('auto', 'manual')
         ORDER BY rowid DESC
         LIMIT 1",
        params![
            fingerprint.normalized_path,
            u64_to_i64(fingerprint.size_bytes)?,
            fingerprint.modified_time,
        ],
        |row| {
            Ok(DuplicateMatch {
                batch_id: BatchId(row.get(0)?),
                file_job_id: FileJobId(row.get(1)?),
                output_path: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(|err| history_error(format!("查询重复处理记录失败：{err}")))
}

pub fn record_undo_for_output(
    conn: &Connection,
    file_job_id: &FileJobId,
    output_path: &Path,
) -> Result<(), AppError> {
    let batch_id: String = conn
        .query_row(
            "SELECT batch_id FROM file_results WHERE file_job_id = ?1",
            [file_job_id.0.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|err| history_error(format!("查询撤销文件记录失败：{err}")))?
        .ok_or_else(|| history_error("无法为不存在的文件结果创建撤销记录。"))?;
    let metadata = fs::metadata(output_path).map_err(|err| {
        undo_error(
            ErrorCode::UndoOutputMissing,
            format!("输出副本不存在，无法记录撤销信息：{err}"),
        )
        .with_path(output_path.display().to_string())
    })?;
    let modified_time = system_time_to_millis_string(metadata.modified().map_err(|err| {
        history_error(format!("读取输出副本修改时间失败：{err}"))
            .with_path(output_path.display().to_string())
    })?)?;
    let sha256 = file_sha256(output_path)?;

    conn.execute(
        "INSERT OR REPLACE INTO undo_records(
            file_job_id,
            batch_id,
            output_path,
            created_size_bytes,
            created_modified_time,
            created_sha256,
            status,
            message,
            undone_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'ready', NULL, NULL)",
        params![
            file_job_id.0,
            batch_id,
            output_path.display().to_string(),
            u64_to_i64(metadata.len())?,
            modified_time,
            sha256,
        ],
    )
    .map_err(|err| history_error(format!("写入撤销记录失败：{err}")))?;

    Ok(())
}

pub fn undo_batch_outputs(conn: &Connection, batch_id: &BatchId) -> Result<UndoResult, AppError> {
    let records = load_ready_undo_records(conn, batch_id)?;
    let mut result = UndoResult {
        deleted: 0,
        skipped_missing: 0,
        skipped_modified: 0,
    };

    for record in records {
        let output_path = PathBuf::from(&record.output_path);
        if !output_path.exists() {
            result.skipped_missing += 1;
            update_undo_status(
                conn,
                &record.file_job_id,
                "missing",
                "输出副本不存在，已跳过撤销。",
            )?;
            continue;
        }

        if output_was_modified(&record, &output_path)? {
            result.skipped_modified += 1;
            update_undo_status(
                conn,
                &record.file_job_id,
                "modified",
                "输出副本已被改动，已跳过撤销。",
            )?;
            continue;
        }

        fs::remove_file(&output_path).map_err(|err| {
            history_error(format!("删除输出副本失败：{err}"))
                .with_path(output_path.display().to_string())
        })?;
        result.deleted += 1;
        update_undo_status(conn, &record.file_job_id, "deleted", "输出副本已删除。")?;
    }

    Ok(result)
}

#[derive(Debug)]
struct UndoRecord {
    file_job_id: FileJobId,
    output_path: String,
    created_size_bytes: u64,
    created_modified_time: String,
    created_sha256: String,
}

fn load_ready_undo_records(
    conn: &Connection,
    batch_id: &BatchId,
) -> Result<Vec<UndoRecord>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT
                file_job_id,
                output_path,
                created_size_bytes,
                created_modified_time,
                created_sha256
             FROM undo_records
             WHERE batch_id = ?1
               AND status = 'ready'
             ORDER BY undo_record_id ASC",
        )
        .map_err(|err| history_error(format!("准备撤销记录查询失败：{err}")))?;
    let rows = stmt
        .query_map([batch_id.0.as_str()], |row| {
            Ok(UndoRecord {
                file_job_id: FileJobId(row.get(0)?),
                output_path: row.get(1)?,
                created_size_bytes: i64_to_u64_sql(row.get(2)?)?,
                created_modified_time: row.get(3)?,
                created_sha256: row.get(4)?,
            })
        })
        .map_err(|err| history_error(format!("查询撤销记录失败：{err}")))?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|err| history_error(format!("读取撤销记录失败：{err}")))
}

fn output_was_modified(record: &UndoRecord, output_path: &Path) -> Result<bool, AppError> {
    let metadata = fs::metadata(output_path).map_err(|err| {
        undo_error(
            ErrorCode::UndoOutputMissing,
            format!("输出副本不存在，无法撤销：{err}"),
        )
        .with_path(output_path.display().to_string())
    })?;
    let current_modified = system_time_to_millis_string(metadata.modified().map_err(|err| {
        history_error(format!("读取输出副本修改时间失败：{err}"))
            .with_path(output_path.display().to_string())
    })?)?;

    if metadata.len() == record.created_size_bytes
        && current_modified == record.created_modified_time
    {
        return Ok(false);
    }

    Ok(file_sha256(output_path)? != record.created_sha256)
}

fn update_undo_status(
    conn: &Connection,
    file_job_id: &FileJobId,
    status: &str,
    message: &str,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE undo_records
         SET status = ?1,
             message = ?2,
             undone_at = ?3
         WHERE file_job_id = ?4",
        params![status, message, Utc::now().to_rfc3339(), file_job_id.0],
    )
    .map_err(|err| history_error(format!("更新撤销状态失败：{err}")))?;

    Ok(())
}

fn file_sha256(path: &Path) -> Result<String, AppError> {
    let mut file = fs::File::open(path).map_err(|err| {
        history_error(format!("读取输出副本用于哈希失败：{err}"))
            .with_path(path.display().to_string())
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = file.read(&mut buffer).map_err(|err| {
            history_error(format!("计算输出副本哈希失败：{err}"))
                .with_path(path.display().to_string())
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

fn system_time_to_millis_string(time: SystemTime) -> Result<String, AppError> {
    let duration = time
        .duration_since(UNIX_EPOCH)
        .map_err(|err| history_error(format!("文件修改时间早于 Unix epoch：{err}")))?;
    Ok(duration.as_millis().to_string())
}

fn delete_scoring_rows(conn: &Connection, file_job_id: &FileJobId) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM rule_details
         WHERE candidate_id IN (
             SELECT candidate_id FROM candidates WHERE file_job_id = ?1
         )",
        [file_job_id.0.as_str()],
    )
    .map_err(|err| history_error(format!("清理规则明细失败：{err}")))?;
    conn.execute(
        "DELETE FROM candidates WHERE file_job_id = ?1",
        [file_job_id.0.as_str()],
    )
    .map_err(|err| history_error(format!("清理候选历史失败：{err}")))?;
    Ok(())
}

fn insert_candidates(
    conn: &Connection,
    file_job_id: &FileJobId,
    scoring_result: &ScoringResult,
) -> Result<(), AppError> {
    for (candidate_order, candidate) in scoring_result.candidates.iter().enumerate() {
        conn.execute(
            "INSERT INTO candidates(
                file_job_id,
                candidate_order,
                text,
                source,
                page_index,
                paragraph_index,
                score,
                layout_score,
                position_score,
                keyword_score,
                text_quality_score,
                penalty_score
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                file_job_id.0,
                usize_to_i64(candidate_order)?,
                candidate.text,
                enum_to_db_text(&candidate.source)?,
                candidate.page_index.map(usize_to_i64).transpose()?,
                candidate.paragraph_index.map(usize_to_i64).transpose()?,
                i64::from(candidate.score),
                i64::from(candidate.category_scores.layout),
                i64::from(candidate.category_scores.position),
                i64::from(candidate.category_scores.keyword),
                i64::from(candidate.category_scores.text_quality),
                i64::from(candidate.category_scores.penalty),
            ],
        )
        .map_err(|err| history_error(format!("写入候选标题失败：{err}")))?;

        let candidate_id = conn.last_insert_rowid();
        for (rule_order, detail) in candidate.rule_details.iter().enumerate() {
            conn.execute(
                "INSERT INTO rule_details(
                    candidate_id,
                    rule_order,
                    rule_name,
                    category,
                    delta,
                    description
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    candidate_id,
                    usize_to_i64(rule_order)?,
                    detail.rule_name,
                    detail.category,
                    i64::from(detail.delta),
                    detail.description,
                ],
            )
            .map_err(|err| history_error(format!("写入规则明细失败：{err}")))?;
        }
    }

    Ok(())
}

fn batch_row_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryBatchRow> {
    Ok(HistoryBatchRow {
        batch_id: BatchId(row.get(0)?),
        created_at: row.get(1)?,
        status: enum_from_db_text(&row.get::<_, String>(2)?)?,
        settings_snapshot_id: row.get(3)?,
        summary: BatchSummary {
            total: i64_to_usize_sql(row.get(4)?)?,
            output_created: i64_to_usize_sql(row.get(5)?)?,
            pending: i64_to_usize_sql(row.get(6)?)?,
            skipped: i64_to_usize_sql(row.get(7)?)?,
            failed: i64_to_usize_sql(row.get(8)?)?,
            cancelled: i64_to_usize_sql(row.get(9)?)?,
        },
    })
}

fn file_result_from_row(
    conn: &Connection,
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<HistoryFileResult> {
    let file_job_id = FileJobId(row.get(0)?);
    let batch_id = BatchId(row.get(1)?);
    let file = FileJobView {
        file_job_id: file_job_id.clone(),
        batch_id,
        source_path: row.get(2)?,
        file_name: row.get(3)?,
        file_type: enum_from_db_text(&row.get::<_, String>(4)?)?,
        status: enum_from_db_text(&row.get::<_, String>(5)?)?,
        recognized_title: row.get(6)?,
        confidence: row
            .get::<_, Option<i64>>(7)?
            .map(i64_to_u8_sql)
            .transpose()?,
        output_path: row.get(8)?,
        failure_reason: row.get(9)?,
        pending_reason: row
            .get::<_, Option<String>>(10)?
            .map(|text| enum_from_db_text(&text))
            .transpose()?,
    };
    let source_fingerprint = SourceFingerprint {
        normalized_path: row.get(11)?,
        size_bytes: i64_to_u64_sql(row.get(12)?)?,
        modified_time: row.get(13)?,
    };
    let scoring_final_title: Option<String> = row.get(14)?;
    let scoring_confidence: Option<i64> = row.get(15)?;
    let scoring_decision: Option<String> = row.get(16)?;
    let error_json: Option<String> = row.get(17)?;
    let candidates = load_candidates(conn, &file_job_id)?;
    let scoring_result = match scoring_decision {
        Some(decision) => Some(ScoringResult {
            final_title: scoring_final_title,
            confidence: scoring_confidence
                .map(i64_to_u8_sql)
                .transpose()?
                .unwrap_or(0),
            candidates,
            decision: enum_from_db_text(&decision)?,
        }),
        None => None,
    };
    let error = error_json
        .map(|json| {
            serde_json::from_str(&json).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    17,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })
        })
        .transpose()?;

    Ok(HistoryFileResult {
        file,
        source_fingerprint,
        scoring_result,
        error,
    })
}

fn load_candidates(
    conn: &Connection,
    file_job_id: &FileJobId,
) -> rusqlite::Result<Vec<CandidateTitle>> {
    let mut stmt = conn.prepare(
        "SELECT
            candidate_id,
            text,
            source,
            page_index,
            paragraph_index,
            score,
            layout_score,
            position_score,
            keyword_score,
            text_quality_score,
            penalty_score
         FROM candidates
         WHERE file_job_id = ?1
         ORDER BY candidate_order ASC",
    )?;
    let rows = stmt.query_map([file_job_id.0.as_str()], |row| {
        let candidate_id: i64 = row.get(0)?;
        Ok(CandidateTitle {
            text: row.get(1)?,
            source: enum_from_db_text::<CandidateSource>(&row.get::<_, String>(2)?)?,
            page_index: row
                .get::<_, Option<i64>>(3)?
                .map(i64_to_usize_sql)
                .transpose()?,
            paragraph_index: row
                .get::<_, Option<i64>>(4)?
                .map(i64_to_usize_sql)
                .transpose()?,
            score: i64_to_u8_sql(row.get(5)?)?,
            category_scores: CategoryScores {
                layout: i64_to_i16_sql(row.get(6)?)?,
                position: i64_to_i16_sql(row.get(7)?)?,
                keyword: i64_to_i16_sql(row.get(8)?)?,
                text_quality: i64_to_i16_sql(row.get(9)?)?,
                penalty: i64_to_i16_sql(row.get(10)?)?,
            },
            rule_details: load_rule_details(conn, candidate_id)?,
        })
    })?;

    rows.collect()
}

fn load_rule_details(conn: &Connection, candidate_id: i64) -> rusqlite::Result<Vec<RuleDetail>> {
    let mut stmt = conn.prepare(
        "SELECT rule_name, category, delta, description
         FROM rule_details
         WHERE candidate_id = ?1
         ORDER BY rule_order ASC",
    )?;
    let rows = stmt.query_map([candidate_id], |row| {
        Ok(RuleDetail {
            rule_name: row.get(0)?,
            category: row.get(1)?,
            delta: i64_to_i16_sql(row.get(2)?)?,
            description: row.get(3)?,
        })
    })?;

    rows.collect()
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>, AppError> {
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|err| history_error(format!("读取历史数据失败：{err}")))
}

fn enum_to_db_text<T: Serialize>(value: &T) -> Result<String, AppError> {
    enum_to_db_text_raw(value).map_err(|err| history_error(format!("序列化历史枚举失败：{err}")))
}

fn enum_to_db_text_raw<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let json_value = serde_json::to_value(value)?;
    Ok(match json_value {
        serde_json::Value::String(text) => text,
        other => other.to_string(),
    })
}

fn enum_from_db_text<T: DeserializeOwned>(text: &str) -> rusqlite::Result<T> {
    serde_json::from_value(serde_json::Value::String(text.to_string())).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })
}

fn u64_to_i64(value: u64) -> Result<i64, AppError> {
    i64::try_from(value).map_err(|_| history_error("历史记录数值超出 SQLite INTEGER 范围。"))
}

fn usize_to_i64(value: usize) -> Result<i64, AppError> {
    i64::try_from(value).map_err(|_| history_error("历史记录数值超出 SQLite INTEGER 范围。"))
}

fn i64_to_usize(value: i64) -> Result<usize, AppError> {
    usize::try_from(value).map_err(|_| history_error("历史记录数值无法转换。"))
}

fn i64_to_usize_sql(value: i64) -> rusqlite::Result<usize> {
    usize::try_from(value).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Integer, Box::new(err))
    })
}

fn i64_to_u64_sql(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Integer, Box::new(err))
    })
}

fn i64_to_u8_sql(value: i64) -> rusqlite::Result<u8> {
    u8::try_from(value).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Integer, Box::new(err))
    })
}

fn i64_to_i16_sql(value: i64) -> rusqlite::Result<i16> {
    i16::try_from(value).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Integer, Box::new(err))
    })
}

fn history_error(message: impl Into<String>) -> AppError {
    AppError {
        code: ErrorCode::Internal,
        category: ErrorCategory::History,
        user_message: message.into(),
        technical_detail: None,
        retryable: false,
        file_path: None,
        stage: Some(ProcessingStage::History),
    }
}

fn undo_error(code: ErrorCode, message: impl Into<String>) -> AppError {
    AppError {
        code,
        category: ErrorCategory::History,
        user_message: message.into(),
        technical_detail: None,
        retryable: false,
        file_path: None,
        stage: Some(ProcessingStage::Undo),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::{AppError, ErrorCategory, ErrorCode, ProcessingStage};
    use crate::models::{
        BatchId, BatchStatus, CandidateSource, CandidateTitle, CategoryScores, FileJobId,
        FileJobView, FileStatus, FileType, HistoryBatchDetail, PendingReason, RuleDetail,
        ScoreDecision, ScoringResult, Settings, SourceFingerprint,
    };
    use crate::settings::create_settings_snapshot;

    #[test]
    fn history_initializes_sqlite_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = history_path(dir.path());

        assert_eq!(path, dir.path().join("history.sqlite"));

        let conn = open_history(dir.path()).unwrap();
        assert!(path.exists());

        let version: i64 = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 1);

        for table in [
            "batches",
            "file_results",
            "candidates",
            "rule_details",
            "undo_records",
            "settings_snapshots",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing table {table}");
        }
    }

    #[test]
    fn records_history_and_reads_detail() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_history(dir.path()).unwrap();
        let snapshot = create_settings_snapshot(&Settings::default());
        let batch_id = BatchId("batch-001".into());
        let file_job_id = FileJobId("file-001".into());
        let scoring = scoring_result_fixture();
        let error = history_file_error_fixture();

        save_settings_snapshot(&conn, &snapshot).unwrap();
        create_batch(
            &conn,
            &BatchRecord {
                batch_id: batch_id.clone(),
                created_at: "2026-06-26T10:00:00Z".into(),
                status: BatchStatus::Completed,
                settings_snapshot_id: snapshot.id.clone(),
                summary: summary_fixture(),
            },
        )
        .unwrap();
        record_file_result(
            &conn,
            &FileResultRecord {
                file: file_view_fixture(batch_id.clone(), file_job_id.clone()),
                source_fingerprint: fingerprint_fixture(),
                scoring_result: Some(scoring.clone()),
                error: Some(error.clone()),
                output_kind: Some(OutputKind::Auto),
            },
        )
        .unwrap();

        let page = list_history(&conn, 0, 10).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.batches[0].batch_id, batch_id);
        assert_eq!(page.batches[0].summary.output_created, 1);

        let detail = get_history_batch(&conn, &BatchId("batch-001".into()))
            .unwrap()
            .unwrap();

        assert_detail_matches_record(detail, snapshot.id, scoring, error);
    }

    #[test]
    fn duplicate_detection_only_uses_output_records() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_history(dir.path()).unwrap();
        let snapshot = create_settings_snapshot(&Settings::default());
        save_settings_snapshot(&conn, &snapshot).unwrap();
        create_batch(
            &conn,
            &BatchRecord {
                batch_id: BatchId("batch-duplicate".into()),
                created_at: "2026-06-26T10:00:00Z".into(),
                status: BatchStatus::Completed,
                settings_snapshot_id: snapshot.id,
                summary: crate::models::BatchSummary {
                    total: 3,
                    output_created: 2,
                    pending: 1,
                    skipped: 0,
                    failed: 0,
                    cancelled: 0,
                },
            },
        )
        .unwrap();

        let fingerprint = fingerprint_fixture();
        record_file_result(
            &conn,
            &FileResultRecord {
                file: pending_file_view_fixture(
                    BatchId("batch-duplicate".into()),
                    FileJobId("file-pending".into()),
                ),
                source_fingerprint: fingerprint.clone(),
                scoring_result: None,
                error: None,
                output_kind: None,
            },
        )
        .unwrap();

        assert!(find_duplicate_by_fingerprint(&conn, &fingerprint)
            .unwrap()
            .is_none());

        let mut auto_output = file_view_fixture(
            BatchId("batch-duplicate".into()),
            FileJobId("file-auto".into()),
        );
        auto_output.output_path = Some("/input/Rustitler 输出/劳动合同.pdf".into());
        record_file_result(
            &conn,
            &FileResultRecord {
                file: auto_output,
                source_fingerprint: fingerprint.clone(),
                scoring_result: None,
                error: None,
                output_kind: Some(OutputKind::Auto),
            },
        )
        .unwrap();

        let duplicate = find_duplicate_by_fingerprint(&conn, &fingerprint)
            .unwrap()
            .unwrap();
        assert_eq!(duplicate.batch_id, BatchId("batch-duplicate".into()));
        assert_eq!(duplicate.file_job_id, FileJobId("file-auto".into()));
        assert_eq!(
            duplicate.output_path.as_deref(),
            Some("/input/Rustitler 输出/劳动合同.pdf")
        );

        let manual_fingerprint = SourceFingerprint {
            normalized_path: "/input/手动.docx".into(),
            size_bytes: 33,
            modified_time: "2026-06-26T09:10:00Z".into(),
        };
        let mut manual_output = file_view_fixture(
            BatchId("batch-duplicate".into()),
            FileJobId("file-manual".into()),
        );
        manual_output.source_path = "/input/手动.docx".into();
        manual_output.file_name = "手动.docx".into();
        manual_output.file_type = FileType::Docx;
        manual_output.output_path = Some("/input/Rustitler 输出/手动.docx".into());
        record_file_result(
            &conn,
            &FileResultRecord {
                file: manual_output,
                source_fingerprint: manual_fingerprint.clone(),
                scoring_result: None,
                error: None,
                output_kind: Some(OutputKind::Manual),
            },
        )
        .unwrap();

        let manual_duplicate = find_duplicate_by_fingerprint(&conn, &manual_fingerprint)
            .unwrap()
            .unwrap();
        assert_eq!(
            manual_duplicate.file_job_id,
            FileJobId("file-manual".into())
        );
    }

    #[test]
    fn undo_batch_deletes_only_unchanged_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_history(dir.path()).unwrap();
        let snapshot = create_settings_snapshot(&Settings::default());
        let batch_id = BatchId("batch-undo".into());
        save_settings_snapshot(&conn, &snapshot).unwrap();
        create_batch(
            &conn,
            &BatchRecord {
                batch_id: batch_id.clone(),
                created_at: "2026-06-26T11:00:00Z".into(),
                status: BatchStatus::Completed,
                settings_snapshot_id: snapshot.id,
                summary: crate::models::BatchSummary {
                    total: 3,
                    output_created: 3,
                    pending: 0,
                    skipped: 0,
                    failed: 0,
                    cancelled: 0,
                },
            },
        )
        .unwrap();

        let unchanged_path = dir.path().join("输出").join("unchanged.pdf");
        let modified_path = dir.path().join("输出").join("modified.pdf");
        let missing_path = dir.path().join("输出").join("missing.pdf");
        std::fs::create_dir_all(unchanged_path.parent().unwrap()).unwrap();
        std::fs::write(&unchanged_path, b"unchanged").unwrap();
        std::fs::write(&modified_path, b"original").unwrap();
        std::fs::write(&missing_path, b"gone").unwrap();

        for (file_id, output_path) in [
            ("file-unchanged", unchanged_path.clone()),
            ("file-modified", modified_path.clone()),
            ("file-missing", missing_path.clone()),
        ] {
            let mut file = file_view_fixture(batch_id.clone(), FileJobId(file_id.into()));
            file.output_path = Some(output_path.display().to_string());
            record_file_result(
                &conn,
                &FileResultRecord {
                    file,
                    source_fingerprint: fingerprint_fixture(),
                    scoring_result: None,
                    error: None,
                    output_kind: Some(OutputKind::Auto),
                },
            )
            .unwrap();
            record_undo_for_output(&conn, &FileJobId(file_id.into()), &output_path).unwrap();
        }

        std::fs::write(&modified_path, b"user changed").unwrap();
        std::fs::remove_file(&missing_path).unwrap();

        let result = undo_batch_outputs(&conn, &batch_id).unwrap();

        assert_eq!(result.deleted, 1);
        assert_eq!(result.skipped_missing, 1);
        assert_eq!(result.skipped_modified, 1);
        assert!(!unchanged_path.exists());
        assert!(modified_path.exists());
        assert!(!missing_path.exists());

        let statuses = undo_statuses(&conn);
        assert!(statuses.contains(&("file-unchanged".into(), "deleted".into())));
        assert!(statuses.contains(&("file-modified".into(), "modified".into())));
        assert!(statuses.contains(&("file-missing".into(), "missing".into())));
    }

    fn assert_detail_matches_record(
        detail: HistoryBatchDetail,
        settings_snapshot_id: String,
        scoring: ScoringResult,
        error: AppError,
    ) {
        assert_eq!(detail.batch_id, BatchId("batch-001".into()));
        assert_eq!(detail.created_at, "2026-06-26T10:00:00Z");
        assert_eq!(detail.settings_snapshot_id, settings_snapshot_id);
        assert_eq!(detail.summary.total, 1);
        assert_eq!(detail.files.len(), 1);

        let file = &detail.files[0];
        assert_eq!(file.file.file_job_id, FileJobId("file-001".into()));
        assert_eq!(file.file.status, FileStatus::OutputCreated);
        assert_eq!(file.source_fingerprint.normalized_path, "/input/合同.pdf");
        assert_eq!(
            file.scoring_result.as_ref().unwrap().final_title,
            scoring.final_title
        );
        assert_eq!(
            file.scoring_result.as_ref().unwrap().candidates[0].rule_details[0].rule_name,
            "keyword-default"
        );
        assert_eq!(file.error.as_ref().unwrap().code, error.code);
    }

    fn summary_fixture() -> crate::models::BatchSummary {
        crate::models::BatchSummary {
            total: 1,
            output_created: 1,
            pending: 0,
            skipped: 0,
            failed: 0,
            cancelled: 0,
        }
    }

    fn file_view_fixture(batch_id: BatchId, file_job_id: FileJobId) -> FileJobView {
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
            pending_reason: Some(PendingReason::LowConfidence),
        }
    }

    fn pending_file_view_fixture(batch_id: BatchId, file_job_id: FileJobId) -> FileJobView {
        FileJobView {
            file_job_id,
            batch_id,
            source_path: "/input/合同.pdf".into(),
            file_name: "合同.pdf".into(),
            file_type: FileType::Pdf,
            status: FileStatus::Pending,
            recognized_title: None,
            confidence: Some(45),
            output_path: None,
            failure_reason: Some("可能已处理过".into()),
            pending_reason: Some(PendingReason::DuplicateSuspected),
        }
    }

    fn fingerprint_fixture() -> SourceFingerprint {
        SourceFingerprint {
            normalized_path: "/input/合同.pdf".into(),
            size_bytes: 2048,
            modified_time: "2026-06-26T09:00:00Z".into(),
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

    fn history_file_error_fixture() -> AppError {
        AppError {
            code: ErrorCode::ConfidenceBelowThreshold,
            category: ErrorCategory::Scoring,
            user_message: "置信度低于阈值。".into(),
            technical_detail: Some("threshold 95".into()),
            retryable: false,
            file_path: Some("/input/合同.pdf".into()),
            stage: Some(ProcessingStage::Score),
        }
    }

    fn undo_statuses(conn: &Connection) -> Vec<(String, String)> {
        let mut stmt = conn
            .prepare(
                "SELECT file_job_id, status
                 FROM undo_records
                 ORDER BY file_job_id ASC",
            )
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }
}
