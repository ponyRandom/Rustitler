import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { subscribeFileDrops } from "./api/dragDrop";
import { subscribeBatchEvents } from "./api/events";
import {
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

const stageLabel: Record<string, string> = {
  ingest: "扫描输入",
  extract: "提取内容",
  score: "标题评分",
  rename: "创建输出",
  history: "写入历史",
  undo: "撤销输出",
};

const summaryText = (summary?: BatchSummary) => {
  if (!summary) {
    return "0 个文件";
  }
  return `${summary.total} 个文件 · ${summary.outputCreated} 已输出 · ${summary.pending} 待处理 · ${summary.failed} 失败`;
};

const errorText = (error: AppErrorView | string | undefined) => {
  if (!error) {
    return "";
  }
  return typeof error === "string" ? error : error.userMessage;
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
  const batchState = useBatchState();
  const settingsState = useSettingsState();

  useEffect(() => {
    void settingsStore.load();
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

    void subscribeFileDrops(
      (paths) => {
        const settings = settingsStore.getState().draft;
        if (paths.length > 0 && settings) {
          void batchStore.start(paths, settings);
          setTab("queue");
        }
      },
      setDropActive,
    ).then((unlisten) => {
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

  return (
    <div className={`app-shell ${dropActive ? "drop-active" : ""}`}>
      <header className="topbar">
        <div>
          <h1>Rustitler</h1>
          <p>离线文档标题识别与重命名</p>
        </div>
        <nav className="tabs" aria-label="主导航">
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
      </header>

      <main>
        {tab === "queue" ? (
          <QueueView batchState={batchState} settingsReady={Boolean(settingsState.draft)} />
        ) : null}
        {tab === "history" ? <HistoryView /> : null}
        {tab === "settings" ? <SettingsView /> : null}
      </main>
    </div>
  );
}

function QueueView({
  batchState,
  settingsReady,
}: {
  batchState: ReturnType<typeof useBatchState>;
  settingsReady: boolean;
}) {
  const selected =
    batchState.files.find((file) => file.fileJobId === batchState.selectedFileJobId) ??
    batchState.files[0];
  const isRunning = batchState.batch?.status === "running";

  return (
    <section className="queue-layout">
      <div className="queue-panel">
        <div className="panel-heading">
          <div>
            <h2>处理队列</h2>
            <p>{summaryText(batchState.batch?.summary)}</p>
          </div>
          <button
            className="secondary"
            disabled={!isRunning || batchState.cancelling}
            onClick={() => void batchStore.cancel()}
          >
            {batchState.cancelling ? "取消中" : "取消批次"}
          </button>
        </div>

        <div className="drop-zone">
          <strong>拖入文件或文件夹开始处理</strong>
          <span>{settingsReady ? "支持 PDF、Word、图片；文件夹仅扫描第一层。" : "正在加载设置。"}</span>
        </div>

        {errorText(batchState.error) ? <p className="error">{errorText(batchState.error)}</p> : null}

        <div className="file-table" role="table" aria-label="文件队列">
          <div className="file-row table-head" role="row">
            <span>文件名</span>
            <span>类型</span>
            <span>状态</span>
            <span>标题</span>
            <span>置信度</span>
            <span>输出 / 原因</span>
          </div>
          {batchState.files.map((file) => (
            <button
              className={`file-row ${selected?.fileJobId === file.fileJobId ? "selected" : ""}`}
              key={file.fileJobId}
              onClick={() => batchStore.selectFile(file.fileJobId)}
              role="row"
            >
              <span title={file.sourcePath}>{file.fileName}</span>
              <span>{fileTypeLabel[file.fileType]}</span>
              <span>
                <mark data-status={file.status}>{statusLabel[file.status]}</mark>
              </span>
              <span>{file.recognizedTitle ?? "—"}</span>
              <span>{file.confidence != null ? `${file.confidence}%` : "—"}</span>
              <span title={file.outputPath ?? file.failureReason ?? ""}>
                {file.outputPath ?? file.failureReason ?? "—"}
              </span>
            </button>
          ))}
          {batchState.files.length === 0 ? (
            <div className="empty">暂无队列。拖入文件或文件夹后会显示处理状态。</div>
          ) : null}
        </div>
      </div>
      <FileDetail file={selected} />
    </section>
  );
}

function FileDetail({ file }: { file?: FileUiState }) {
  const [editedStem, setEditedStem] = useState("");

  useEffect(() => {
    setEditedStem(file?.recognizedTitle ?? (file ? fileStem(file.fileName) : ""));
  }, [file?.fileJobId, file?.recognizedTitle, file?.fileName]);

  if (!file) {
    return (
      <aside className="detail-panel">
        <h2>详情</h2>
        <p className="muted">选择一个队列项查看候选标题、规则明细和输出状态。</p>
      </aside>
    );
  }

  const candidates = file.scoringResult?.candidates ?? [];
  const extension = fileExtension(file.fileName);

  return (
    <aside className="detail-panel">
      <div className="panel-heading">
        <div>
          <h2>详情</h2>
          <p title={file.sourcePath}>{file.sourcePath}</p>
        </div>
      </div>

      <dl className="facts">
        <div>
          <dt>最终标题</dt>
          <dd>{file.recognizedTitle ?? "—"}</dd>
        </div>
        <div>
          <dt>置信度</dt>
          <dd>{file.confidence != null ? `${file.confidence}%` : "—"}</dd>
        </div>
        <div>
          <dt>输出路径</dt>
          <dd>{file.outputPath ?? "—"}</dd>
        </div>
        <div>
          <dt>失败原因</dt>
          <dd>{file.failureReason ?? file.error?.technicalDetail ?? "—"}</dd>
        </div>
        <div>
          <dt>处理日志</dt>
          <dd>
            {file.progressStage ? stageLabel[file.progressStage] ?? file.progressStage : "—"}
            {file.extractMethod ? ` · ${file.extractMethod}` : ""}
          </dd>
        </div>
      </dl>

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

      <section>
        <h3>候选标题</h3>
        <div className="candidate-list">
          {candidates.map((candidate) => (
            <details key={`${candidate.source}-${candidate.text}-${candidate.score}`} open>
              <summary>
                <span>{candidate.text}</span>
                <strong>{candidate.score}%</strong>
              </summary>
              <div className="score-grid">
                <span>版式 {candidate.categoryScores.layout}</span>
                <span>位置 {candidate.categoryScores.position}</span>
                <span>关键词 {candidate.categoryScores.keyword}</span>
                <span>文本质量 {candidate.categoryScores.textQuality}</span>
                <span>惩罚 {candidate.categoryScores.penalty}</span>
              </div>
              <ul className="rule-list">
                {candidate.ruleDetails.map((rule) => (
                  <li key={`${rule.ruleName}-${rule.delta}`}>
                    <span>{rule.ruleName}</span>
                    <span>{rule.delta > 0 ? `+${rule.delta}` : rule.delta}</span>
                    <small>{rule.description}</small>
                  </li>
                ))}
              </ul>
            </details>
          ))}
          {candidates.length === 0 ? <p className="muted">暂无候选明细。</p> : null}
        </div>
      </section>
    </aside>
  );
}

function HistoryView() {
  const [page, setPage] = useState<HistoryBatchPage>({ batches: [], total: 0 });
  const [detail, setDetail] = useState<HistoryBatchDetail | null>(null);
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
        {page.batches.map((batch) => (
          <div className="history-row" key={batch.batchId}>
            <div>
              <strong>{batch.batchId}</strong>
              <span>{batch.createdAt}</span>
              <small>{summaryText(batch.summary)}</small>
            </div>
            <button
              className="secondary"
              onClick={async () => {
                setDetail(await getHistoryBatch(batch.batchId));
                setMessage("");
              }}
            >
              查看详情
            </button>
          </div>
        ))}
        {page.batches.length === 0 ? <p className="empty">暂无历史记录。</p> : null}
      </div>

      <aside className="detail-panel">
        <div className="panel-heading">
          <div>
            <h2>批次详情</h2>
            <p>{detail ? summaryText(detail.summary) : "选择左侧批次"}</p>
          </div>
          <button
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
        {detail?.files.map((result) => (
          <div className="detail-file" key={result.file.fileJobId}>
            <strong>{result.file.fileName}</strong>
            <span>{result.file.recognizedTitle ?? result.error?.userMessage ?? "—"}</span>
            <small>{result.file.outputPath ?? result.file.failureReason ?? result.file.sourcePath}</small>
          </div>
        ))}
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
    <section className="settings-layout">
      <div className="settings-panel">
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
        </div>
      </div>

      <RuleEditor
        title="关键词规则"
        keywordRules={draft.keywordRules}
        regexRules={draft.regexRules}
      />

      <div className="settings-actions">
        <button onClick={() => void settingsStore.save()} disabled={state.saving}>
          保存设置
        </button>
        <PathAction label="导入设置" onSubmit={(path) => settingsStore.importFrom(path)} />
        <PathAction label="导出设置" onSubmit={(path) => settingsStore.exportTo(path)} />
        <button className="secondary" onClick={() => void settingsStore.reset()}>
          恢复默认
        </button>
      </div>
      {state.message ? <p className="notice">{state.message}</p> : null}
      {state.error ? <p className="error">{state.error}</p> : null}
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
    <div className="settings-panel">
      <div className="panel-heading">
        <div>
          <h2>{title}</h2>
          <p>{keywordRules.length} 个关键词 · {regexRules.length} 个正则</p>
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

      <div className="rule-editor">
        {keywordRules.map((rule, index) => (
          <div className="rule-row" key={`keyword-${index}`}>
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
          <div className="rule-row" key={`regex-${index}`}>
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
