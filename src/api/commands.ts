import { invoke } from "@tauri-apps/api/core";
import type {
  BatchId,
  BatchState,
  FileJobId,
  FileJobView,
  HistoryBatchDetail,
  HistoryBatchPage,
  Settings,
  UndoResult,
} from "../types/ipc";

export const startBatch = (paths: string[], settingsSnapshot: Settings): Promise<BatchId> =>
  invoke<BatchId>("start_batch", { paths, settingsSnapshot });

export const cancelBatch = (batchId: BatchId): Promise<void> =>
  invoke<void>("cancel_batch", { batchId });

export const getBatchState = (batchId: BatchId): Promise<BatchState | null> =>
  invoke<BatchState | null>("get_batch_state", { batchId });

export const confirmPendingOutput = (
  fileJobId: FileJobId,
  editedNameStem: string,
): Promise<FileJobView> =>
  invoke<FileJobView>("confirm_pending_output", { fileJobId, editedNameStem });

export const undoBatch = (batchId: BatchId): Promise<UndoResult> =>
  invoke<UndoResult>("undo_batch", { batchId });

export const listHistory = (offset: number, limit: number): Promise<HistoryBatchPage> =>
  invoke<HistoryBatchPage>("list_history", { offset, limit });

export const getHistoryBatch = (batchId: BatchId): Promise<HistoryBatchDetail | null> =>
  invoke<HistoryBatchDetail | null>("get_history_batch", { batchId });

export const loadSettings = (): Promise<Settings> => invoke<Settings>("load_settings");

export const saveSettings = (settings: Settings): Promise<Settings> =>
  invoke<Settings>("save_settings", { settings });

export const importSettings = (path: string): Promise<Settings> =>
  invoke<Settings>("import_settings", { path });

export const exportSettings = (path: string): Promise<void> =>
  invoke<void>("export_settings", { path });

export const resetSettings = (): Promise<Settings> => invoke<Settings>("reset_settings");
