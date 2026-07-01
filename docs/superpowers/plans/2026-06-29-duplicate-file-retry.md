# Duplicate File Retry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let suspected duplicate files stay pending and be manually renamed for output.

**Architecture:** The Rust ingest layer already creates duplicate jobs as `pending` with `pendingReason: duplicateSuspected`. The fix is in the React state mirror and detail panel: progress metadata must not overwrite actionable statuses, and duplicate pending rows must use the existing manual confirmation form.

**Tech Stack:** Tauri IPC DTOs, React, TypeScript, Vitest, Testing Library.

---

### Task 1: Preserve Pending Status During Progress

**Files:**
- Modify: `src/stores/batchStore.ts`
- Test: `src/stores/batchStore.test.ts`

- [ ] **Step 1: Write the failing store test**

Add this test to `src/stores/batchStore.test.ts`:

```ts
it("keeps duplicate pending files pending when progress events arrive", async () => {
  const { createBatchStore } = await import("./batchStore");
  const store = createBatchStore();

  store.applyEvent({ type: "BatchStarted", batchId: "batch-1", createdAt: "now", totalFiles: 1 });
  store.applyEvent({
    type: "FileQueued",
    batchId: "batch-1",
    file: fileView({
      fileJobId: "file-1",
      status: "pending",
      pendingReason: "duplicateSuspected",
      failureReason: "疑似重复：历史批次 batch-old 的文件 file-old 已输出到 /output/a.pdf。",
    }),
  });
  store.applyEvent({
    type: "FileProgress",
    batchId: "batch-1",
    fileJobId: "file-1",
    stage: "history",
    progress: 0,
  });

  expect(store.getState().files[0]).toMatchObject({
    status: "pending",
    pendingReason: "duplicateSuspected",
    progressStage: "history",
    progress: 0,
  });
});
```

- [ ] **Step 2: Run the failing store test**

Run: `npm test -- src/stores/batchStore.test.ts --run`

Expected before implementation: the new test fails because `FileProgress` changes status to `analyzing`.

- [ ] **Step 3: Implement the minimal status guard**

In `src/stores/batchStore.ts`, replace the `FileProgress` updater with:

```ts
const shouldMarkAnalyzing = file.status === "queued" || file.status === "analyzing";
return {
  ...file,
  status: shouldMarkAnalyzing ? "analyzing" : file.status,
  progressStage: event.stage,
  progress: event.progress,
};
```

- [ ] **Step 4: Re-run the store test**

Run: `npm test -- src/stores/batchStore.test.ts --run`

Expected: all tests in the file pass.

### Task 2: Show Manual Confirmation For Duplicate Pending Files

**Files:**
- Modify: `src/App.tsx`
- Test: `src/App.test.tsx`

- [ ] **Step 1: Write the failing UI test**

Add a test that renders a selected duplicate pending file and asserts the manual filename input and confirm button are visible. Use the existing app test patterns and a `FileQueued` event with:

```ts
fileView({
  fileJobId: "file-duplicate",
  fileName: "2.pdf",
  sourcePath: "/Users/example/Desktop/2.pdf",
  status: "pending",
  pendingReason: "duplicateSuspected",
  failureReason: "疑似重复：历史批次 batch-old 的文件 file-old 已输出到 /Users/example/Desktop/Rustitler 输出/旧标题.pdf。",
})
```

Assert that `screen.getByLabelText("文件名主体")` has value `"2"` and `screen.getByRole("button", { name: "确认输出" })` exists.

- [ ] **Step 2: Run the failing UI test**

Run: `npm test -- src/App.test.tsx --run`

Expected before implementation if the current UI does not show the form for duplicate pending items: the new test fails.

- [ ] **Step 3: Implement the UI condition**

If needed, extract a helper near `fileExtension`:

```ts
const canConfirmManualOutput = (file: FileUiState) => file.status === "pending";
```

Then render the manual output form with `canConfirmManualOutput(file)` so duplicate pending files use the same form.

- [ ] **Step 4: Re-run the UI test**

Run: `npm test -- src/App.test.tsx --run`

Expected: all tests in the file pass.

### Task 3: Full Verification

**Files:**
- No additional source changes.

- [ ] **Step 1: Run targeted tests**

Run:

```bash
npm test -- src/stores/batchStore.test.ts src/App.test.tsx --run
```

Expected: both test files pass.

- [ ] **Step 2: Run typecheck**

Run:

```bash
npm run typecheck
```

Expected: TypeScript completes without errors.

---

## Revision: Duplicate Warnings Do Not Block Processing

**Goal:** Treat duplicate detection as a warning only. Duplicate files should keep the same processing path as new files, including extraction, scoring, auto output, and history writes.

**Architecture:** Move duplicate metadata out of `failureReason`/`pendingReason` and into a dedicated `duplicateWarning` field on file jobs. Ingest marks supported duplicate files as `queued`; the scheduler processes them normally; the frontend renders the warning as a neutral detail instead of a failure reason or manual confirmation state.

### Task 4: Backend Duplicate Warning Semantics

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/ingest.rs`
- Modify: `src-tauri/src/batch_scheduler.rs`

- [ ] **Step 1: Add failing backend tests**

Update existing ingest/scheduler duplicate tests so a supported duplicate file is queued and is later auto-processed.

- [ ] **Step 2: Verify backend tests fail**

Run:

```bash
cargo test scan_inputs_marks_supported_duplicate_as_queued_with_warning start_batch_processes_auto_pending_skipped_and_duplicate_files
```

- [ ] **Step 3: Add `duplicate_warning` to job/view models**

Expose the new field through serde as `duplicateWarning`.

- [ ] **Step 4: Queue duplicate supported files**

Set `status = Queued`, keep `pending_reason = None`, `failure_reason = None`, and populate `duplicate_warning`.

- [ ] **Step 5: Preserve warning through output creation and history**

Do not clear `duplicate_warning` when output is created.

- [ ] **Step 6: Verify backend tests pass**

Run the same `cargo test` command and confirm both tests pass.

### Task 5: Frontend Duplicate Warning Display

**Files:**
- Modify: `src/types/ipc.ts`
- Modify: `src/stores/batchStore.ts`
- Modify: `src/stores/batchStore.test.ts`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/test/fixtures.ts`

- [ ] **Step 1: Add failing frontend tests**

Update store/UI tests to expect duplicate files as queued/analyzing/output-created with `duplicateWarning`, no pending form, and no failure reason.

- [ ] **Step 2: Verify frontend tests fail**

Run:

```bash
npm test -- src/stores/batchStore.test.ts src/App.test.tsx --run
```

- [ ] **Step 3: Add `duplicateWarning` frontend types and fixtures**

Add the optional field to `FileJobView` and `FileJob`.

- [ ] **Step 4: Render duplicate warning separately**

Show the warning in the table/detail as `疑似重复` context, not under `失败原因`; keep the pending form gated on real `status === "pending"`.

- [ ] **Step 5: Verify frontend tests pass**

Run the same focused frontend test command and confirm both files pass.

### Task 6: Build and Install

**Files:**
- None

- [ ] **Step 1: Run full verification**

Run:

```bash
npm test
```

- [ ] **Step 2: Build release app**

Run:

```bash
npm run tauri:build:offline
```

- [ ] **Step 3: Install the rebuilt app**

Quit Rustitler, copy `src-tauri/target/release/bundle/macos/Rustitler.app` to `/Applications/Rustitler.app`, clear quarantine, and relaunch.
