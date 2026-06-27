import {
  exportSettings,
  importSettings,
  loadSettings,
  resetSettings,
  saveSettings,
} from "../api/commands";
import type { KeywordRule, RegexRule, Settings } from "../types/ipc";
import { createObservable, type ObservableStore } from "./observable";

export interface SettingsStoreState {
  current?: Settings;
  draft?: Settings;
  loading: boolean;
  saving: boolean;
  message?: string;
  error?: string;
}

export interface SettingsStore extends ObservableStore<SettingsStoreState> {
  load: () => Promise<Settings>;
  updateDraft: (patch: Partial<Settings>) => void;
  addKeywordRule: () => void;
  updateKeywordRule: (index: number, patch: Partial<KeywordRule>) => void;
  removeKeywordRule: (index: number) => void;
  addRegexRule: () => void;
  updateRegexRule: (index: number, patch: Partial<RegexRule>) => void;
  removeRegexRule: (index: number) => void;
  save: () => Promise<Settings>;
  importFrom: (path: string) => Promise<Settings>;
  exportTo: (path: string) => Promise<void>;
  reset: () => Promise<Settings>;
}

const initialState: SettingsStoreState = {
  loading: false,
  saving: false,
};

const errorMessage = (error: unknown) => {
  if (typeof error === "object" && error && "userMessage" in error) {
    return String(error.userMessage);
  }
  return error instanceof Error ? error.message : String(error);
};

export const createSettingsStore = (): SettingsStore => {
  const observable = createObservable<SettingsStoreState>(initialState);

  const setSettings = (settings: Settings, message?: string) => {
    observable.updateState((state) => ({
      ...state,
      current: settings,
      draft: structuredClone(settings),
      loading: false,
      saving: false,
      message,
      error: undefined,
    }));
  };

  const requireDraft = () => {
    const draft = observable.getState().draft;
    if (!draft) {
      throw new Error("settings have not been loaded");
    }
    return draft;
  };

  const load = async () => {
    observable.updateState((state) => ({ ...state, loading: true, error: undefined }));
    try {
      const settings = await loadSettings();
      setSettings(settings);
      return settings;
    } catch (error) {
      observable.updateState((state) => ({
        ...state,
        loading: false,
        error: errorMessage(error),
      }));
      throw error;
    }
  };

  const updateDraft = (patch: Partial<Settings>) => {
    observable.updateState((state) => ({
      ...state,
      draft: state.draft ? { ...state.draft, ...patch } : state.draft,
      message: undefined,
      error: undefined,
    }));
  };

  const addKeywordRule = () => {
    const draft = requireDraft();
    updateDraft({ keywordRules: [...draft.keywordRules, { keyword: "", scoreDelta: 0 }] });
  };

  const updateKeywordRule = (index: number, patch: Partial<KeywordRule>) => {
    const draft = requireDraft();
    updateDraft({
      keywordRules: draft.keywordRules.map((rule, currentIndex) =>
        currentIndex === index ? { ...rule, ...patch } : rule,
      ),
    });
  };

  const removeKeywordRule = (index: number) => {
    const draft = requireDraft();
    updateDraft({ keywordRules: draft.keywordRules.filter((_, currentIndex) => currentIndex !== index) });
  };

  const addRegexRule = () => {
    const draft = requireDraft();
    updateDraft({ regexRules: [...draft.regexRules, { pattern: "", scoreDelta: 0 }] });
  };

  const updateRegexRule = (index: number, patch: Partial<RegexRule>) => {
    const draft = requireDraft();
    updateDraft({
      regexRules: draft.regexRules.map((rule, currentIndex) =>
        currentIndex === index ? { ...rule, ...patch } : rule,
      ),
    });
  };

  const removeRegexRule = (index: number) => {
    const draft = requireDraft();
    updateDraft({ regexRules: draft.regexRules.filter((_, currentIndex) => currentIndex !== index) });
  };

  const save = async () => {
    const draft = requireDraft();
    observable.updateState((state) => ({ ...state, saving: true, error: undefined }));
    try {
      const settings = await saveSettings(draft);
      setSettings(settings, "设置已保存");
      return settings;
    } catch (error) {
      observable.updateState((state) => ({
        ...state,
        saving: false,
        error: errorMessage(error),
      }));
      throw error;
    }
  };

  const importFrom = async (path: string) => {
    const settings = await importSettings(path);
    setSettings(settings, "设置已导入");
    return settings;
  };

  const exportTo = async (path: string) => {
    await exportSettings(path);
    observable.updateState((state) => ({ ...state, message: "设置已导出", error: undefined }));
  };

  const reset = async () => {
    const settings = await resetSettings();
    setSettings(settings, "已恢复默认设置");
    return settings;
  };

  return {
    getState: observable.getState,
    subscribe: observable.subscribe,
    load,
    updateDraft,
    addKeywordRule,
    updateKeywordRule,
    removeKeywordRule,
    addRegexRule,
    updateRegexRule,
    removeRegexRule,
    save,
    importFrom,
    exportTo,
    reset,
  };
};

export const settingsStore = createSettingsStore();
