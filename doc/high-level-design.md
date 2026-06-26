# Rustitler MVP 概要设计文档

版本：0.1  
日期：2026-06-24  
状态：概要设计  
输入需求：`doc/proposal.md`

## 1. 设计目标

Rustitler MVP 是一个完全本地离线运行、无 AI 参与的桌面批量文件重命名工具。应用接收用户拖入的 Word、PDF 和图片扫描件，在本地提取文档标题，将高置信度文件复制到输出目录并按标题重命名，将低置信度和异常文件放入待处理列表。

概要设计遵循以下约束：

- 原文件只读，不修改、不移动、不删除。
- 全流程离线，不调用云端 API，不上传文件，不依赖联网服务，不部署或调用本地大模型。
- Tauri 作为桌面外壳，React + TypeScript 作为前端，Rust 作为核心后端。
- PDF 坐标文本提取使用 `liteparse`。
- Word 纯文本提取使用 `undoc`。
- OCR 使用内置 Tesseract 和简体中文语言包。
- `.doc` 通过内置离线转换组件转为可提取文本的中间格式，但本设计只定义 `DocConverter` 抽象接口，具体组件留到技术验证阶段。
- MVP 不做多行标题合并，不渲染文档页面，不高亮标题区域，不递归扫描子目录。

## 2. 总体架构

Rustitler 采用单个 Tauri 工作区。前端负责用户交互和状态展示，Rust 后端负责批处理状态、文件 I/O、解析、评分、输出、历史和设置。后端是批次、队列、文件状态、历史写入和撤销状态的唯一权威来源；前端只维护当前选中项、筛选条件、展开状态和待处理编辑框草稿等 UI 状态。

```text
React UI
  -> Tauri commands
  -> Rust commands
  -> batch scheduler
  -> ingest
  -> extract
  -> scoring
  -> rename
  -> history / settings / diagnostics
  -> Tauri events
  -> React UI
```

核心调用关系：

```text
UI drag/drop
  -> start_batch(paths, settings_snapshot)
  -> ingest scan and enqueue
  -> duplicate check against history
  -> bounded worker pool
  -> extract ExtractedDocument
  -> scoring ScoringResult
  -> confidence >= threshold ? rename copy : pending
  -> history persist
  -> diagnostics log
  -> event push
```

## 3. 模块划分

### 3.1 前端模块

前端目录建议保持需求文档中的结构：

```text
src/
  components/
  pages/
  stores/
  types/
```

前端模块职责：

- `pages/MainPage`：双栏主界面，左侧文件队列，右侧候选详情。
- `pages/HistoryPage`：查看批次列表、批次文件结果和撤销入口。
- `pages/SettingsPage`：阈值、敏感度、关键词、正则、导入导出和恢复默认。
- `components/FileQueue`：展示文件名、来源路径、类型、状态、识别标题、置信度、输出结果和失败原因。
- `components/CandidatePanel`：展示候选标题、最终标题、分类分数、规则明细、输出路径、失败原因和处理日志摘要。
- `components/PendingEditor`：待处理项文件名主体编辑，不允许用户修改扩展名。
- `stores/batchStore`：订阅后端事件，维护前端显示用状态镜像。
- `stores/settingsStore`：维护设置页表单状态，保存时调用后端设置命令。
- `types/ipc.ts`：与 Rust DTO 对齐的 TypeScript 类型。

前端不直接写历史，不直接决定批次最终状态，不直接进行文件复制和删除。

### 3.2 `commands`

`commands` 是 Tauri IPC 桥接层，负责把前端命令转为 Rust 服务调用，并把后端状态通过事件推送给前端。

主要命令：

- `start_batch(paths, settings_snapshot) -> BatchId`
- `cancel_batch(batch_id) -> CancelResult`
- `get_batch_state(batch_id) -> BatchState`
- `confirm_pending_output(file_job_id, edited_name_stem) -> FileResult`
- `undo_batch(batch_id) -> UndoResult`
- `list_history(query) -> HistoryBatchPage`
- `get_history_batch(batch_id) -> HistoryBatchDetail`
- `load_settings() -> Settings`
- `save_settings(settings) -> Settings`
- `import_settings(path) -> Settings`
- `export_settings(path) -> ExportResult`
- `reset_settings() -> Settings`

`commands` 不包含业务算法，只负责参数校验、错误转换和事件发布。

### 3.3 `ingest`

`ingest` 负责输入扫描和队列初始化。

职责：

- 接收拖入的文件和文件夹路径。
- 文件夹只扫描第一层，不递归。
- 识别支持格式：`.docx`、`.doc`、`.pdf`、`.png`、`.jpg`、`.jpeg`。
- 不支持格式加入队列并标记为跳过。
- 记录来源路径，便于前端展示。
- 计算重复检测所需源文件指纹：规范化路径、文件大小、修改时间。
- 生成 `FileJob`，交给批处理调度器。

MVP 不做持续文件系统监听。所谓文件监控仅指队列状态、进度事件和撤销前输出副本状态检查。

### 3.4 `extract`

`extract` 负责把不同文件类型转换为统一结构化提取结果 `ExtractedDocument`。它不生成候选标题，不做评分，不写历史。

职责：

- PDF：通过 `liteparse` 提取带坐标文本块。
- DOCX：通过 `undoc` 提取纯文本段落。
- DOC：通过抽象 `DocConverter` 转为可提取文本的中间格式，再进入文本提取。
- 图片：通过内置 Tesseract 和简体中文语言包提取 OCR 文本块。
- 扫描 PDF：当原生文本提取失败或为空时，进入 OCR 兜底。

扫描 PDF OCR 允许后端内部临时栅格化页面。这里的“不渲染文档页面”指前端不展示页面预览、不做高亮；后端为了 OCR 可以把指定页临时栅格化为图片。临时文件按批次隔离并清理。

PDF 和 OCR 的页面范围一致：

- 先处理第一页。
- 如果第一页没有达到阈值候选，再扩展到前 3 页。
- 不处理整篇文档。

### 3.5 `scoring`

`scoring` 是标题候选生成和评分核心。它采用纯函数接口：

```rust
fn score_document(
    extracted: ExtractedDocument,
    profile: ScoringProfile,
) -> ScoringResult;
```

约束：

- 不读写文件。
- 不调用 Tauri。
- 不写历史。
- 不持有批次状态。
- 不调用 PDF、Word 或 OCR 解析能力。

评分模块消费 `ExtractedDocument`，输出候选列表、分类分数、规则明细、最终标题和 0-100 置信度。PDF 和图片使用坐标/几何评分，Word 使用纯文本启发式评分。OCR 和 Word 结果整体更保守。

### 3.6 `rename`

`rename` 负责输出副本创建和文件名处理。

职责：

- 按源目录创建固定输出目录 `Rustitler 输出`。
- 原文件只读，输出始终是副本。
- 输出保留原始扩展名。
- 文件名清洗尽量保留原文。
- 自动追加序号处理重名，绝不覆盖已有文件。
- 输出写入采用临时文件加原子替换思路：先复制到临时路径，完成后再移动到最终路径；如果最终路径已存在则重新计算序号。

待处理项手动输出时，前端只提交文件名主体 `edited_name_stem`，后端保留原始扩展名并统一执行清洗、冲突处理和复制。

### 3.7 `history`

`history` 使用本地 SQLite，保存在应用数据目录中。设置仍使用 `settings.json`。

职责：

- 永久保留批次历史。
- 记录文件处理结果、候选详情、分类分数、规则明细和失败原因。
- 支持批次撤销。
- 支持重复处理检测。
- 记录设置快照摘要，保证历史结果可追溯。

重复处理检测只覆盖已自动输出和用户手动确认输出的历史记录。失败、格式不支持、低置信度未输出的记录不作为重复处理依据。

### 3.8 `settings`

`settings` 负责用户配置。

职责：

- 读取和写入应用数据目录中的 `settings.json`。
- 保存自动输出阈值，默认 70。
- 保存版式、位置、关键词、文本质量和 OCR 保守度敏感度。
- 保存高级关键词和正则规则。
- 支持导入、导出和恢复默认。
- 对导入设置执行版本、字段和正则合法性校验。

批次启动时拷贝一份 `settings_snapshot`，该批次全程使用快照。运行中修改设置只影响后续批次。

### 3.9 `diagnostics`

`diagnostics` 负责运行日志和 Debug 诊断数据。

策略：

- 普通模式始终写本地结构化运行日志，按大小或日期轮转。
- 普通模式记录批次级、文件级阶段和错误码。
- Debug 模式增加模块内部细节。
- 默认历史日志不保存全文提取文本、OCR 原始块或 PDF 全量坐标块。
- Debug 模式开启后，才额外保存完整提取结果和详细执行日志，并允许用户手动清理。

## 4. 批处理调度

后端使用有界并发任务池处理文件：

- 普通解析任务可以并发。
- OCR 和 `.doc` 转换单独限流，避免 CPU、内存和外部组件资源争用。
- 批次级取消使用 cancellation token 通知任务尽快停止。
- 单个文件失败不影响其他文件继续处理。
- 每个文件都有独立状态、错误和历史记录。

批次状态由后端维护。前端收到事件后更新本地 UI 镜像，必要时通过 `get_batch_state(batch_id)` 重新拉取快照。

## 5. IPC 协议

### 5.1 命令模式

批处理采用命令加事件模式：

```text
Frontend
  -> start_batch(paths, settings_snapshot)
  <- batch_id
  <- BatchEvent stream
```

取消：

```text
Frontend
  -> cancel_batch(batch_id)
  <- cancel accepted
  <- BatchCancelled or BatchCompleted event
```

状态恢复：

```text
Frontend
  -> get_batch_state(batch_id)
  <- BatchState
```

### 5.2 事件粒度

后端事件采用增量事件为主，同时提供全量快照兜底。

事件类型：

- `BatchStarted`
- `FileQueued`
- `FileProgress`
- `FileExtracted`
- `FileScored`
- `FileOutputCreated`
- `FilePending`
- `FileSkipped`
- `FileFailed`
- `BatchCompleted`
- `BatchCancelled`
- `BatchError`

事件必须带 `batch_id`。文件级事件必须带 `file_job_id`。

### 5.3 TypeScript 事件示例

```ts
export type BatchEvent =
  | { type: "BatchStarted"; batchId: string; createdAt: string; totalFiles: number }
  | { type: "FileQueued"; batchId: string; file: FileJobView }
  | { type: "FileProgress"; batchId: string; fileJobId: string; stage: ProcessingStage; progress?: number }
  | { type: "FileExtracted"; batchId: string; fileJobId: string; extractMethod: ExtractMethod }
  | { type: "FileScored"; batchId: string; fileJobId: string; result: ScoringResultView }
  | { type: "FileOutputCreated"; batchId: string; fileJobId: string; outputPath: string }
  | { type: "FilePending"; batchId: string; fileJobId: string; reason: PendingReason; suggestion?: string }
  | { type: "FileSkipped"; batchId: string; fileJobId: string; reason: string }
  | { type: "FileFailed"; batchId: string; fileJobId: string; error: AppErrorView }
  | { type: "BatchCompleted"; batchId: string; summary: BatchSummary }
  | { type: "BatchCancelled"; batchId: string; summary: BatchSummary }
  | { type: "BatchError"; batchId: string; error: AppErrorView };
```

### 5.4 状态快照示例

```ts
export interface BatchState {
  batchId: string;
  createdAt: string;
  status: BatchStatus;
  settingsSnapshotId: string;
  files: FileJobView[];
  summary: BatchSummary;
}
```

## 6. 核心数据结构

### 6.1 文件与批次

```rust
pub struct BatchId(pub String);
pub struct FileJobId(pub String);

pub struct SourceFingerprint {
    pub normalized_path: String,
    pub size_bytes: u64,
    pub modified_time: String,
}

pub enum FileType {
    Docx,
    Doc,
    Pdf,
    Png,
    Jpg,
    Jpeg,
    Unsupported,
}

pub enum FileStatus {
    Queued,
    Analyzing,
    OutputCreated,
    Pending,
    Skipped,
    Failed,
    Undoable,
    Cancelled,
}
```

### 6.2 提取结果

`ExtractedDocument` 是 `extract` 和 `scoring` 的边界结构。

```rust
pub struct ExtractedDocument {
    pub source_type: FileType,
    pub extract_method: ExtractMethod,
    pub pages: Vec<ExtractedPage>,
    pub paragraphs: Vec<ParagraphBlock>,
    pub diagnostics_ref: Option<String>,
}

pub enum ExtractMethod {
    PdfNativeLiteparse,
    WordUndoc,
    DocConvertedUndoc,
    ImageOcrTesseract,
    PdfOcrFallbackTesseract,
}

pub struct ExtractedPage {
    pub page_index: usize,
    pub width: f32,
    pub height: f32,
    pub unit: SourceUnit,
    pub blocks: Vec<LayoutBlock>,
}

pub enum SourceUnit {
    PdfPoint,
    Pixel,
    Unknown,
}

pub struct LayoutBlock {
    pub text: String,
    pub bbox: NormalizedBox,
    pub raw_bbox: Option<RawBox>,
    pub font_size: Option<f32>,
    pub bold: Option<bool>,
    pub ocr_confidence: Option<f32>,
    pub line_index: Option<usize>,
}

pub struct NormalizedBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

pub struct RawBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

pub struct ParagraphBlock {
    pub text: String,
    pub paragraph_index: usize,
}
```

PDF 与 OCR 文本块统一使用页面归一化坐标，范围为 `0.0-1.0`。同时保留页面原始宽高、原始单位和可选原始坐标，用于 Debug 和后续扩展。

### 6.3 评分结果

```rust
pub struct ScoringProfile {
    pub auto_output_threshold: u8,
    pub layout_sensitivity: f32,
    pub position_sensitivity: f32,
    pub keyword_sensitivity: f32,
    pub text_quality_sensitivity: f32,
    pub ocr_conservatism: f32,
    pub keyword_rules: Vec<KeywordRule>,
    pub regex_rules: Vec<RegexRule>,
}

pub struct ScoringResult {
    pub final_title: Option<String>,
    pub confidence: u8,
    pub candidates: Vec<CandidateTitle>,
    pub decision: ScoreDecision,
}

pub struct CandidateTitle {
    pub text: String,
    pub source: CandidateSource,
    pub page_index: Option<usize>,
    pub paragraph_index: Option<usize>,
    pub score: u8,
    pub category_scores: CategoryScores,
    pub rule_details: Vec<RuleDetail>,
}

pub struct CategoryScores {
    pub layout: i16,
    pub position: i16,
    pub keyword: i16,
    pub text_quality: i16,
    pub penalty: i16,
}
```

分类分数默认展示，逐条规则明细可展开展示。规则分值不在需求阶段固定，设计只保证权重方向：

- PDF 和图片以版式、位置为主。
- 关键词只作为辅助。
- 文本质量用于排除噪声。
- OCR 和 Word 候选整体更保守。

## 7. 标题提取与评分策略

### 7.1 PDF

PDF 提取流程：

```text
liteparse extract first page
  -> normalize layout blocks
  -> score
  -> if no candidate reaches threshold, extract pages 1-3
  -> score again
  -> if native text empty or extraction failed, enter OCR fallback
```

评分因素：

- 字号或视觉尺寸是否明显高于正文。
- 是否加粗。
- 是否位于页面上部。
- 是否水平居中或接近中轴。
- 是否处于合理标题区域。
- 是否命中中文办公关键词。
- 文本长度和标点是否像标题。
- 是否疑似页眉、页脚、页码、编号、日期、密级或落款。

### 7.2 图片扫描件

图片 OCR 流程：

```text
load image
  -> Tesseract OCR with simplified Chinese language data
  -> normalize OCR text blocks to page coordinates
  -> score with OCR conservatism
```

OCR 候选整体更保守。OCR 失败或低置信度进入待处理列表。

### 7.3 扫描 PDF

扫描 PDF 兜底流程：

```text
native PDF text empty or extraction failed
  -> rasterize first page to temporary image
  -> OCR
  -> score
  -> if no candidate reaches threshold, rasterize pages 1-3
  -> OCR and score
```

临时图片存放在批次隔离目录，处理完成后清理。Debug 模式可保留诊断引用。

### 7.4 Word / DOCX / DOC

Word 流程：

```text
docx -> undoc -> paragraphs -> score
doc -> DocConverter -> intermediate format -> text extraction -> paragraphs -> score
```

候选范围默认从前 10 个非空段落中选择。Word 不读取字号、居中、加粗等版式信息，因此评分天然更保守。

`.doc` 转换接口：

```rust
pub trait DocConverter {
    fn convert(&self, input_path: &Path, work_dir: &Path) -> Result<ConvertedDoc, AppError>;
}

pub struct ConvertedDoc {
    pub intermediate_path: PathBuf,
    pub format: ConvertedDocFormat,
}
```

具体转换组件不在概要设计中指定，后续技术验证需要确认许可、包体积、静默调用稳定性和跨平台路径处理。

## 8. 输出与重命名设计

输出目录规则：

- 每个源目录旁创建 `Rustitler 输出`。
- 多个源目录分别创建各自输出目录。
- 不保留源目录相对层级。

文件名清洗规则：

- 替换 Windows 和 macOS 绝对非法字符。
- 保留中文标点、书名号和普通空格。
- 压缩明显异常的连续空白。
- 避免空文件名。
- 避免系统保留名称。
- 始终保留原始扩展名。

重名处理：

```text
关于召开年度工作会议的通知.pdf
关于召开年度工作会议的通知 (2).pdf
关于召开年度工作会议的通知 (3).pdf
```

所有复制和输出错误都转为结构化 `AppError`，单文件失败不影响批次。

## 9. 历史、撤销与重复检测

### 9.1 SQLite 存储

SQLite 表建议：

- `batches`：批次 ID、创建时间、状态、设置快照、统计信息。
- `file_results`：文件任务 ID、批次 ID、源路径、源指纹、文件类型、状态、输出路径、失败原因。
- `candidates`：文件任务 ID、候选标题、分数、页码或段落序号、分类分数。
- `rule_details`：候选 ID、规则名称、规则类别、加减分、说明。
- `undo_records`：输出路径、创建时元数据、创建时哈希、撤销状态。
- `settings_snapshots`：批次使用的设置快照。

历史永久保留，由用户手动清理。输出目录不生成报告文件。

### 9.2 撤销

撤销指删除该批次生成的重命名副本。

撤销前检查：

1. 输出副本不存在：跳过并提示。
2. 输出副本存在：先比对大小和修改时间。
3. 如元数据不一致或需要确认，再计算内容哈希。
4. 如果判定已被用户改动，默认跳过并提示风险。
5. 如果未改动，删除输出副本并更新历史状态。

原文件始终不受影响。

### 9.3 重复检测

重复检测依据：

- 源文件路径。
- 源文件大小。
- 源文件修改时间。

检测范围：

- 已自动输出的历史记录。
- 用户手动确认输出的历史记录。

疑似重复文件自动跳过，进入待处理列表，显示“可能已处理过”。用户确认后才允许重新输出。

## 10. 错误处理

后端定义统一 `AppError` 和 `ErrorCode`。

```rust
pub struct AppError {
    pub code: ErrorCode,
    pub category: ErrorCategory,
    pub user_message: String,
    pub technical_detail: Option<String>,
    pub retryable: bool,
    pub file_path: Option<String>,
    pub stage: Option<ProcessingStage>,
}
```

错误分类：

- `UnsupportedFormat`
- `FileReadFailed`
- `PermissionDenied`
- `PdfExtractFailed`
- `PdfOcrFallbackFailed`
- `OcrEngineFailed`
- `DocConvertFailed`
- `WordExtractFailed`
- `NoTrustedTitle`
- `ConfidenceBelowThreshold`
- `DuplicateSuspected`
- `OutputDirectoryCreateFailed`
- `FileCopyFailed`
- `SanitizedNameEmpty`
- `UndoOutputMissing`
- `UndoOutputModified`
- `Cancelled`
- `Internal`

处理原则：

- 单文件错误只影响该文件。
- 批次继续处理其他文件。
- 错误同时进入队列状态、Tauri event、SQLite 历史和结构化运行日志。
- 前端展示 `user_message`，Debug 模式可查看 `technical_detail`。

## 11. 数据隔离与本地存储

应用数据目录包含：

```text
settings.json
history.sqlite
logs/
debug/
temp/
```

隔离策略：

- 源文件只读。
- 输出副本只写入源目录旁的 `Rustitler 输出`。
- 批处理临时文件使用独立批次目录。
- 正常处理结束后清理临时文件。
- 应用启动时可清理过期临时批次目录。
- 普通历史不保存全文提取文本、OCR 原始块或 PDF 全量坐标块。
- Debug 模式才保存完整提取结果和内部诊断数据。

## 12. 设置设计

设置文件示例：

```json
{
  "version": 1,
  "autoOutputThreshold": 70,
  "layoutSensitivity": 1.0,
  "positionSensitivity": 1.0,
  "keywordSensitivity": 1.0,
  "textQualitySensitivity": 1.0,
  "ocrConservatism": 1.0,
  "keywordRules": ["关于", "通知", "报告", "方案", "制度", "合同", "函"],
  "regexRules": [],
  "debugMode": false
}
```

导入设置时必须校验：

- `version` 是否支持。
- 阈值是否在 `0-100`。
- 敏感度是否在允许范围。
- 正则表达式是否可编译。
- 规则列表是否超出合理长度。

## 13. 离线约束与权限

离线能力作为架构约束实现：

- 核心模块不引入网络客户端。
- 不调用云端 OCR、NLP、文件处理或 AI API。
- Tauri 权限配置不开放无关网络能力。
- 依赖选型必须通过离线审查。
- 打包产物内置 Tesseract 和简体中文语言包。
- `.doc` 转换组件必须离线、内置、跨平台。

MVP 不做运行时全量网络拦截。

## 14. 前端页面设计

### 14.1 主界面

主界面采用双栏：

- 左侧文件队列。
- 右侧候选详情。

文件队列展示：

- 文件名。
- 来源路径。
- 文件类型。
- 当前状态。
- 识别标题。
- 置信度。
- 输出结果。
- 失败原因。

右侧详情展示：

- 当前选中文件路径。
- 候选标题列表。
- 最终识别标题。
- 置信度。
- 分类分数。
- 展开后的逐条规则明细。
- 输出路径。
- 失败原因。
- 处理日志摘要。

### 14.2 待处理列表

待处理项允许编辑文件名主体。确认后：

```text
confirm_pending_output(file_job_id, edited_name_stem)
  -> backend validates and sanitizes
  -> preserve original extension
  -> copy output
  -> update SQLite history
  -> emit FileOutputCreated
```

### 14.3 历史页

历史页通过后端读取 SQLite：

- 查看批次列表。
- 查看批次内每个文件处理结果。
- 查看失败原因。
- 对已输出批次执行撤销。

### 14.4 设置页

设置页包含：

- 自动输出阈值。
- 版式敏感度。
- 位置敏感度。
- 关键词敏感度。
- 文本质量敏感度。
- OCR 保守度。
- 关键词规则。
- 正则规则。
- Debug 模式。
- 设置导入、导出和恢复默认。

## 15. 验收映射

| 需求 | 设计对应 |
| --- | --- |
| 批量拖入文件和文件夹 | `ingest` 第一层扫描 |
| 支持 `.docx`、`.doc`、`.pdf`、图片 | `extract` 按文件类型分派 |
| PDF 使用 `liteparse` | `PdfNativeLiteparse` |
| Word 使用 `undoc` | `WordUndoc` |
| 图片和扫描 PDF OCR | Tesseract OCR 与 PDF 内部栅格化 |
| 高置信度自动输出 | `scoring` + `rename` |
| 低置信度进入待处理 | `FilePending` |
| 原文件不修改 | `rename` 只复制副本 |
| 重名自动追加序号 | `rename` 冲突处理 |
| 历史永久保留 | SQLite `history` |
| 撤销本批次输出 | `undo_records` + 元数据/哈希检查 |
| 设置持久化、导入导出 | `settings.json` |
| 批处理不卡界面 | 后端任务池 + Tauri event |
| 可取消批处理 | cancellation token + `cancel_batch` |
| 单文件失败不影响其他文件 | 结构化 `AppError` 和文件级状态 |
| 全流程离线 | 架构约束和 Tauri 权限约束 |

## 16. 风险与后续验证

需要在实现前或实现早期验证：

- `liteparse` 是否能稳定提供中文 PDF 文本、坐标、页码和页面尺寸。
- `undoc` 是否能稳定保留中文文本和段落边界。
- Tesseract 在 macOS 和 Windows 的打包、语言包路径、许可和体积。
- `.doc` 转换组件的具体选型、许可、体积、静默调用稳定性和错误提示。
- SQLite schema 迁移策略。
- 输出副本哈希在大文件撤销场景下的性能。
- Debug 模式保存诊断数据时的磁盘占用和用户清理入口。
