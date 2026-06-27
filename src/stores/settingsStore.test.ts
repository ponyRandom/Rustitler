import { describe, expect, it, vi } from "vitest";
import { defaultSettings } from "../test/fixtures";

vi.mock("../api/commands", () => ({
  loadSettings: vi.fn(),
  saveSettings: vi.fn(),
  importSettings: vi.fn(),
  exportSettings: vi.fn(),
  resetSettings: vi.fn(),
}));

describe("settings store", () => {
  it("loads, edits, saves, imports, exports, and resets settings", async () => {
    const commands = await import("../api/commands");
    const initial = defaultSettings();
    const saved = { ...initial, autoOutputThreshold: 88 };
    const imported = { ...initial, debugMode: true };
    vi.mocked(commands.loadSettings).mockResolvedValue(initial);
    vi.mocked(commands.saveSettings).mockResolvedValue(saved);
    vi.mocked(commands.importSettings).mockResolvedValue(imported);
    vi.mocked(commands.resetSettings).mockResolvedValue(defaultSettings());
    const { createSettingsStore } = await import("./settingsStore");
    const store = createSettingsStore();

    await store.load();
    store.updateDraft({ autoOutputThreshold: 88 });
    store.addKeywordRule();
    store.updateKeywordRule(1, { keyword: "会议", scoreDelta: 4 });
    store.addRegexRule();
    store.updateRegexRule(0, { pattern: "^.{4,}$", scoreDelta: 2 });
    await store.save();
    await store.importFrom("/tmp/settings.json");
    await store.exportTo("/tmp/out.json");
    await store.reset();

    expect(commands.saveSettings).toHaveBeenCalledWith(
      expect.objectContaining({
        autoOutputThreshold: 88,
        keywordRules: [
          { keyword: "通知", scoreDelta: 5 },
          { keyword: "会议", scoreDelta: 4 },
        ],
        regexRules: [{ pattern: "^.{4,}$", scoreDelta: 2 }],
      }),
    );
    expect(commands.importSettings).toHaveBeenCalledWith("/tmp/settings.json");
    expect(commands.exportSettings).toHaveBeenCalledWith("/tmp/out.json");
    expect(store.getState().draft).toEqual(defaultSettings());
  });
});
