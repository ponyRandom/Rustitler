import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import App from "./App";

vi.mock("./api/commands", () => ({
  startBatch: vi.fn().mockResolvedValue("batch-1"),
  cancelBatch: vi.fn().mockResolvedValue(undefined),
  getBatchState: vi.fn().mockResolvedValue(null),
  confirmPendingOutput: vi.fn().mockResolvedValue({}),
  undoBatch: vi.fn().mockResolvedValue({ deleted: 1, skippedMissing: 0, skippedModified: 0 }),
  listHistory: vi.fn().mockResolvedValue({
    total: 1,
    batches: [
      {
        batchId: "batch-1",
        createdAt: "2026-06-27T01:00:00Z",
        status: "completed",
        settingsSnapshotId: "settings-1",
        summary: {
          total: 1,
          outputCreated: 1,
          pending: 0,
          skipped: 0,
          failed: 0,
          cancelled: 0,
        },
      },
    ],
  }),
  getHistoryBatch: vi.fn().mockResolvedValue({
    batchId: "batch-1",
    createdAt: "2026-06-27T01:00:00Z",
    status: "completed",
    settingsSnapshotId: "settings-1",
    summary: {
      total: 1,
      outputCreated: 1,
      pending: 0,
      skipped: 0,
      failed: 0,
      cancelled: 0,
    },
    files: [
      {
        file: {
          fileJobId: "file-1",
          batchId: "batch-1",
          sourcePath: "/input/source.pdf",
          fileName: "source.pdf",
          fileType: "pdf",
          status: "outputCreated",
          recognizedTitle: "项目通知",
          confidence: 84,
          outputPath: "/input/Rustitler 输出/项目通知.pdf",
        },
        sourceFingerprint: {
          normalizedPath: "/input/source.pdf",
          sizeBytes: 2048,
          modifiedTime: "2026-06-27T00:00:00Z",
        },
      },
    ],
  }),
  loadSettings: vi.fn().mockResolvedValue({
    version: 1,
    autoOutputThreshold: 70,
    layoutSensitivity: 1,
    positionSensitivity: 1,
    keywordSensitivity: 1,
    textQualitySensitivity: 1,
    ocrConservatism: 1,
    keywordRules: [{ keyword: "通知", scoreDelta: 5 }],
    regexRules: [],
    debugMode: false,
  }),
  saveSettings: vi.fn().mockImplementation(async (settings) => settings),
  importSettings: vi.fn().mockResolvedValue({
    version: 1,
    autoOutputThreshold: 70,
    layoutSensitivity: 1,
    positionSensitivity: 1,
    keywordSensitivity: 1,
    textQualitySensitivity: 1,
    ocrConservatism: 1,
    keywordRules: [{ keyword: "通知", scoreDelta: 5 }],
    regexRules: [],
    debugMode: false,
  }),
  exportSettings: vi.fn().mockResolvedValue(undefined),
  resetSettings: vi.fn().mockResolvedValue({
    version: 1,
    autoOutputThreshold: 70,
    layoutSensitivity: 1,
    positionSensitivity: 1,
    keywordSensitivity: 1,
    textQualitySensitivity: 1,
    ocrConservatism: 1,
    keywordRules: [{ keyword: "通知", scoreDelta: 5 }],
    regexRules: [],
    debugMode: false,
  }),
}));

vi.mock("./api/events", () => ({
  subscribeBatchEvents: vi.fn().mockResolvedValue(() => undefined),
}));

vi.mock("./api/dragDrop", () => ({
  subscribeFileDrops: vi.fn().mockResolvedValue(() => undefined),
}));

describe("App", () => {
  it("renders the main queue, history, and settings workflows", async () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Rustitler" })).toBeInTheDocument();
    expect(screen.getByText("拖入文件或文件夹开始处理")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "历史" }));
    await waitFor(() => expect(screen.getByText("batch-1")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "查看详情" }));
    await waitFor(() => expect(screen.getByText("项目通知")).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    await waitFor(() => expect(screen.getByLabelText("自动输出阈值")).toHaveValue(70));
    fireEvent.change(screen.getByLabelText("自动输出阈值"), { target: { value: "80" } });
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
    await waitFor(() => expect(screen.getByText("设置已保存")).toBeInTheDocument());
  });
});
