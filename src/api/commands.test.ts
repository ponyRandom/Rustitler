import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultSettings } from "../test/fixtures";
import type { Settings } from "../types/ipc";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("command wrappers", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("passes camelCase command arguments expected by Tauri", async () => {
    const { startBatch, confirmPendingOutput, listHistory } = await import("./commands");
    const settings = defaultSettings();
    invokeMock.mockResolvedValueOnce("batch-1");
    invokeMock.mockResolvedValueOnce({ fileJobId: "file-1" });
    invokeMock.mockResolvedValueOnce({ batches: [], total: 0 });

    await expect(startBatch(["/input/a.pdf"], settings)).resolves.toBe("batch-1");
    await confirmPendingOutput("file-1", "项目通知");
    await listHistory(20, 10);

    expect(invokeMock).toHaveBeenNthCalledWith(1, "start_batch", {
      paths: ["/input/a.pdf"],
      settingsSnapshot: settings,
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "confirm_pending_output", {
      fileJobId: "file-1",
      editedNameStem: "项目通知",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "list_history", {
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
});
