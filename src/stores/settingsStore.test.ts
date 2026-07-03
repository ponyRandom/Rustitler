import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultSettings } from "../test/fixtures";

vi.mock("../api/commands", () => ({
  loadSettings: vi.fn(),
  saveSettings: vi.fn(),
  importSettings: vi.fn(),
  exportSettings: vi.fn(),
  resetSettings: vi.fn(),
}));

describe("settings store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

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

  it("preserves backend classification settings in current and draft when loading", async () => {
    const commands = await import("../api/commands");
    const classificationSettings = {
      categories: [
        { name: "Invoices", keywords: ["invoice", "billing"] },
        { name: "Needs Review", keywords: [], systemKind: "needsReview" as const },
        { name: "Other", keywords: [], systemKind: "other" as const },
      ],
    };
    const loaded = { ...defaultSettings(), classificationSettings };
    vi.mocked(commands.loadSettings).mockResolvedValue(loaded);
    const { createSettingsStore } = await import("./settingsStore");
    const store = createSettingsStore();

    await store.load();

    expect(store.getState().current?.classificationSettings).toEqual(classificationSettings);
    expect(store.getState().draft?.classificationSettings).toEqual(classificationSettings);
    expect(store.getState().draft?.classificationSettings).not.toBe(classificationSettings);
  });

  it("adds, edits, and removes classification categories and keywords", async () => {
    const commands = await import("../api/commands");
    const initial = {
      ...defaultSettings(),
      classificationSettings: {
        categories: [{ name: "Other", keywords: [], systemKind: "other" as const }],
      },
    };
    vi.mocked(commands.loadSettings).mockResolvedValue(initial);
    const { createSettingsStore } = await import("./settingsStore");
    const store = createSettingsStore();

    await store.load();
    store.addClassificationCategory();
    store.updateClassificationCategory(1, { name: "Contracts" });
    store.updateClassificationKeyword(1, 0, "agreement");
    store.addClassificationKeyword(1);
    store.updateClassificationKeyword(1, 1, "msa");
    store.removeClassificationKeyword(1, 0);
    store.removeClassificationCategory(0);

    expect(store.getState().draft?.classificationSettings.categories).toEqual([
      { name: "Contracts", keywords: ["msa"] },
    ]);
  });

  it("saves classification edits through the existing saveSettings command", async () => {
    const commands = await import("../api/commands");
    const initial = defaultSettings();
    const saved = {
      ...initial,
      classificationSettings: {
        categories: [{ name: "Contracts", keywords: ["agreement"] }],
      },
    };
    vi.mocked(commands.loadSettings).mockResolvedValue(initial);
    vi.mocked(commands.saveSettings).mockResolvedValue(saved);
    const { createSettingsStore } = await import("./settingsStore");
    const store = createSettingsStore();

    await store.load();
    store.addClassificationCategory();
    const newCategoryIndex = initial.classificationSettings.categories.length;
    store.updateClassificationCategory(newCategoryIndex, { name: "Contracts" });
    store.updateClassificationKeyword(newCategoryIndex, 0, "agreement");
    await store.save();

    expect(commands.saveSettings).toHaveBeenCalledWith(
      expect.objectContaining({
        classificationSettings: {
          categories: [
            ...initial.classificationSettings.categories,
            { name: "Contracts", keywords: ["agreement"] },
          ],
        },
      }),
    );
    expect(store.getState().current).toEqual(saved);
    expect(store.getState().draft).toEqual(saved);
  });

  it("retains backend validation errors and keeps the editable draft after save failure", async () => {
    const commands = await import("../api/commands");
    const initial = defaultSettings();
    vi.mocked(commands.loadSettings).mockResolvedValue(initial);
    vi.mocked(commands.saveSettings).mockRejectedValue({
      userMessage: "Category name is required",
    });
    const { createSettingsStore } = await import("./settingsStore");
    const store = createSettingsStore();

    await store.load();
    store.addClassificationCategory();
    const editedDraft = structuredClone(store.getState().draft);

    await expect(store.save()).rejects.toEqual({
      userMessage: "Category name is required",
    });

    expect(store.getState().error).toBe("Category name is required");
    expect(store.getState().draft).toEqual(editedDraft);
    expect(store.getState().saving).toBe(false);
  });
});
