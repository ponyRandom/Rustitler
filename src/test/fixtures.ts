import type {
  AppErrorView,
  BatchEvent,
  BatchState,
  BatchSummary,
  FileJobView,
  HistoryBatchDetail,
  HistoryBatchPage,
  ScoringResult,
  Settings,
} from "../types/ipc";

export const defaultSummary = (): BatchSummary => ({
  total: 0,
  outputCreated: 0,
  pending: 0,
  skipped: 0,
  failed: 0,
  cancelled: 0,
});

export const defaultSettings = (): Settings => ({
  version: 1,
  autoOutputThreshold: 70,
  layoutSensitivity: 1,
  positionSensitivity: 1,
  keywordSensitivity: 1,
  textQualitySensitivity: 1,
  ocrConservatism: 1,
  maxTitleChars: 45,
  keywordRules: [{ keyword: "通知", scoreDelta: 5 }],
  regexRules: [],
  debugMode: false,
  classificationSettings: {
    categories: [
      { name: "请示", keywords: ["请示"] },
      { name: "报告", keywords: ["报告"] },
      { name: "通知", keywords: ["通知"] },
      { name: "标准", keywords: ["标准"] },
    ],
  },
});

export const fileView = (overrides: Partial<FileJobView> = {}): FileJobView => ({
  fileJobId: "file-1",
  batchId: "batch-1",
  sourcePath: "/input/source.pdf",
  fileName: "source.pdf",
  fileType: "pdf",
  status: "queued",
  ...overrides,
});

export const scoringResult = (overrides: Partial<ScoringResult> = {}): ScoringResult => ({
  finalTitle: "项目通知",
  confidence: 84,
  decision: "autoOutput",
  candidates: [
    {
      text: "项目通知",
      source: "pdfLayout",
      pageIndex: 0,
      score: 84,
      categoryScores: {
        layout: 30,
        position: 20,
        keyword: 15,
        textQuality: 20,
        penalty: -1,
      },
      ruleDetails: [
        {
          ruleName: "keyword:通知",
          category: "keyword",
          delta: 5,
          description: "命中关键词“通知”",
        },
      ],
    },
  ],
  ...overrides,
});

export const appError = (overrides: Partial<AppErrorView> = {}): AppErrorView => ({
  code: "pdfExtractFailed",
  category: "extraction",
  userMessage: "PDF 提取失败",
  retryable: true,
  stage: "extract",
  ...overrides,
});

export const batchState = (overrides: Partial<BatchState> = {}): BatchState => ({
  batchId: "batch-1",
  createdAt: "2026-06-27T01:00:00Z",
  status: "running",
  settingsSnapshotId: "settings-1",
  files: [],
  summary: defaultSummary(),
  ...overrides,
});

export const historyPage = (overrides: Partial<HistoryBatchPage> = {}): HistoryBatchPage => ({
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
  ...overrides,
});

export const historyDetail = (
  overrides: Partial<HistoryBatchDetail> = {},
): HistoryBatchDetail => ({
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
      file: fileView({
        status: "outputCreated",
        recognizedTitle: "项目通知",
        confidence: 84,
        outputPath: "/input/Rustitler 输出/项目通知.pdf",
      }),
      sourceFingerprint: {
        normalizedPath: "/input/source.pdf",
        sizeBytes: 2048,
        modifiedTime: "2026-06-27T00:00:00Z",
      },
      scoringResult: scoringResult(),
    },
  ],
  ...overrides,
});

export const batchEvent = (event: BatchEvent): BatchEvent => event;
