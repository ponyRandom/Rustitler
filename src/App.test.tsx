import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultSettings, fileView, scoringResult } from "./test/fixtures";
import type { ClassificationSummary } from "./types/ipc";

const mocks = vi.hoisted(() => ({
  startBatch: vi.fn(),
  cancelBatch: vi.fn(),
  getBatchState: vi.fn(),
  confirmPendingOutput: vi.fn(),
  selectCandidateTitle: vi.fn(),
  undoBatch: vi.fn(),
  listHistory: vi.fn(),
  getHistoryBatch: vi.fn(),
  loadSettings: vi.fn(),
  saveSettings: vi.fn(),
  importSettings: vi.fn(),
  exportSettings: vi.fn(),
  resetSettings: vi.fn(),
  classifyFolder: vi.fn(),
  subscribeBatchEvents: vi.fn(),
  subscribeFileDrops: vi.fn(),
  selectFiles: vi.fn(),
  selectFolder: vi.fn(),
  dropHandler: undefined as undefined | ((paths: string[]) => void),
  activeHandler: undefined as undefined | ((active: boolean) => void),
}));

const settingsFixture = {
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
  classificationSettings: defaultSettings().classificationSettings,
  debugMode: false,
};

vi.mock("./api/commands", () => ({
  startBatch: mocks.startBatch,
  cancelBatch: mocks.cancelBatch,
  getBatchState: mocks.getBatchState,
  confirmPendingOutput: mocks.confirmPendingOutput,
  selectCandidateTitle: mocks.selectCandidateTitle,
  undoBatch: mocks.undoBatch,
  listHistory: mocks.listHistory,
  getHistoryBatch: mocks.getHistoryBatch,
  loadSettings: mocks.loadSettings,
  saveSettings: mocks.saveSettings,
  importSettings: mocks.importSettings,
  exportSettings: mocks.exportSettings,
  resetSettings: mocks.resetSettings,
  classifyFolder: mocks.classifyFolder,
}));

vi.mock("./api/events", () => ({
  subscribeBatchEvents: mocks.subscribeBatchEvents,
}));

vi.mock("./api/dragDrop", () => ({
  subscribeFileDrops: mocks.subscribeFileDrops,
}));

vi.mock("./api/fileDialog", () => ({
  selectFiles: mocks.selectFiles,
  selectFolder: mocks.selectFolder,
}));

const renderApp = async () => {
  const { default: App } = await import("./App");
  return render(<App />);
};

const waitForQueueReady = async () => {
  const toolbar = screen.getByRole("banner", { name: "应用工具栏" });
  await waitFor(() =>
    expect(within(toolbar).getByRole("button", { name: "导入文件" })).toBeEnabled(),
  );
};

const classificationSummary = (
  overrides: Partial<ClassificationSummary> = {},
): ClassificationSummary => ({
  sourcePath: "/input/classify-source",
  outputPath: "/input/Rustitler 分类输出 2026-07-03 1530",
  totalFiles: 6,
  copiedFiles: 5,
  failedFiles: 1,
  categoryCounts: [
    { category: "请示", count: 1 },
    { category: "报告", count: 1 },
    { category: "通知", count: 1 },
    { category: "待确认", count: 1 },
    { category: "其他", count: 1 },
  ],
  failures: [{ sourcePath: "/input/classify-source/broken.pdf", reason: "文件复制失败" }],
  ...overrides,
});

const deferred = <T,>() => {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });
  return { promise, resolve, reject };
};

describe("App", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    mocks.dropHandler = undefined;
    mocks.activeHandler = undefined;
    mocks.startBatch.mockResolvedValue("batch-1");
    mocks.cancelBatch.mockResolvedValue(undefined);
    mocks.getBatchState.mockResolvedValue(null);
    mocks.confirmPendingOutput.mockResolvedValue({});
    mocks.selectCandidateTitle.mockResolvedValue({});
    mocks.undoBatch.mockResolvedValue({ deleted: 1, skippedMissing: 0, skippedModified: 0 });
    mocks.listHistory.mockResolvedValue({
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
    });
    mocks.getHistoryBatch.mockResolvedValue({
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
          file: {
            fileJobId: "file-1",
            batchId: "batch-1",
            sourcePath: "/input/source.pdf",
            fileName: "source.pdf",
            fileType: "pdf",
            status: "outputCreated",
            recognizedTitle: "项目通知",
            confidence: 84,
            outputPath: "/input/Rustitler 输出/项目通知.pdf",
          },
          sourceFingerprint: {
            normalizedPath: "/input/source.pdf",
            sizeBytes: 2048,
            modifiedTime: "2026-06-27T00:00:00Z",
          },
        },
      ],
    });
    mocks.loadSettings.mockResolvedValue(settingsFixture);
    mocks.saveSettings.mockImplementation(async (settings) => settings);
    mocks.importSettings.mockResolvedValue(settingsFixture);
    mocks.exportSettings.mockResolvedValue(undefined);
    mocks.resetSettings.mockResolvedValue(settingsFixture);
    mocks.classifyFolder.mockResolvedValue(classificationSummary({ failedFiles: 0, failures: [] }));
    mocks.subscribeBatchEvents.mockResolvedValue(() => undefined);
    mocks.subscribeFileDrops.mockImplementation(async (onDrop, onActiveChange) => {
      mocks.dropHandler = onDrop;
      mocks.activeHandler = onActiveChange;
      return () => undefined;
    });
    mocks.selectFiles.mockResolvedValue([]);
    mocks.selectFolder.mockResolvedValue([]);
  });

  it("renders the main queue, history, and settings workflows", async () => {
    await renderApp();

    expect(screen.getByRole("heading", { name: "Rustitler" })).toBeInTheDocument();
    expect(screen.getByText("拖入文件或文件夹开始处理")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "历史" }));
    await waitFor(() => expect(screen.getByText("batch-1")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "查看详情" }));
    await waitFor(() => expect(screen.getByText("项目通知")).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    await waitFor(() => expect(screen.getByLabelText("自动输出阈值")).toHaveValue(70));
    expect(screen.getByLabelText("标题最大字数")).toHaveValue(45);
    fireEvent.change(screen.getByLabelText("自动输出阈值"), { target: { value: "80" } });
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
    await waitFor(() => expect(screen.getByText("设置已保存")).toBeInTheDocument());
  });

  it("renders the macOS shell", async () => {
    await renderApp();

    expect(screen.getByRole("navigation", { name: "主导航" })).toHaveClass("sidebar-nav");
    expect(screen.getByRole("banner", { name: "应用工具栏" })).toHaveClass("toolbar");
    expect(screen.queryByRole("status", { name: "服务状态" })).not.toBeInTheDocument();
    expect(screen.queryByText("服务正常")).not.toBeInTheDocument();
  });

  it("starts a batch from dropped file paths", async () => {
    await renderApp();

    await waitFor(() => expect(mocks.dropHandler).toBeDefined());
    await waitForQueueReady();
    act(() => {
      mocks.dropHandler?.(["/input/a.pdf", "/input/folder"]);
    });

    await waitFor(() =>
      expect(mocks.startBatch).toHaveBeenCalledWith(["/input/a.pdf", "/input/folder"], settingsFixture),
    );
  });

  it("shows a newly dropped batch after a previous batch completed", async () => {
    let emitBatchEvent: ((event: unknown) => void) | undefined;
    mocks.subscribeBatchEvents.mockImplementation(async (handler) => {
      emitBatchEvent = handler;
      return () => undefined;
    });
    mocks.startBatch.mockImplementation(async () => {
      emitBatchEvent?.({ type: "BatchStarted", batchId: "batch-2", createdAt: "later", totalFiles: 1 });
      emitBatchEvent?.({
        type: "FileQueued",
        batchId: "batch-2",
        file: fileView({
          batchId: "batch-2",
          fileJobId: "file-new",
          fileName: "new.pdf",
          sourcePath: "/input/new.pdf",
          status: "queued",
        }),
      });
      return "batch-2";
    });
    await renderApp();

    await waitFor(() => expect(emitBatchEvent).toBeDefined());
    await waitForQueueReady();
    act(() => {
      emitBatchEvent?.({ type: "BatchStarted", batchId: "batch-1", createdAt: "now", totalFiles: 1 });
      emitBatchEvent?.({
        type: "FileQueued",
        batchId: "batch-1",
        file: fileView({
          fileJobId: "file-old",
          fileName: "old.pdf",
          sourcePath: "/input/old.pdf",
          status: "outputCreated",
          outputPath: "/input/Rustitler 输出/old.pdf",
        }),
      });
      emitBatchEvent?.({
        type: "BatchCompleted",
        batchId: "batch-1",
        summary: {
          total: 1,
          outputCreated: 1,
          pending: 0,
          skipped: 0,
          failed: 0,
          cancelled: 0,
        },
      });
    });

    await act(async () => {
      mocks.dropHandler?.(["/input/new.pdf"]);
      await Promise.resolve();
    });

    expect(screen.getByRole("button", { name: /new\.pdf/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /old\.pdf/ })).not.toBeInTheDocument();
  });

  it("shows an error instead of ignoring drops while settings are loading", async () => {
    mocks.loadSettings.mockImplementation(() => new Promise(() => undefined));
    await renderApp();

    await waitFor(() => expect(mocks.dropHandler).toBeDefined());
    act(() => {
      mocks.dropHandler?.(["/input/a.pdf"]);
    });

    expect(await screen.findByText("设置仍在加载，请稍后再导入。")).toBeInTheDocument();
    expect(mocks.startBatch).not.toHaveBeenCalled();
  });

  it("shows settings load errors on the queue view", async () => {
    mocks.loadSettings.mockRejectedValue(new Error("settings.json 无法读取"));
    await renderApp();

    expect(await screen.findByText("settings.json 无法读取")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "导入文件" })).toBeDisabled();
  });

  it("offers explicit file and folder import actions", async () => {
    await renderApp();

    const toolbar = screen.getByRole("banner", { name: "应用工具栏" });
    expect(within(toolbar).getByRole("button", { name: "导入文件" })).toBeInTheDocument();
    expect(within(toolbar).getByRole("button", { name: "导入文件夹" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "选择文件" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "选择文件夹" })).not.toBeInTheDocument();
  });

  it("shows a distinct folder classification action in the queue toolbar", async () => {
    await renderApp();

    const toolbar = screen.getByRole("banner", { name: "应用工具栏" });
    expect(within(toolbar).getByRole("button", { name: "分类文件夹" })).toBeInTheDocument();
  });

  it("does not classify when folder selection is cancelled", async () => {
    mocks.selectFolder.mockResolvedValue([]);
    await renderApp();

    await waitForQueueReady();
    fireEvent.click(screen.getByRole("button", { name: "分类文件夹" }));

    await waitFor(() => expect(mocks.selectFolder).toHaveBeenCalledTimes(1));
    expect(mocks.classifyFolder).not.toHaveBeenCalled();
  });

  it("classifies the selected folder with the current classification settings snapshot", async () => {
    mocks.selectFolder.mockResolvedValue(["/input/classify-source"]);
    await renderApp();

    await waitForQueueReady();
    fireEvent.click(screen.getByRole("button", { name: "分类文件夹" }));

    await waitFor(() =>
      expect(mocks.classifyFolder).toHaveBeenCalledWith(
        "/input/classify-source",
        settingsFixture.classificationSettings,
      ),
    );
    expect(mocks.startBatch).not.toHaveBeenCalled();
  });

  it("marks the classification action busy while classification is running", async () => {
    const pendingClassification = deferred<ClassificationSummary>();
    mocks.selectFolder.mockResolvedValue(["/input/classify-source"]);
    mocks.classifyFolder.mockReturnValue(pendingClassification.promise);
    await renderApp();

    await waitForQueueReady();
    fireEvent.click(screen.getByRole("button", { name: "分类文件夹" }));

    const busyButton = await screen.findByRole("button", { name: "分类中..." });
    expect(busyButton).toBeDisabled();
    expect(screen.queryByRole("button", { name: "取消分类" })).not.toBeInTheDocument();

    await act(async () => {
      pendingClassification.resolve(classificationSummary({ failedFiles: 0, failures: [] }));
      await pendingClassification.promise;
    });
    await waitFor(() => expect(screen.getByRole("button", { name: "分类文件夹" })).toBeEnabled());
  });

  it("shows a classification success summary without an empty failure list", async () => {
    mocks.selectFolder.mockResolvedValue(["/input/classify-source"]);
    mocks.classifyFolder.mockResolvedValue(
      classificationSummary({
        totalFiles: 5,
        copiedFiles: 5,
        failedFiles: 0,
        failures: [],
      }),
    );
    await renderApp();

    await waitForQueueReady();
    fireEvent.click(screen.getByRole("button", { name: "分类文件夹" }));

    const result = await screen.findByRole("region", { name: "分类结果" });
    expect(within(result).getByText("/input/classify-source")).toBeInTheDocument();
    expect(within(result).getByText("/input/Rustitler 分类输出 2026-07-03 1530")).toBeInTheDocument();
    expect(within(result).getByText("总文件数")).toBeInTheDocument();
    expect(within(result).getByText("5")).toBeInTheDocument();
    expect(within(result).getByText("成功复制")).toBeInTheDocument();
    expect(within(result).getByText("失败文件")).toBeInTheDocument();
    expect(within(result).getByText("请示")).toBeInTheDocument();
    expect(within(result).queryByRole("list", { name: "失败明细" })).not.toBeInTheDocument();
  });

  it("lays out classification paths and counts for long Windows folders", async () => {
    const rendered = await renderApp();
    mocks.selectFolder.mockResolvedValue(["C:\\Users\\22418\\Desktop\\standards"]);
    mocks.classifyFolder.mockResolvedValue(
      classificationSummary({
        sourcePath: "C:\\Users\\22418\\Desktop\\standards\\learning\\deeply\\nested\\source",
        outputPath:
          "C:\\Users\\22418\\Desktop\\standards\\Rustitler classification output 2026-07-03 1530",
        categoryCounts: [
          { category: "Request", count: 1 },
          { category: "Report", count: 2 },
          { category: "Notice", count: 3 },
          { category: "Standard", count: 4 },
          { category: "Review", count: 5 },
          { category: "Other", count: 6 },
        ],
        failures: [],
        failedFiles: 0,
      }),
    );

    await waitForQueueReady();
    fireEvent.click(rendered.container.querySelector(".classify-action") as HTMLElement);

    await waitFor(() => expect(rendered.container.querySelector(".classification-result")).toBeInTheDocument());
    expect(rendered.container.querySelector(".classification-path-facts")).toBeInTheDocument();
    expect(rendered.container.querySelectorAll(".classification-path-fact")).toHaveLength(2);
    expect(rendered.container.querySelector(".classification-stat-facts")).toBeInTheDocument();
    expect(rendered.container.querySelector(".category-counts-scroller")).toBeInTheDocument();
  });

  it("shows classification failure details when individual files fail", async () => {
    mocks.selectFolder.mockResolvedValue(["/input/classify-source"]);
    mocks.classifyFolder.mockResolvedValue(classificationSummary());
    await renderApp();

    await waitForQueueReady();
    fireEvent.click(screen.getByRole("button", { name: "分类文件夹" }));

    const failures = await screen.findByRole("list", { name: "失败明细" });
    expect(within(failures).getByText("/input/classify-source/broken.pdf")).toBeInTheDocument();
    expect(within(failures).getByText("文件复制失败")).toBeInTheDocument();
  });

  it("shows batch-level classification errors and restores the classify action", async () => {
    mocks.selectFolder.mockResolvedValue(["/missing"]);
    mocks.classifyFolder.mockRejectedValue({
      code: "invalidCommandArgument",
      category: "input",
      userMessage: "源文件夹不存在",
      retryable: false,
    });
    await renderApp();

    await waitForQueueReady();
    fireEvent.click(screen.getByRole("button", { name: "分类文件夹" }));

    expect(await screen.findByText("源文件夹不存在")).toBeInTheDocument();
    expect(screen.getByText("invalidCommandArgument")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "分类文件夹" })).toBeEnabled();
  });

  it("keeps the empty queue as a composed panel instead of a horizontally scrolling table", async () => {
    await renderApp();

    expect(screen.getByRole("status", { name: "队列为空" })).toHaveClass("queue-empty");
    expect(screen.queryByRole("table", { name: "文件队列" })).not.toBeInTheDocument();
  });

  it("uses a macOS queue workspace", async () => {
    await renderApp();

    await waitForQueueReady();
    expect(screen.getByRole("region", { name: "队列工作区" })).toHaveClass("queue-layout");
    expect(screen.getByRole("region", { name: "文件队列" })).toHaveClass("queue-panel");
    expect(screen.getByRole("complementary", { name: "文件检查器" })).toHaveClass("detail-panel");
    expect(screen.queryByText("支持 PDF、Word、图片；文件夹仅扫描第一层。")).not.toBeInTheDocument();
    expect(screen.queryByText("导入文件后，这里会按文件展示状态、标题和输出结果。")).not.toBeInTheDocument();
    expect(
      screen.queryByText("选择一个队列项查看文件地址、最终标题、候选标题和重复提示。"),
    ).not.toBeInTheDocument();
    expect(
      within(screen.getByRole("banner", { name: "应用工具栏" })).getByRole("button", { name: "导入文件" }),
    ).toBeEnabled();
  });

  it("renders completed queue rows as selected file cards with visible status text", async () => {
    let emitBatchEvent: ((event: unknown) => void) | undefined;
    mocks.subscribeBatchEvents.mockImplementation(async (handler) => {
      emitBatchEvent = handler;
      return () => undefined;
    });
    await renderApp();

    await waitFor(() => expect(emitBatchEvent).toBeDefined());
    act(() => {
      emitBatchEvent?.({ type: "BatchStarted", batchId: "batch-1", createdAt: "now", totalFiles: 1 });
      emitBatchEvent?.({
        type: "FileQueued",
        batchId: "batch-1",
        file: fileView({
          fileJobId: "file-complete",
          fileName: "done.pdf",
          sourcePath: "/input/done.pdf",
          status: "outputCreated",
          recognizedTitle: "已完成标题",
          confidence: 100,
          outputPath: "/input/Rustitler 输出/已完成标题.pdf",
        }),
      });
    });

    const fileCard = screen.getByRole("button", { name: /done\.pdf/ });
    expect(fileCard).toHaveClass("file-card", "selected");
    expect(within(fileCard).getByText("已输出")).toBeVisible();
    expect(within(fileCard).getByText("100%")).toBeVisible();
  });

  it("renders history batches as compact cards without exposing long ids in the visible list", async () => {
    const longBatchId = "20f4ecce-7718-4eac-94bc-b30926330aad";
    mocks.listHistory.mockResolvedValue({
      total: 1,
      batches: [
        {
          batchId: longBatchId,
          createdAt: "2026-06-29T14:38:17.960285+00:00",
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
    });

    await renderApp();
    fireEvent.click(screen.getByRole("button", { name: "历史" }));

    const historyList = await screen.findByRole("list", { name: "历史批次" });
    expect(within(historyList).getByText("20f4ecce...30aad")).toBeInTheDocument();
    expect(within(historyList).queryByText(longBatchId)).not.toBeInTheDocument();
    expect(within(historyList).getByRole("button", { name: "查看详情" })).toHaveClass("secondary");
  });

  it("separates settings content from the bottom action bar", async () => {
    await renderApp();

    fireEvent.click(screen.getByRole("button", { name: "设置" }));

    const settingsRegion = await screen.findByRole("region", { name: "设置" });
    expect(within(settingsRegion).getByLabelText("设置内容")).toHaveClass("settings-content");
    expect(within(settingsRegion).getByRole("group", { name: "设置操作" })).toHaveClass("settings-footer");
    expect(within(settingsRegion).getByRole("list", { name: "关键词和正则规则" })).toBeInTheDocument();
  });

  it("edits classification categories and keywords from settings", async () => {
    await renderApp();

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    const settingsRegion = await screen.findByRole("region", { name: "设置" });
    const classificationPanel = within(settingsRegion).getByRole("group", { name: "分类配置" });

    expect(within(classificationPanel).getByRole("list", { name: "分类列表" })).toBeInTheDocument();

    fireEvent.click(within(classificationPanel).getByRole("button", { name: "添加分类" }));
    fireEvent.change(within(classificationPanel).getByLabelText("分类名称 5"), {
      target: { value: "合同" },
    });
    fireEvent.change(within(classificationPanel).getByLabelText("分类关键词 5-1"), {
      target: { value: "协议" },
    });
    fireEvent.click(within(classificationPanel).getByRole("button", { name: "添加关键词 5" }));
    fireEvent.change(within(classificationPanel).getByLabelText("分类关键词 5-2"), {
      target: { value: "补充协议" },
    });

    expect(within(classificationPanel).getByDisplayValue("合同")).toBeInTheDocument();
    expect(within(classificationPanel).getByDisplayValue("协议")).toBeInTheDocument();
    expect(within(classificationPanel).getByDisplayValue("补充协议")).toBeInTheDocument();

    fireEvent.click(within(classificationPanel).getByRole("button", { name: "删除关键词 5-2" }));
    expect(within(classificationPanel).queryByDisplayValue("补充协议")).not.toBeInTheDocument();

    fireEvent.click(within(classificationPanel).getByRole("button", { name: "删除分类 5" }));
    expect(within(classificationPanel).queryByDisplayValue("合同")).not.toBeInTheDocument();
  });

  it("shows backend classification validation errors already held in settings state", async () => {
    mocks.saveSettings.mockRejectedValue({
      code: "invalidSettings",
      category: "settings",
      userMessage: "清洗后分类名称重复",
      retryable: true,
    });
    await renderApp();

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    await screen.findByRole("group", { name: "分类配置" });
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));

    expect(await screen.findByText("清洗后分类名称重复")).toBeInTheDocument();
  });

  it("keeps history and settings in macOS groups", async () => {
    await renderApp();

    fireEvent.click(screen.getByRole("button", { name: "历史" }));
    expect(await screen.findByRole("complementary", { name: "批次检查器" })).toHaveClass("detail-panel");

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    const settingsRegion = await screen.findByRole("region", { name: "设置" });
    expect(within(settingsRegion).getByRole("group", { name: "评分设置" })).toHaveClass("settings-panel");
    expect(within(settingsRegion).getByRole("group", { name: "关键词规则" })).toHaveClass("settings-panel");
  });

  it("starts a batch from selected files", async () => {
    mocks.selectFiles.mockResolvedValue(["/input/a.pdf", "/input/b.docx"]);
    await renderApp();

    await waitForQueueReady();
    fireEvent.click(screen.getByRole("button", { name: "导入文件" }));

    await waitFor(() =>
      expect(mocks.startBatch).toHaveBeenCalledWith(["/input/a.pdf", "/input/b.docx"], settingsFixture),
    );
  });

  it("shows a useful error when the file picker fails", async () => {
    mocks.selectFiles.mockRejectedValue(new Error("dialog not available"));
    await renderApp();

    await waitForQueueReady();
    fireEvent.click(screen.getByRole("button", { name: "导入文件" }));

    expect(await screen.findByText("dialog not available")).toBeInTheDocument();
    expect(mocks.startBatch).not.toHaveBeenCalled();
  });

  it("shows a useful error when starting an import fails", async () => {
    mocks.selectFiles.mockResolvedValue(["/input/a.pdf"]);
    mocks.startBatch.mockRejectedValue({
      userMessage: "无法读取所选文件，请检查权限。",
    });
    await renderApp();

    await waitForQueueReady();
    fireEvent.click(screen.getByRole("button", { name: "导入文件" }));

    expect(await screen.findByText("无法读取所选文件，请检查权限。")).toBeInTheDocument();
  });

  it("starts a batch from a selected folder", async () => {
    mocks.selectFolder.mockResolvedValue(["/input/folder"]);
    await renderApp();

    await waitForQueueReady();
    fireEvent.click(screen.getByRole("button", { name: "导入文件夹" }));

    await waitFor(() =>
      expect(mocks.startBatch).toHaveBeenCalledWith(["/input/folder"], settingsFixture),
    );
  });

  it("shows duplicate warnings without blocking output", async () => {
    let emitBatchEvent: ((event: unknown) => void) | undefined;
    mocks.subscribeBatchEvents.mockImplementation(async (handler) => {
      emitBatchEvent = handler;
      return () => undefined;
    });
    await renderApp();

    await waitFor(() => expect(emitBatchEvent).toBeDefined());
    act(() => {
      emitBatchEvent?.({ type: "BatchStarted", batchId: "batch-1", createdAt: "now", totalFiles: 1 });
      emitBatchEvent?.({
        type: "FileQueued",
        batchId: "batch-1",
        file: fileView({
          fileJobId: "file-duplicate",
          fileName: "2.pdf",
          sourcePath: "/Users/example/Desktop/2.pdf",
          status: "queued",
          duplicateWarning:
            "疑似重复：历史批次 batch-old 的文件 file-old 已输出到 /Users/example/Desktop/Rustitler 输出/旧标题.pdf。",
        }),
      });
      emitBatchEvent?.({
        type: "FileProgress",
        batchId: "batch-1",
        fileJobId: "file-duplicate",
        stage: "extract",
        progress: 0,
      });
      emitBatchEvent?.({
        type: "FileOutputCreated",
        batchId: "batch-1",
        fileJobId: "file-duplicate",
        outputPath: "/Users/example/Desktop/Rustitler 输出/新标题.pdf",
      });
    });

    expect(screen.getAllByText("/Users/example/Desktop/Rustitler 输出/新标题.pdf").length).toBeGreaterThan(0);
    const detailPanel = screen.getByRole("heading", { name: "详情" }).closest("aside");
    expect(detailPanel).not.toBeNull();
    expect(within(detailPanel!).getByText("文件地址")).toBeInTheDocument();
    expect(within(detailPanel!).getByText("/Users/example/Desktop/2.pdf")).toBeInTheDocument();
    expect(within(detailPanel!).getByText("疑似重复")).toBeInTheDocument();
    expect(within(detailPanel!).getByText("可能已处理过，请核对历史输出。")).toBeInTheDocument();
    expect(within(detailPanel!).queryByText(/batch-old/)).not.toBeInTheDocument();
    expect(within(detailPanel!).queryByText(/file-old/)).not.toBeInTheDocument();
    expect(within(detailPanel!).queryByText(/旧标题\.pdf/)).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "处理信息" })).not.toBeInTheDocument();
    expect(screen.queryByText("处理日志")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("文件名主体")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "确认输出" })).not.toBeInTheDocument();
  });

  it("shows five candidate titles without detailed scoring", async () => {
    let emitBatchEvent: ((event: unknown) => void) | undefined;
    mocks.subscribeBatchEvents.mockImplementation(async (handler) => {
      emitBatchEvent = handler;
      return () => undefined;
    });
    await renderApp();

    await waitFor(() => expect(emitBatchEvent).toBeDefined());
    const candidates = Array.from({ length: 6 }, (_, index) => ({
      text: `候选标题 ${index + 1}`,
      source: "pdfLayout" as const,
      pageIndex: 0,
      score: 90 - index,
      categoryScores: {
        layout: 30,
        position: 20,
        keyword: 15,
        textQuality: 20,
        penalty: 0,
      },
      ruleDetails: [],
    }));
    act(() => {
      emitBatchEvent?.({ type: "BatchStarted", batchId: "batch-1", createdAt: "now", totalFiles: 1 });
      emitBatchEvent?.({
        type: "FileQueued",
        batchId: "batch-1",
        file: fileView({
          fileJobId: "file-candidates",
          fileName: "many.pdf",
          sourcePath: "/input/many.pdf",
          status: "queued",
        }),
      });
      emitBatchEvent?.({
        type: "FileScored",
        batchId: "batch-1",
        fileJobId: "file-candidates",
        result: scoringResult({
          finalTitle: "候选标题 1",
          confidence: 90,
          candidates,
        }),
      });
    });

    const detailPanel = screen.getByRole("heading", { name: "详情" }).closest("aside");
    expect(detailPanel).not.toBeNull();
    expect(within(detailPanel!).getByText("最终标题")).toBeInTheDocument();
    const candidatesSection = within(detailPanel!)
      .getByRole("heading", { name: "候选标题" })
      .closest("section");
    expect(candidatesSection).not.toBeNull();
    expect(
      within(candidatesSection!).getByRole("group", { name: "候选标题操作" }),
    ).toContainElement(within(candidatesSection!).getByRole("button", { name: "确认使用该标题" }));
    const candidateList = within(detailPanel!).getByRole("list", { name: "候选标题列表" });
    expect(within(candidateList).getByRole("button", { name: "候选标题 1" })).toBeInTheDocument();
    expect(within(candidateList).getByRole("button", { name: "候选标题 2" })).toBeInTheDocument();
    expect(within(candidateList).getByRole("button", { name: "候选标题 3" })).toBeInTheDocument();
    expect(within(candidateList).getByRole("button", { name: "候选标题 4" })).toBeInTheDocument();
    expect(within(candidateList).getByRole("button", { name: "候选标题 5" })).toBeInTheDocument();
    expect(within(candidateList).queryByRole("button", { name: "候选标题 6" })).not.toBeInTheDocument();
    expect(screen.queryByText("版式")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "使用" })).not.toBeInTheDocument();
  });

  it("requires explicit confirmation before applying a candidate title", async () => {
    let emitBatchEvent: ((event: unknown) => void) | undefined;
    mocks.subscribeBatchEvents.mockImplementation(async (handler) => {
      emitBatchEvent = handler;
      return () => undefined;
    });
    mocks.selectCandidateTitle.mockResolvedValue(
      fileView({
        fileJobId: "file-confirm-candidate",
        fileName: "confirm.pdf",
        sourcePath: "/input/confirm.pdf",
        status: "outputCreated",
        recognizedTitle: "候选标题 2",
        confidence: 76,
        outputPath: "/input/Rustitler 输出/候选标题 2.pdf",
      }),
    );
    await renderApp();

    await waitFor(() => expect(emitBatchEvent).toBeDefined());
    await waitForQueueReady();
    act(() => {
      emitBatchEvent?.({ type: "BatchStarted", batchId: "batch-1", createdAt: "now", totalFiles: 1 });
      emitBatchEvent?.({
        type: "FileQueued",
        batchId: "batch-1",
        file: fileView({
          fileJobId: "file-confirm-candidate",
          fileName: "confirm.pdf",
          sourcePath: "/input/confirm.pdf",
          status: "outputCreated",
          recognizedTitle: "候选标题 1",
          confidence: 90,
          outputPath: "/input/Rustitler 输出/候选标题 1.pdf",
        }),
      });
      emitBatchEvent?.({
        type: "FileScored",
        batchId: "batch-1",
        fileJobId: "file-confirm-candidate",
        result: scoringResult({
          finalTitle: "候选标题 1",
          confidence: 90,
          candidates: [
            {
              text: "候选标题 1",
              source: "pdfLayout",
              pageIndex: 0,
              score: 90,
              categoryScores: {
                layout: 30,
                position: 20,
                keyword: 15,
                textQuality: 20,
                penalty: 0,
              },
              ruleDetails: [],
            },
            {
              text: "候选标题 2",
              source: "pdfLayout",
              pageIndex: 0,
              score: 76,
              categoryScores: {
                layout: 24,
                position: 18,
                keyword: 14,
                textQuality: 20,
                penalty: 0,
              },
              ruleDetails: [],
            },
          ],
        }),
      });
    });

    const detailPanel = screen.getByRole("heading", { name: "详情" }).closest("aside");
    expect(detailPanel).not.toBeNull();
    const candidateList = within(detailPanel!).getByRole("list", { name: "候选标题列表" });

    fireEvent.click(within(candidateList).getByRole("button", { name: "候选标题 2" }));

    expect(mocks.selectCandidateTitle).not.toHaveBeenCalled();
    expect(within(candidateList).getByRole("button", { name: "候选标题 2" })).toHaveClass("active");

    await act(async () => {
      fireEvent.click(within(detailPanel!).getByRole("button", { name: "确认使用该标题" }));
      await Promise.resolve();
    });

    await waitFor(() =>
      expect(mocks.selectCandidateTitle).toHaveBeenCalledWith("file-confirm-candidate", "候选标题 2"),
    );
  });

  it("renders long candidate titles with a multiline text element", async () => {
    let emitBatchEvent: ((event: unknown) => void) | undefined;
    mocks.subscribeBatchEvents.mockImplementation(async (handler) => {
      emitBatchEvent = handler;
      return () => undefined;
    });
    await renderApp();

    const longTitle = "关于统筹推进自然保护地和生态保护红线生态环境监管有关事项的通知";
    await waitFor(() => expect(emitBatchEvent).toBeDefined());
    await waitForQueueReady();
    act(() => {
      emitBatchEvent?.({ type: "BatchStarted", batchId: "batch-1", createdAt: "now", totalFiles: 1 });
      emitBatchEvent?.({
        type: "FileQueued",
        batchId: "batch-1",
        file: fileView({
          fileJobId: "file-long-title",
          fileName: "long.pdf",
          sourcePath: "/input/long.pdf",
          status: "outputCreated",
          recognizedTitle: longTitle,
          confidence: 92,
          outputPath: `/input/Rustitler 输出/${longTitle}.pdf`,
        }),
      });
      emitBatchEvent?.({
        type: "FileScored",
        batchId: "batch-1",
        fileJobId: "file-long-title",
        result: scoringResult({
          finalTitle: longTitle,
          confidence: 92,
          candidates: [
            {
              text: longTitle,
              source: "pdfLayout",
              pageIndex: 0,
              score: 92,
              categoryScores: {
                layout: 30,
                position: 20,
                keyword: 15,
                textQuality: 20,
                penalty: 0,
              },
              ruleDetails: [],
            },
          ],
        }),
      });
    });

    const detailPanel = screen.getByRole("heading", { name: "详情" }).closest("aside");
    expect(detailPanel).not.toBeNull();
    const candidateButton = within(detailPanel!).getByRole("button", { name: longTitle });
    expect(within(candidateButton).getByText(longTitle)).toHaveClass("candidate-title-text");
    expect(within(detailPanel!).getAllByText(longTitle).length).toBeGreaterThanOrEqual(2);
  });

  it("keeps file details concise with address, final title, duplicate notice, and candidate titles", async () => {
    let emitBatchEvent: ((event: unknown) => void) | undefined;
    mocks.subscribeBatchEvents.mockImplementation(async (handler) => {
      emitBatchEvent = handler;
      return () => undefined;
    });
    await renderApp();

    await waitFor(() => expect(emitBatchEvent).toBeDefined());
    act(() => {
      emitBatchEvent?.({ type: "BatchStarted", batchId: "batch-1", createdAt: "now", totalFiles: 1 });
      emitBatchEvent?.({
        type: "FileQueued",
        batchId: "batch-1",
        file: fileView({
          fileJobId: "file-details",
          fileName: "details.pdf",
          sourcePath: "/input/details.pdf",
          status: "queued",
          duplicateWarning: "疑似重复：历史批次 batch-old 的文件 file-old 已输出到 /output/旧文件.pdf。",
        }),
      });
      emitBatchEvent?.({
        type: "FileScored",
        batchId: "batch-1",
        fileJobId: "file-details",
        result: scoringResult({
          finalTitle: "精简详情标题",
          confidence: 88,
          candidates: [
            {
              text: "精简详情标题",
              source: "pdfLayout",
              pageIndex: 0,
              score: 88,
              categoryScores: {
                layout: 30,
                position: 20,
                keyword: 15,
                textQuality: 20,
                penalty: 0,
              },
              ruleDetails: [],
            },
          ],
        }),
      });
      emitBatchEvent?.({
        type: "FileExtracted",
        batchId: "batch-1",
        fileJobId: "file-details",
        extractMethod: "pdfNativeLiteparse",
      });
      emitBatchEvent?.({
        type: "FileProgress",
        batchId: "batch-1",
        fileJobId: "file-details",
        stage: "score",
        progress: 0.62,
      });
      emitBatchEvent?.({
        type: "FileOutputCreated",
        batchId: "batch-1",
        fileJobId: "file-details",
        outputPath: "/input/Rustitler 输出/精简详情标题.pdf",
      });
    });

    const detailPanel = screen.getByRole("heading", { name: "详情" }).closest("aside");

    expect(detailPanel).not.toBeNull();
    expect(within(detailPanel!).getByText("文件地址")).toBeInTheDocument();
    expect(within(detailPanel!).getByText("/input/details.pdf")).toBeInTheDocument();
    expect(within(detailPanel!).getByText("最终标题")).toBeInTheDocument();
    expect(within(detailPanel!).getAllByText("精简详情标题")).toHaveLength(2);
    expect(within(detailPanel!).getByText("疑似重复")).toBeInTheDocument();
    expect(within(detailPanel!).getByText("可能已处理过，请核对历史输出。")).toBeInTheDocument();
    expect(within(detailPanel!).getByRole("heading", { name: "候选标题" })).toBeInTheDocument();
    expect(
      within(within(detailPanel!).getByRole("list", { name: "候选标题列表" })).getByRole("button", {
        name: "精简详情标题",
      }),
    ).toBeInTheDocument();
    expect(within(detailPanel!).queryByText(/batch-old/)).not.toBeInTheDocument();
    expect(within(detailPanel!).queryByText(/file-old/)).not.toBeInTheDocument();
    expect(within(detailPanel!).queryByText(/旧文件\.pdf/)).not.toBeInTheDocument();
    expect(within(detailPanel!).queryByText("置信度")).not.toBeInTheDocument();
    expect(within(detailPanel!).queryByText("88%")).not.toBeInTheDocument();
    expect(within(detailPanel!).queryByText("输出路径")).not.toBeInTheDocument();
    expect(within(detailPanel!).queryByText("/input/Rustitler 输出/精简详情标题.pdf")).not.toBeInTheDocument();
    expect(within(detailPanel!).queryByText("提取方式")).not.toBeInTheDocument();
    expect(within(detailPanel!).queryByText("PDF 原生文本")).not.toBeInTheDocument();
    expect(within(detailPanel!).queryByText("当前阶段")).not.toBeInTheDocument();
    expect(within(detailPanel!).queryByText("评分 / 62%")).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "处理信息" })).not.toBeInTheDocument();
  });
});
