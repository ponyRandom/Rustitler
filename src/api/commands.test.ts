import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultSettings } from "../test/fixtures";
import type { ClassificationSettings, ClassificationSummary, Settings } from "../types/ipc";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("command wrappers", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("passes camelCase command arguments expected by Tauri", async () => {
    const { startBatch, confirmPendingOutput, selectCandidateTitle, listHistory } = await import("./commands");
    const settings = defaultSettings();
    invokeMock.mockResolvedValueOnce("batch-1");
    invokeMock.mockResolvedValueOnce({ fileJobId: "file-1" });
    invokeMock.mockResolvedValueOnce({ fileJobId: "file-1" });
    invokeMock.mockResolvedValueOnce({ batches: [], total: 0 });

    await expect(startBatch(["/input/a.pdf"], settings)).resolves.toBe("batch-1");
    await confirmPendingOutput("file-1", "项目通知");
    await selectCandidateTitle("file-1", "候选标题");
    await listHistory(20, 10);

    expect(invokeMock).toHaveBeenNthCalledWith(1, "start_batch", {
      paths: ["/input/a.pdf"],
      settingsSnapshot: settings,
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "confirm_pending_output", {
      fileJobId: "file-1",
      editedNameStem: "项目通知",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "select_candidate_title", {
      fileJobId: "file-1",
      candidateText: "候选标题",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(4, "list_history", {
      offset: 20,
      limit: 10,
    });
  });

  it("returns typed settings from settings commands", async () => {
    const { loadSettings, saveSettings } = await import("./commands");
    const settings: Settings = { ...defaultSettings(), autoOutputThreshold: 82 };
    invokeMock.mockResolvedValueOnce(settings);
    invokeMock.mockResolvedValueOnce(settings);

    await expect(loadSettings()).resolves.toEqual(settings);
    await expect(saveSettings(settings)).resolves.toEqual(settings);

    expect(invokeMock).toHaveBeenNthCalledWith(1, "load_settings");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "save_settings", { settings });
  });

  it("classifies a folder with the provided classification settings snapshot", async () => {
    const { classifyFolder } = await import("./commands");
    const classificationSettings: ClassificationSettings = {
      categories: [
        { name: "Invoices", keywords: ["invoice", "billing"] },
        { name: "Needs Review", keywords: [], systemKind: "needsReview" },
        { name: "Other", keywords: [], systemKind: "other" },
      ],
    };
    const summary: ClassificationSummary = {
      sourcePath: "/input/folder",
      outputPath: "/input/folder/classified",
      totalFiles: 3,
      copiedFiles: 2,
      failedFiles: 1,
      categoryCounts: [
        { category: "Invoices", count: 2 },
        { category: "Needs Review", count: 0 },
        { category: "Other", count: 0 },
      ],
      failures: [{ sourcePath: "/input/folder/bad.pdf", reason: "read failed" }],
    };
    invokeMock.mockResolvedValueOnce(summary);

    await expect(classifyFolder("/input/folder", classificationSettings)).resolves.toEqual(summary);

    expect(invokeMock).toHaveBeenCalledWith("classify_folder", {
      sourcePath: "/input/folder",
      classificationSettings,
    });
  });
});
