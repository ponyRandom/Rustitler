import { beforeEach, describe, expect, it, vi } from "vitest";

const openMock = vi.fn();

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: openMock,
}));

describe("folder selection dialog", () => {
  beforeEach(() => {
    vi.resetModules();
    openMock.mockReset();
  });

  it("selects one folder path for classification", async () => {
    openMock.mockResolvedValueOnce("/input/folder");
    const { selectFolder } = await import("./fileDialog");

    await expect(selectFolder()).resolves.toEqual(["/input/folder"]);

    expect(openMock).toHaveBeenCalledWith({
      title: expect.any(String),
      directory: true,
      multiple: false,
    });
  });

  it("returns an empty array when folder selection is cancelled", async () => {
    openMock.mockResolvedValueOnce(null);
    const { selectFolder } = await import("./fileDialog");

    await expect(selectFolder()).resolves.toEqual([]);
  });

  it("normalizes multiple folder paths to the first valid path", async () => {
    openMock.mockResolvedValueOnce(["", "/input/folder", "/input/other-folder"]);
    const { selectFolder } = await import("./fileDialog");

    await expect(selectFolder()).resolves.toEqual(["/input/folder"]);
  });
});
