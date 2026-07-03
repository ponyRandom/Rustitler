import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { subscribeFileDrops } from "./api/dragDrop";
import { subscribeBatchEvents } from "./api/events";
import { selectFiles, selectFolder } from "./api/fileDialog";
import {
  classifyFolder,
  getHistoryBatch,
  listHistory,
  undoBatch,
} from "./api/commands";
import "./App.css";
import { batchStore, type FileUiState } from "./stores/batchStore";
import { settingsStore } from "./stores/settingsStore";
import type {
  AppErrorView,
  BatchSummary,
  ClassificationCategory,
  ClassificationSummary,
  HistoryBatchDetail,
  HistoryBatchPage,
  KeywordRule,
  RegexRule,
} from "./types/ipc";

type Tab = "queue" | "history" | "settings";

const statusLabel = {
  queued: "排队中",
  analyzing: "分析中",
  outputCreated: "已输出",
  pending: "待处理",
  skipped: "已跳过",
  failed: "失败",
  undoable: "可撤销",
  cancelled: "已取消",
};

const fileTypeLabel = {
  docx: "DOCX",
  doc: "DOC",
  pdf: "PDF",
  png: "PNG",
  jpg: "JPG",
  jpeg: "JPEG",
  unsupported: "不支持",
};

const summaryText = (summary?: BatchSummary) => {
  if (!summary) {
    return "0 个文件";
  }
  return `${summary.total} 个文件 / ${summary.outputCreated} 已输出 / ${summary.pending} 待处理 / ${summary.failed} 失败`;
};

const errorText = (error: AppErrorView | string | undefined) => {
  if (!error) {
    return "";
  }
  return typeof error === "string" ? error : error.userMessage;
};

const fileOutcomeText = (file: FileUiState) => {
  if (file.outputPath) return file.outputPath;
  if (file.failureReason) return file.failureReason;
  if (file.duplicateWarning) return "疑似重复";
  return "-";
};

const duplicateNoticeText = () => "可能已处理过，请核对历史输出。";

const compactId = (id: string) => (id.length > 18 ? `${id.slice(0, 8)}...${id.slice(-5)}` : id);

const formatBatchTime = (value: string) => {
  const [date, rest = ""] = value.split("T");
  const time = rest.slice(0, 5);
  return time ? `${date} ${time}` : value;
};

const unknownErrorText = (error: unknown) => {
  if (typeof error === "object" && error && "userMessage" in error) {
    return String(error.userMessage);
  }
  return error instanceof Error ? error.message : String(error);
};

const fileStem = (fileName: string) => {
  const dot = fileName.lastIndexOf(".");
  return dot > 0 ? fileName.slice(0, dot) : fileName;
};

const fileExtension = (fileName: string) => {
  const dot = fileName.lastIndexOf(".");
  return dot > 0 ? fileName.slice(dot) : "";
};

const useBatchState = () =>
  useSyncExternalStore(batchStore.subscribe, batchStore.getState, batchStore.getState);

const useSettingsState = () =>
  useSyncExternalStore(settingsStore.subscribe, settingsStore.getState, settingsStore.getState);

export default function App() {
  const [tab, setTab] = useState<Tab>("queue");
  const [dropActive, setDropActive] = useState(false);
  const [importError, setImportError] = useState("");
  const [classificationBusy, setClassificationBusy] = useState(false);
  const [classificationSummary, setClassificationSummary] = useState<ClassificationSummary | null>(null);
  const [classificationError, setClassificationError] = useState<AppErrorView | string | undefined>();
  const batchState = useBatchState();
  const settingsState = useSettingsState();
  const settingsReady = Boolean(settingsState.draft);
  const isRunning = batchState.batch?.status === "running";

  useEffect(() => {
    void settingsStore.load().catch(() => undefined);
  }, []);

  useEffect(() => {
    let disposed = false;
    let stopEvents: (() => void) | undefined;
    let stopDrops: (() => void) | undefined;

    void subscribeBatchEvents((event) => {
      batchStore.applyEvent(event);
    }).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        stopEvents = unlisten;
      }
    });

    void subscribeFileDrops((paths) => void startImport(paths), setDropActive).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        stopDrops = unlisten;
      }
    });

    return () => {
      disposed = true;
      stopEvents?.();
      stopDrops?.();
    };
  }, []);

  const startImport = async (paths: string[]) => {
    const settings = settingsStore.getState().draft;
    if (paths.length === 0) {
      return;
    }
    if (!settings) {
      setImportError("设置仍在加载，请稍后再导入。");
      return;
    }
    setImportError("");
    try {
      await batchStore.start(paths, settings);
      setTab("queue");
    } catch (error) {
      setImportError(unknownErrorText(error));
    }
  };

  const importFromDialog = async (selectPaths: () => Promise<string[]>) => {
    try {
      await startImport(await selectPaths());
    } catch (error) {
      setImportError(unknownErrorText(error));
    }
  };

  const classifySelectedFolder = async () => {
    try {
      const [sourcePath] = await selectFolder();
      if (!sourcePath) {
        return;
      }
      const settings = settingsStore.getState().draft;
      if (!settings) {
        setClassificationSummary(null);
        setClassificationError("设置仍在加载，请稍后再分类。");
        return;
      }
      const classificationSettings = structuredClone(settings.classificationSettings);
      setClassificationBusy(true);
      setClassificationSummary(null);
      setClassificationError(undefined);
      setClassificationSummary(await classifyFolder(sourcePath, classificationSettings));
      setTab("queue");
    } catch (error) {
      setClassificationSummary(null);
      setClassificationError(
        typeof error === "object" && error && "userMessage" in error
          ? (error as AppErrorView)
          : unknownErrorText(error),
      );
    } finally {
      setClassificationBusy(false);
    }
  };

  const toolbarTitle = tab === "queue" ? "处理队列" : tab === "history" ? "历史记录" : "设置";
  const toolbarSummary =
    tab === "queue"
      ? summaryText(batchState.batch?.summary)
      : tab === "history"
        ? "查看历史批次与撤销输出"
        : "调整识别评分与规则";

  return (
    <div className={`app-shell ${dropActive ? "drop-active" : ""}`}>
      <aside className="sidebar">
        <div className="brand-block">
          <div className="brand-mark" aria-hidden="true">
            R
          </div>
          <h1>Rustitler</h1>
          <p>离线文档标题识别与重命名</p>
        </div>
        <nav className="sidebar-nav" aria-label="主导航">
          <button className={tab === "queue" ? "active" : ""} onClick={() => setTab("queue")}>
            队列
          </button>
          <button className={tab === "history" ? "active" : ""} onClick={() => setTab("history")}>
            历史
          </button>
          <button className={tab === "settings" ? "active" : ""} onClick={() => setTab("settings")}>
            设置
          </button>
        </nav>
      </aside>

      <div className="workspace-shell">
        <header className="toolbar" role="banner" aria-label="应用工具栏">
          <div>
            <h2>{toolbarTitle}</h2>
            <p>{toolbarSummary}</p>
          </div>
          {tab === "queue" ? (
            <div className="toolbar-actions">
              <button type="button" onClick={() => void importFromDialog(selectFiles)} disabled={!settingsReady}>
                导入文件
              </button>
              <button
                type="button"
                className="secondary"
                onClick={() => void importFromDialog(selectFolder)}
                disabled={!settingsReady}
              >
                导入文件夹
              </button>
              <button
                type="button"
                className="secondary classify-action"
                onClick={() => void classifySelectedFolder()}
                disabled={!settingsReady || classificationBusy}
              >
                {classificationBusy ? "分类中..." : "分类文件夹"}
              </button>
              <button
                className="secondary"
                disabled={!isRunning || batchState.cancelling}
                onClick={() => void batchStore.cancel()}
              >
                {batchState.cancelling ? "取消中" : "取消批次"}
              </button>
            </div>
          ) : null}
        </header>

        <main>
          {tab === "queue" ? (
            <QueueView
              batchState={batchState}
              importError={importError}
              settingsError={settingsState.error}
              classificationSummary={classificationSummary}
              classificationError={classificationError}
            />
          ) : null}
          {tab === "history" ? <HistoryView /> : null}
          {tab === "settings" ? <SettingsView /> : null}
        </main>
      </div>
    </div>
  );
}

function QueueView({
  batchState,
  importError,
  settingsError,
  classificationSummary,
  classificationError,
}: {
  batchState: ReturnType<typeof useBatchState>;
  importError: string;
  settingsError?: string;
  classificationSummary?: ClassificationSummary | null;
  classificationError?: AppErrorView | string;
}) {
  const selected =
    batchState.files.find((file) => file.fileJobId === batchState.selectedFileJobId) ??
    batchState.files[0];

  return (
    <section className="queue-layout" aria-label="队列工作区">
      <div className="queue-panel" role="region" aria-label="文件队列">
        <div className="panel-heading">
          <div>
            <h2>处理队列</h2>
            <p>{summaryText(batchState.batch?.summary)}</p>
          </div>
        </div>

        <div className="drop-zone">
          <div>
            <strong>拖入文件或文件夹开始处理</strong>
          </div>
        </div>

        <div className="message-stack">
          {importError ? <p className="error">{importError}</p> : null}
          {settingsError ? <p className="error">{settingsError}</p> : null}
          {classificationError ? <ClassificationErrorMessage error={classificationError} /> : null}
          {errorText(batchState.error) ? <p className="error">{errorText(batchState.error)}</p> : null}
        </div>

        {classificationSummary ? <ClassificationResult summary={classificationSummary} /> : null}

        {batchState.files.length === 0 ? (
          <div className="queue-empty" role="status" aria-label="队列为空">
            <strong>队列为空</strong>
          </div>
        ) : (
          <div className="file-list" role="list" aria-label="文件队列">
            {batchState.files.map((file) => (
              <div className="file-list-item" role="listitem" key={file.fileJobId}>
                <button
                  className={`file-card ${selected?.fileJobId === file.fileJobId ? "selected" : ""}`}
                  onClick={() => batchStore.selectFile(file.fileJobId)}
                >
                  <div className="file-card-main">
                    <strong title={file.sourcePath}>{file.fileName}</strong>
                    <span title={file.sourcePath}>{file.sourcePath}</span>
                  </div>
                  <div className="file-card-state">
                    <span>{fileTypeLabel[file.fileType]}</span>
                    <mark data-status={file.status}>{statusLabel[file.status]}</mark>
                  </div>
                  <div className="file-card-title">
                    <span>标题</span>
                    <strong>{file.recognizedTitle ?? "-"}</strong>
                  </div>
                  <div className="file-card-result">
                    <span>{file.confidence != null ? `${file.confidence}%` : "未评分"}</span>
                    <small title={file.outputPath ?? file.failureReason ?? file.duplicateWarning ?? ""}>
                      {fileOutcomeText(file)}
                    </small>
                  </div>
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
      <FileDetail file={selected} />
    </section>
  );
}

function ClassificationErrorMessage({ error }: { error: AppErrorView | string }) {
  if (typeof error === "string") {
    return <p className="error">{error}</p>;
  }

  return (
    <div className="error classification-error" role="alert">
      <strong>{error.userMessage}</strong>
      <span>{error.code}</span>
      {error.technicalDetail ? <small>{error.technicalDetail}</small> : null}
    </div>
  );
}

function ClassificationResult({ summary }: { summary: ClassificationSummary }) {
  return (
    <section className="classification-result" role="region" aria-label="分类结果">
      <div className="panel-heading">
        <div>
          <h2>分类结果</h2>
          <p>{summary.totalFiles} 个文件</p>
        </div>
      </div>

      <dl className="facts classification-facts">
        <div>
          <dt>源文件夹</dt>
          <dd title={summary.sourcePath}>{summary.sourcePath}</dd>
        </div>
        <div>
          <dt>输出文件夹</dt>
          <dd title={summary.outputPath}>{summary.outputPath}</dd>
        </div>
        <div>
          <dt>总文件数</dt>
          <dd>{summary.totalFiles}</dd>
        </div>
        <div>
          <dt>成功复制</dt>
          <dd>{summary.copiedFiles} 个</dd>
        </div>
        <div>
          <dt>失败文件</dt>
          <dd>{summary.failedFiles} 个</dd>
        </div>
      </dl>

      <div className="category-counts" role="list" aria-label="分类计数">
        {summary.categoryCounts.map((item) => (
          <div role="listitem" key={item.category}>
            <span>{item.category}</span>
            <strong>{item.count}</strong>
          </div>
        ))}
      </div>

      {summary.failures.length > 0 ? (
        <div className="classification-failures">
          <h3>失败明细</h3>
          <div role="list" aria-label="失败明细">
            {summary.failures.map((failure) => (
              <div role="listitem" key={`${failure.sourcePath}-${failure.reason}`}>
                <strong title={failure.sourcePath}>{failure.sourcePath}</strong>
                <span>{failure.reason}</span>
              </div>
            ))}
          </div>
        </div>
      ) : null}
    </section>
  );
}

function FileDetail({ file }: { file?: FileUiState }) {
  const [editedStem, setEditedStem] = useState("");
  const [selectedCandidateTitle, setSelectedCandidateTitle] = useState("");

  useEffect(() => {
    setEditedStem(file?.recognizedTitle ?? (file ? fileStem(file.fileName) : ""));
    setSelectedCandidateTitle(file?.recognizedTitle ?? "");
  }, [file?.fileJobId, file?.recognizedTitle, file?.fileName]);

  if (!file) {
    return (
      <aside className="detail-panel" aria-label="文件检查器">
        <div className="detail-scroll">
          <h2>详情</h2>
        </div>
      </aside>
    );
  }

  const extension = fileExtension(file.fileName);
  const candidateTitles = file.scoringResult?.candidates.slice(0, 5) ?? [];
  const selectedCandidateIsCurrent = selectedCandidateTitle === file.recognizedTitle;

  return (
    <aside className="detail-panel" aria-label="文件检查器">
      <div className="detail-scroll">
        <div className="detail-heading">
          <h2>详情</h2>
          <p>当前文件的关键信息</p>
        </div>

        <dl className="facts summary-facts concise-facts">
          <div>
            <dt>文件地址</dt>
            <dd title={file.sourcePath}>{file.sourcePath}</dd>
          </div>
          <div>
            <dt>最终标题</dt>
            <dd>{file.recognizedTitle ?? "-"}</dd>
          </div>
          {file.duplicateWarning ? (
            <div className="duplicate-fact">
              <dt>疑似重复</dt>
              <dd>{duplicateNoticeText()}</dd>
            </div>
          ) : null}
        </dl>

        {candidateTitles.length > 0 ? (
          <section className="candidate-section" aria-labelledby="candidate-heading">
            <div className="candidate-heading-row">
              <h3 id="candidate-heading">候选标题</h3>
              <div className="candidate-actions" role="group" aria-label="候选标题操作">
                <button
                  type="button"
                  disabled={!selectedCandidateTitle || selectedCandidateIsCurrent}
                  onClick={() => void batchStore.selectCandidateTitle(file.fileJobId, selectedCandidateTitle)}
                >
                  确认使用该标题
                </button>
              </div>
            </div>
            <div className="candidate-title-list" role="list" aria-label="候选标题列表">
              {candidateTitles.map((candidate, index) => {
                const selected = candidate.text === selectedCandidateTitle;
                const current = candidate.text === file.recognizedTitle;
                return (
                  <button
                    type="button"
                    className={`candidate-title ${selected ? "active" : ""} ${current ? "current" : ""}`}
                    key={`${candidate.text}-${index}`}
                    onClick={() => setSelectedCandidateTitle(candidate.text)}
                    title={candidate.text}
                  >
                    <span className="candidate-title-text">{candidate.text}</span>
                  </button>
                );
              })}
            </div>
            <p className="candidate-hint">
              {selectedCandidateIsCurrent
                ? "当前标题已选中。"
                : "点击确认后会按所选标题生成新的输出文件。"}
            </p>
          </section>
        ) : null}

        {file.status === "pending" ? (
          <form
            className="pending-form"
            onSubmit={(event) => {
              event.preventDefault();
              void batchStore.confirmPending(file.fileJobId, editedStem);
            }}
          >
            <label htmlFor="pending-name">文件名主体</label>
            <div className="stem-editor">
              <input
                id="pending-name"
                value={editedStem}
                onChange={(event) => setEditedStem(event.target.value)}
              />
              <span>{extension}</span>
            </div>
            <button type="submit">确认输出</button>
          </form>
        ) : null}
      </div>
    </aside>
  );
}

function HistoryView() {
  const [page, setPage] = useState<HistoryBatchPage>({ batches: [], total: 0 });
  const [detail, setDetail] = useState<HistoryBatchDetail | null>(null);
  const [selectedBatchId, setSelectedBatchId] = useState("");
  const [message, setMessage] = useState("");
  const [loading, setLoading] = useState(false);

  const load = async () => {
    setLoading(true);
    try {
      setPage(await listHistory(0, 50));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  return (
    <section className="history-layout">
      <div className="history-list">
        <div className="panel-heading">
          <div>
            <h2>历史批次</h2>
            <p>{page.total} 条记录</p>
          </div>
          <button className="secondary" onClick={() => void load()}>
            刷新
          </button>
        </div>
        {loading ? <p className="muted">加载中</p> : null}
        <div className="history-items" role="list" aria-label="历史批次">
          {page.batches.map((batch) => (
            <div className="history-row" role="listitem" key={batch.batchId}>
              <div>
                <strong title={batch.batchId}>{compactId(batch.batchId)}</strong>
                <span>{formatBatchTime(batch.createdAt)}</span>
                <small>{summaryText(batch.summary)}</small>
              </div>
              <button
                className={`secondary ${selectedBatchId === batch.batchId ? "active" : ""}`}
                onClick={async () => {
                  setSelectedBatchId(batch.batchId);
                  setDetail(await getHistoryBatch(batch.batchId));
                  setMessage("");
                }}
              >
                查看详情
              </button>
            </div>
          ))}
        </div>
        {page.batches.length === 0 ? <p className="empty">暂无历史记录。</p> : null}
      </div>

      <aside className="detail-panel history-detail" aria-label="批次检查器">
        <div className="detail-scroll">
          <div className="panel-heading">
            <div>
              <h2>批次详情</h2>
              <p>{detail ? summaryText(detail.summary) : "选择左侧批次"}</p>
            </div>
            <button
              className="secondary"
              disabled={!detail}
              onClick={async () => {
                if (!detail) {
                  return;
                }
                const result = await undoBatch(detail.batchId);
                setMessage(`撤销完成：删除 ${result.deleted}，缺失 ${result.skippedMissing}，已修改 ${result.skippedModified}`);
              }}
            >
              撤销输出
            </button>
          </div>
          {message ? <p className="notice">{message}</p> : null}
          <div className="history-detail-files">
            {detail?.files.map((result) => (
              <div className="detail-file" key={result.file.fileJobId}>
                <strong>{result.file.fileName}</strong>
                <span>{result.file.recognizedTitle ?? result.error?.userMessage ?? "-"}</span>
                <small>{result.file.outputPath ?? result.file.failureReason ?? result.file.sourcePath}</small>
              </div>
            ))}
          </div>
        </div>
      </aside>
    </section>
  );
}

function SettingsView() {
  const state = useSettingsState();
  const draft = state.draft;

  if (state.loading || !draft) {
    return <p className="muted">正在加载设置。</p>;
  }

  const numberPatch = (key: keyof typeof draft) => (event: React.ChangeEvent<HTMLInputElement>) => {
    settingsStore.updateDraft({ [key]: Number(event.target.value) });
  };

  return (
    <section className="settings-layout" role="region" aria-label="设置">
      <div className="settings-content" aria-label="设置内容">
        <div className="settings-panel" role="group" aria-label="评分设置">
          <div className="panel-heading">
            <div>
              <h2>评分设置</h2>
              <p>保存时由后端执行完整校验。</p>
            </div>
            <label className="switch">
              <input
                type="checkbox"
                checked={draft.debugMode}
                onChange={(event) => settingsStore.updateDraft({ debugMode: event.target.checked })}
              />
              Debug 模式
            </label>
          </div>

          <div className="control-grid">
            <NumberField
              label="自动输出阈值"
              min={0}
              max={100}
              value={draft.autoOutputThreshold}
              onChange={numberPatch("autoOutputThreshold")}
            />
            <NumberField label="版式敏感度" min={0} max={2} step={0.1} value={draft.layoutSensitivity} onChange={numberPatch("layoutSensitivity")} />
            <NumberField label="位置敏感度" min={0} max={2} step={0.1} value={draft.positionSensitivity} onChange={numberPatch("positionSensitivity")} />
            <NumberField label="关键词敏感度" min={0} max={2} step={0.1} value={draft.keywordSensitivity} onChange={numberPatch("keywordSensitivity")} />
            <NumberField label="文本质量敏感度" min={0} max={2} step={0.1} value={draft.textQualitySensitivity} onChange={numberPatch("textQualitySensitivity")} />
            <NumberField label="OCR 保守度" min={0} max={2} step={0.1} value={draft.ocrConservatism} onChange={numberPatch("ocrConservatism")} />
            <NumberField label="标题最大字数" min={10} max={120} value={draft.maxTitleChars} onChange={numberPatch("maxTitleChars")} />
          </div>
        </div>

        <ClassificationSettingsEditor categories={draft.classificationSettings.categories} />

        <RuleEditor
          title="关键词规则"
          keywordRules={draft.keywordRules}
          regexRules={draft.regexRules}
        />
      </div>

      <div className="settings-footer" role="group" aria-label="设置操作">
        <div className="settings-actions">
          <button onClick={() => void settingsStore.save().catch(() => undefined)} disabled={state.saving}>
            保存设置
          </button>
          <PathAction label="导入设置" onSubmit={(path) => settingsStore.importFrom(path)} />
          <PathAction label="导出设置" onSubmit={(path) => settingsStore.exportTo(path)} />
          <button className="secondary" onClick={() => void settingsStore.reset()}>
            恢复默认
          </button>
        </div>
        <div className="settings-feedback">
          {state.message ? <p className="notice">{state.message}</p> : null}
          {state.error ? <p className="error">{state.error}</p> : null}
        </div>
      </div>
    </section>
  );
}

function NumberField({
  label,
  value,
  min,
  max,
  step = 1,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
}) {
  return (
    <label className="number-field">
      <span>{label}</span>
      <input aria-label={label} type="number" min={min} max={max} step={step} value={value} onChange={onChange} />
    </label>
  );
}

function ClassificationSettingsEditor({ categories }: { categories: ClassificationCategory[] }) {
  return (
    <div className="settings-panel classification-panel" role="group" aria-label="分类配置">
      <div className="panel-heading">
        <div>
          <h2>分类配置</h2>
          <p>{categories.length} 个分类</p>
        </div>
        <button className="secondary" onClick={() => settingsStore.addClassificationCategory()}>
          添加分类
        </button>
      </div>

      <div className="classification-editor" role="list" aria-label="分类列表">
        {categories.map((category, categoryIndex) => {
          const categoryNumber = categoryIndex + 1;
          return (
            <div className="classification-row" role="listitem" key={`${category.name}-${categoryIndex}`}>
              <div className="classification-category-fields">
                <label>
                  <span>分类名称</span>
                  <input
                    aria-label={`分类名称 ${categoryNumber}`}
                    value={category.name}
                    onChange={(event) =>
                      settingsStore.updateClassificationCategory(categoryIndex, { name: event.target.value })
                    }
                  />
                </label>
                <button
                  className="icon-button"
                  aria-label={`删除分类 ${categoryNumber}`}
                  onClick={() => settingsStore.removeClassificationCategory(categoryIndex)}
                >
                  ×
                </button>
              </div>

              <div className="classification-keywords">
                {category.keywords.map((keyword, keywordIndex) => {
                  const keywordNumber = keywordIndex + 1;
                  return (
                    <div className="classification-keyword-row" key={`${categoryIndex}-${keywordIndex}`}>
                      <input
                        aria-label={`分类关键词 ${categoryNumber}-${keywordNumber}`}
                        value={keyword}
                        onChange={(event) =>
                          settingsStore.updateClassificationKeyword(categoryIndex, keywordIndex, event.target.value)
                        }
                      />
                      <button
                        className="icon-button"
                        aria-label={`删除关键词 ${categoryNumber}-${keywordNumber}`}
                        onClick={() => settingsStore.removeClassificationKeyword(categoryIndex, keywordIndex)}
                      >
                        ×
                      </button>
                    </div>
                  );
                })}
                <button
                  className="secondary"
                  aria-label={`添加关键词 ${categoryNumber}`}
                  onClick={() => settingsStore.addClassificationKeyword(categoryIndex)}
                >
                  添加关键词
                </button>
              </div>
            </div>
          );
        })}
        {categories.length === 0 ? <p className="empty">暂无分类。</p> : null}
      </div>
    </div>
  );
}

function RuleEditor({
  title,
  keywordRules,
  regexRules,
}: {
  title: string;
  keywordRules: KeywordRule[];
  regexRules: RegexRule[];
}) {
  return (
    <div className="settings-panel rule-panel" role="group" aria-label={title}>
      <div className="panel-heading">
        <div>
          <h2>{title}</h2>
          <p>{keywordRules.length} 个关键词 / {regexRules.length} 个正则</p>
        </div>
        <div className="button-pair">
          <button className="secondary" onClick={() => settingsStore.addKeywordRule()}>
            添加关键词
          </button>
          <button className="secondary" onClick={() => settingsStore.addRegexRule()}>
            添加正则
          </button>
        </div>
      </div>

      <div className="rule-editor" role="list" aria-label="关键词和正则规则">
        {keywordRules.map((rule, index) => (
          <div className="rule-row" role="listitem" key={`keyword-${index}`}>
            <span className="rule-kind">关键词</span>
            <input
              aria-label={`关键词 ${index + 1}`}
              value={rule.keyword}
              onChange={(event) => settingsStore.updateKeywordRule(index, { keyword: event.target.value })}
            />
            <input
              aria-label={`关键词分值 ${index + 1}`}
              type="number"
              value={rule.scoreDelta}
              onChange={(event) => settingsStore.updateKeywordRule(index, { scoreDelta: Number(event.target.value) })}
            />
            <button className="icon-button" onClick={() => settingsStore.removeKeywordRule(index)} aria-label="删除关键词">
              ×
            </button>
          </div>
        ))}
        {regexRules.map((rule, index) => (
          <div className="rule-row" role="listitem" key={`regex-${index}`}>
            <span className="rule-kind">正则</span>
            <input
              aria-label={`正则 ${index + 1}`}
              value={rule.pattern}
              onChange={(event) => settingsStore.updateRegexRule(index, { pattern: event.target.value })}
            />
            <input
              aria-label={`正则分值 ${index + 1}`}
              type="number"
              value={rule.scoreDelta}
              onChange={(event) => settingsStore.updateRegexRule(index, { scoreDelta: Number(event.target.value) })}
            />
            <button className="icon-button" onClick={() => settingsStore.removeRegexRule(index)} aria-label="删除正则">
              ×
            </button>
          </div>
        ))}
        {keywordRules.length === 0 && regexRules.length === 0 ? (
          <p className="empty">暂无规则。</p>
        ) : null}
      </div>
    </div>
  );
}

function PathAction({
  label,
  onSubmit,
}: {
  label: string;
  onSubmit: (path: string) => Promise<unknown>;
}) {
  const [path, setPath] = useState("");
  const inputId = useMemo(() => label.replace(/\s/g, "-"), [label]);

  return (
    <form
      className="path-action"
      onSubmit={(event) => {
        event.preventDefault();
        if (path.trim()) {
          void onSubmit(path.trim());
        }
      }}
    >
      <label htmlFor={inputId}>{label}</label>
      <input id={inputId} value={path} onChange={(event) => setPath(event.target.value)} />
      <button className="secondary" type="submit">
        执行
      </button>
    </form>
  );
}
