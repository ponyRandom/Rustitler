# Batch Scheduler Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the batch scheduler module described by `doc/tasks/batch-scheduler.md`.

**Architecture:** Keep the scheduler testable and synchronous at its core: it owns a `BatchRuntimeState`, emits `BatchEvent`s through an injected sink, and calls injected extraction/output/history dependencies so tests can use deterministic fakes. The module starts a batch by scanning inputs, persisting initial history, processing queued files through extract -> score -> output/pending/failure transitions, recording each result, and exposing cancellation and state snapshots.

**Tech Stack:** Rust standard library synchronization primitives, `uuid`, `chrono`, existing `models.rs` DTOs, `ingest`, `extract`, `scoring`, `rename`, `history`, and `rusqlite` for integration-style persistence tests.

---

### Task 1: Runtime State and Batch Initialization

**Files:**
- Modify: `src-tauri/src/batch_scheduler.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving `start_batch_with_services` rejects an empty path list, creates a batch state with a generated batch id, stores a settings snapshot, emits `BatchStarted` before `FileQueued`, initializes skipped unsupported entries, and exposes the snapshot through `get_batch_state`.

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml batch_scheduler::tests::start_batch_initializes_state_history_and_queue_events`

Expected: FAIL because scheduler APIs are not implemented.

- [ ] **Step 3: Implement state and initialization**

Define `BatchScheduler`, `BatchSchedulerServices`, `HistoryStore`, `EventSink`, `BatchRuntimeState`, `BatchRunResult`, and `start_batch_with_services`. Use `ingest::scan_inputs`, `settings::create_settings_snapshot`, `history::create_batch`, and `history::save_settings_snapshot`. Emit `BatchStarted` then one `FileQueued` for each scanned job. Keep state in memory by batch id.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml batch_scheduler::tests::start_batch_initializes_state_history_and_queue_events`

Expected: PASS.

### Task 2: File Processing Flow and History Writes

**Files:**
- Modify: `src-tauri/src/batch_scheduler.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving queued supported files emit extract and score progress, record `FileExtracted` and `FileScored`, auto-output high-confidence results through the output service, mark low-confidence results pending, keep unsupported and duplicate jobs out of extraction, write file results to history, and create undo records for auto outputs.

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml batch_scheduler::tests::start_batch_processes_auto_pending_skipped_and_duplicate_files`

Expected: FAIL because file processing is not implemented.

- [ ] **Step 3: Implement processing transitions**

Add injected `Extractor` and `OutputCreator` traits. For queued files, mark status `Analyzing`, emit `FileProgress` for extract/score/rename/history stages, call extraction, score with `ScoringProfile::from(&settings)`, and branch on `ScoreDecision`: `AutoOutput` copies output and writes undo metadata, `Pending` records `PendingReason::LowConfidence`, `Failed` records `PendingReason::ExtractionFailed`. Skipped and duplicate-pending jobs are recorded without extraction. Always update the in-memory state and write `history::FileResultRecord`.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml batch_scheduler::tests::start_batch_processes_auto_pending_skipped_and_duplicate_files`

Expected: PASS.

### Task 3: Error Isolation, Cancellation, and Final Events

**Files:**
- Modify: `src-tauri/src/batch_scheduler.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving one file extraction/output/history error emits `FileFailed` and does not stop other files, `cancel_batch` marks remaining queued/analyzing files cancelled, cancelled batches emit `BatchCancelled`, completed batches emit `BatchCompleted`, and `get_batch_state` returns current summary counts.

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml batch_scheduler::tests::file_failure_does_not_stop_batch batch_scheduler::tests::cancel_batch_updates_state_and_emits_cancelled`

Expected: FAIL because error isolation and cancellation are not implemented.

- [ ] **Step 3: Implement cancellation and finalization**

Add a cancellation flag per batch, `cancel_batch`, `get_batch_state`, summary recomputation, and final-event publishing. Check the cancellation flag before starting each file and after each major stage. Convert per-file errors into failed jobs and keep processing unless cancellation was requested. Persist a final batch record with `Completed` or `Cancelled` status after processing.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml batch_scheduler::tests`

Expected: PASS.

### Task 4: Documentation Status and Full Verification

**Files:**
- Modify: `doc/tasks/batch-scheduler.md`
- Modify: `doc/tasks/progress.md`

- [ ] **Step 1: Mark batch scheduler tasks complete**

Update `doc/tasks/batch-scheduler.md` BS-01 through BS-21 from `[ ]` to `[x]`.

- [ ] **Step 2: Update progress**

Update `doc/tasks/progress.md`: mark `batch-scheduler` complete, set current module to `commands`, add a current-state bullet for `batch-scheduler`, and set the next module to `commands`.

- [ ] **Step 3: Run targeted scheduler tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml batch_scheduler::tests`

Expected: PASS.

- [ ] **Step 4: Run full Rust tests and frontend build**

Run: `cargo test --manifest-path src-tauri/Cargo.toml && npm run build`

Expected: PASS.
