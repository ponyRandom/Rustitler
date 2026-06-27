import { beforeEach, describe, expect, it, vi } from "vitest";
import { appError, batchState, defaultSettings, fileView, scoringResult } from "../test/fixtures";

vi.mock("../api/commands", () => ({
  startBatch: vi.fn(),
  cancelBatch: vi.fn(),
  getBatchState: vi.fn(),
  confirmPendingOutput: vi.fn(),
}));

describe("batch store", () => {
  beforeEach(() => {
    vi.resetModules();
  });

  it("merges batch events into a frontend state mirror", async () => {
    const { createBatchStore } = await import("./batchStore");
    const store = createBatchStore();

    store.applyEvent({ type: "BatchStarted", batchId: "batch-1", createdAt: "now", totalFiles: 2 });
    store.applyEvent({ type: "FileQueued", batchId: "batch-1", file: fileView({ fileJobId: "file-1" }) });
    store.applyEvent({
      type: "FileScored",
      batchId: "batch-1",
      fileJobId: "file-1",
      result: scoringResult({ finalTitle: "项目通知", confidence: 88 }),
    });
    store.applyEvent({
      type: "FilePending",
      batchId: "batch-1",
      fileJobId: "file-1",
      reason: "lowConfidence",
      suggestion: "项目通知",
    });
    store.applyEvent({
      type: "FileFailed",
      batchId: "batch-1",
      fileJobId: "file-2",
      error: appError({ userMessage: "提取失败" }),
    });
    store.applyEvent({
      type: "BatchCompleted",
      batchId: "batch-1",
      summary: {
        total: 2,
        outputCreated: 0,
        pending: 1,
        skipped: 0,
        failed: 1,
        cancelled: 0,
      },
    });

    expect(store.getState().batch?.status).toBe("completed");
    expect(store.getState().files).toHaveLength(2);
    expect(store.getState().files[0]).toMatchObject({
      fileJobId: "file-1",
      status: "pending",
      recognizedTitle: "项目通知",
      confidence: 88,
      pendingReason: "lowConfidence",
    });
    expect(store.getState().files[1]).toMatchObject({
      fileJobId: "file-2",
      status: "failed",
      failureReason: "提取失败",
    });
  });

  it("starts batches, refreshes snapshots, confirms pending names, and cancels", async () => {
    const commands = await import("../api/commands");
    vi.mocked(commands.startBatch).mockResolvedValue("batch-1");
    vi.mocked(commands.getBatchState).mockResolvedValue(
      batchState({
        files: [fileView({ fileJobId: "file-1", status: "pending", pendingReason: "lowConfidence" })],
      }),
    );
    vi.mocked(commands.confirmPendingOutput).mockResolvedValue(
      fileView({
        fileJobId: "file-1",
        status: "outputCreated",
        recognizedTitle: "手动标题",
        outputPath: "/input/Rustitler 输出/手动标题.pdf",
      }),
    );
    const { createBatchStore } = await import("./batchStore");
    const store = createBatchStore();

    await store.start(["/input/a.pdf"], defaultSettings());
    await store.refresh();
    await store.confirmPending("file-1", "手动标题");
    await store.cancel();

    expect(commands.startBatch).toHaveBeenCalledWith(["/input/a.pdf"], defaultSettings());
    expect(commands.getBatchState).toHaveBeenCalledWith("batch-1");
    expect(commands.confirmPendingOutput).toHaveBeenCalledWith("file-1", "手动标题");
    expect(commands.cancelBatch).toHaveBeenCalledWith("batch-1");
    expect(store.getState().files[0].status).toBe("outputCreated");
  });
});
