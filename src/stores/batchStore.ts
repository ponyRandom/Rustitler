import {
  cancelBatch,
  confirmPendingOutput,
  getBatchState,
  selectCandidateTitle,
  startBatch,
} from "../api/commands";
import type {
  AppErrorView,
  BatchEvent,
  BatchId,
  BatchState,
  BatchStatus,
  BatchSummary,
  FileJobId,
  FileJobView,
  ScoringResultView,
  Settings,
} from "../types/ipc";
import { createObservable, type ObservableStore } from "./observable";

export interface FileUiState extends FileJobView {
  progressStage?: string;
  progress?: number;
  extractMethod?: string;
  scoringResult?: ScoringResultView;
  error?: AppErrorView;
}

export interface BatchStoreState {
  batch: BatchState | null;
  files: FileUiState[];
  selectedFileJobId?: FileJobId;
  starting: boolean;
  cancelling: boolean;
  error?: AppErrorView | string;
}

export interface BatchStore extends ObservableStore<BatchStoreState> {
  start: (paths: string[], settings: Settings) => Promise<BatchId>;
  cancel: () => Promise<void>;
  refresh: () => Promise<void>;
  confirmPending: (fileJobId: FileJobId, editedNameStem: string) => Promise<FileJobView>;
  selectCandidateTitle: (fileJobId: FileJobId, candidateText: string) => Promise<FileJobView>;
  selectFile: (fileJobId: FileJobId) => void;
  applyEvent: (event: BatchEvent) => void;
  replaceSnapshot: (snapshot: BatchState) => void;
}

const emptySummary = (): BatchSummary => ({
  total: 0,
  outputCreated: 0,
  pending: 0,
  skipped: 0,
  failed: 0,
  cancelled: 0,
});

const initialState = (): BatchStoreState => ({
  batch: null,
  files: [],
  starting: false,
  cancelling: false,
});

export const createBatchStore = (): BatchStore => {
  const observable = createObservable<BatchStoreState>(initialState());

  const updateFile = (
    files: FileUiState[],
    fileJobId: FileJobId,
    updater: (file: FileUiState) => FileUiState,
    createMissing?: () => FileUiState,
  ): FileUiState[] => {
    const index = files.findIndex((file) => file.fileJobId === fileJobId);
    if (index === -1) {
      return createMissing ? [...files, updater(createMissing())] : files;
    }

    return files.map((file, currentIndex) => (currentIndex === index ? updater(file) : file));
  };

  const ensureBatch = (
    state: BatchStoreState,
    batchId: BatchId,
    status: BatchStatus = "running",
  ): BatchState => {
    if (state.batch?.batchId === batchId) {
      return state.batch;
    }

    return {
      batchId,
      createdAt: "",
      status,
      settingsSnapshotId: "",
      files: [],
      summary: emptySummary(),
    };
  };

  const replaceSnapshot = (snapshot: BatchState) => {
    observable.setState({
      ...observable.getState(),
      batch: snapshot,
      files: snapshot.files,
      selectedFileJobId:
        observable.getState().selectedFileJobId ?? snapshot.files[0]?.fileJobId,
      error: undefined,
    });
  };

  const applyEvent = (event: BatchEvent) => {
    observable.updateState((state) => {
      if (
        event.type !== "BatchStarted" &&
        state.batch &&
        "batchId" in event &&
        state.batch.batchId !== event.batchId
      ) {
        return state;
      }

      switch (event.type) {
        case "BatchStarted": {
          return {
            ...state,
            batch: {
              batchId: event.batchId,
              createdAt: event.createdAt,
              status: "running",
              settingsSnapshotId: "",
              files: [],
              summary: { ...emptySummary(), total: event.totalFiles },
            },
            files: [],
            selectedFileJobId: undefined,
            error: undefined,
          };
        }
        case "FileQueued": {
          const files = updateFile(
            state.files,
            event.file.fileJobId,
            () => event.file,
            () => event.file,
          );
          const batch = ensureBatch(state, event.batchId);
          return {
            ...state,
            batch: { ...batch, files },
            files,
            selectedFileJobId: state.selectedFileJobId ?? event.file.fileJobId,
          };
        }
        case "FileProgress": {
          const files = updateFile(state.files, event.fileJobId, (file) => {
            const shouldMarkAnalyzing = file.status === "queued" || file.status === "analyzing";
            return {
              ...file,
              status: shouldMarkAnalyzing ? "analyzing" : file.status,
              progressStage: event.stage,
              progress: event.progress,
            };
          });
          return { ...state, batch: state.batch ? { ...state.batch, files } : state.batch, files };
        }
        case "FileExtracted": {
          const files = updateFile(state.files, event.fileJobId, (file) => ({
            ...file,
            extractMethod: event.extractMethod,
          }));
          return { ...state, batch: state.batch ? { ...state.batch, files } : state.batch, files };
        }
        case "FileScored": {
          const files = updateFile(state.files, event.fileJobId, (file) => ({
            ...file,
            recognizedTitle: event.result.finalTitle ?? file.recognizedTitle,
            confidence: event.result.confidence,
            scoringResult: event.result,
          }));
          return { ...state, batch: state.batch ? { ...state.batch, files } : state.batch, files };
        }
        case "FileOutputCreated": {
          const files = updateFile(state.files, event.fileJobId, (file) => ({
            ...file,
            status: "outputCreated",
            outputPath: event.outputPath,
            failureReason: undefined,
            pendingReason: undefined,
          }));
          return { ...state, batch: state.batch ? { ...state.batch, files } : state.batch, files };
        }
        case "FilePending": {
          const files = updateFile(state.files, event.fileJobId, (file) => ({
            ...file,
            status: "pending",
            recognizedTitle: event.suggestion ?? file.recognizedTitle,
            pendingReason: event.reason,
          }));
          return { ...state, batch: state.batch ? { ...state.batch, files } : state.batch, files };
        }
        case "FileSkipped": {
          const files = updateFile(state.files, event.fileJobId, (file) => ({
            ...file,
            status: "skipped",
            failureReason: event.reason,
          }));
          return { ...state, batch: state.batch ? { ...state.batch, files } : state.batch, files };
        }
        case "FileFailed": {
          const files = updateFile(
            state.files,
            event.fileJobId,
            (file) => ({
              ...file,
              status: "failed",
              failureReason: event.error.userMessage,
              error: event.error,
            }),
            () => ({
              fileJobId: event.fileJobId,
              batchId: event.batchId,
              sourcePath: event.error.filePath ?? "",
              fileName: event.error.filePath?.split(/[\\/]/).pop() ?? event.fileJobId,
              fileType: "unsupported",
              status: "failed",
            }),
          );
          return {
            ...state,
            batch: { ...ensureBatch(state, event.batchId), files },
            files,
            selectedFileJobId: state.selectedFileJobId ?? event.fileJobId,
          };
        }
        case "BatchCompleted":
        case "BatchCancelled": {
          const status: BatchStatus = event.type === "BatchCompleted" ? "completed" : "cancelled";
          return {
            ...state,
            batch: {
              ...ensureBatch(state, event.batchId, status),
              status,
              summary: event.summary,
              files: state.files,
            },
            cancelling: false,
            starting: false,
          };
        }
        case "BatchError": {
          return { ...state, error: event.error, starting: false, cancelling: false };
        }
        default:
          return state;
      }
    });
  };

  const start = async (paths: string[], settings: Settings): Promise<BatchId> => {
    observable.updateState((state) => ({ ...state, starting: true, error: undefined }));
    try {
      const batchId = await startBatch(paths, settings);
      observable.updateState((state) => ({
        ...state,
        starting: false,
        batch: state.batch ?? {
          batchId,
          createdAt: "",
          status: "running",
          settingsSnapshotId: "",
          files: [],
          summary: emptySummary(),
        },
      }));
      return batchId;
    } catch (error) {
      observable.updateState((state) => ({ ...state, starting: false, error: error as string }));
      throw error;
    }
  };

  const refresh = async () => {
    const batchId = observable.getState().batch?.batchId;
    if (!batchId) {
      return;
    }
    const snapshot = await getBatchState(batchId);
    if (snapshot) {
      replaceSnapshot(snapshot);
    }
  };

  const confirmPending = async (fileJobId: FileJobId, editedNameStem: string) => {
    const updated = await confirmPendingOutput(fileJobId, editedNameStem);
    observable.updateState((state) => {
      const files = updateFile(state.files, fileJobId, () => updated, () => updated);
      return { ...state, batch: state.batch ? { ...state.batch, files } : state.batch, files };
    });
    return updated;
  };

  const chooseCandidateTitle = async (fileJobId: FileJobId, candidateText: string) => {
    const updated = await selectCandidateTitle(fileJobId, candidateText);
    observable.updateState((state) => {
      const files = updateFile(
        state.files,
        fileJobId,
        (file) => ({
          ...file,
          ...updated,
          scoringResult: file.scoringResult
            ? {
                ...file.scoringResult,
                finalTitle: updated.recognizedTitle,
                confidence: updated.confidence ?? file.scoringResult.confidence,
              }
            : file.scoringResult,
        }),
        () => updated,
      );
      return { ...state, batch: state.batch ? { ...state.batch, files } : state.batch, files };
    });
    return updated;
  };

  const cancel = async () => {
    const batchId = observable.getState().batch?.batchId;
    if (!batchId) {
      return;
    }
    observable.updateState((state) => ({ ...state, cancelling: true }));
    try {
      await cancelBatch(batchId);
    } finally {
      observable.updateState((state) => ({ ...state, cancelling: false }));
    }
  };

  const selectFile = (fileJobId: FileJobId) => {
    observable.updateState((state) => ({ ...state, selectedFileJobId: fileJobId }));
  };

  return {
    getState: observable.getState,
    subscribe: observable.subscribe,
    start,
    cancel,
    refresh,
    confirmPending,
    selectCandidateTitle: chooseCandidateTitle,
    selectFile,
    applyEvent,
    replaceSnapshot,
  };
};

export const batchStore = createBatchStore();
