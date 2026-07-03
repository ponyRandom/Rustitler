# Rustitler 文件夹分类功能概要设计文档

版本：0.1  
日期：2026-07-03  
状态：概要设计  
输入需求：`doc/newfunction.md`

## 1. 设计目标

文件夹分类功能是 Rustitler 中独立于标题识别、评分和重命名链路的“文件夹分类复制”工具。用户从主界面选择一个源文件夹后，应用递归扫描该文件夹及其子目录中的普通文件，依据文件名主体和本地分类配置决定目标分类，将文件复制到源文件夹同级的新建分类输出目录，并在完成后返回本次分类摘要。

概要设计遵循以下约束：

- 原文件只读，不修改、不移动、不删除。
- 分类判断只依赖文件名主体、文件扩展名和本地分类配置。
- 第一版完全离线运行，不读取正文，不调用 AI、云端服务或联网分类服务。
- 不复用现有标题提取、评分、重命名、历史写入和撤销输出流程。
- 输出目录每次按批次新建，不复用历史分类输出目录。
- 单文件失败只记录到本次摘要，不中断其他文件处理。

## 2. 总体架构

新增功能采用“前端触发 + Tauri IPC + Rust 独立分类模块 + 设置持久化”的架构。前端负责入口、文件夹选择、配置编辑和摘要展示；Rust 后端负责源目录校验、递归扫描、分类决策、输出目录创建、文件复制、冲突处理和摘要汇总。

```text
React UI
  -> fileDialog.selectFolder
  -> settingsStore 当前分类配置快照
  -> api/commands.classifyFolder
  -> Tauri command classify_folder
  -> classify.rs
       -> settings 分类配置校验/清洗
       -> models 分类 DTO
       -> errors 结构化错误
       -> std::fs 文件扫描与复制
  -> ClassificationSummary
  -> React UI 摘要展示
```

与现有标题识别链路的关系：

```text
文件夹分类链路
  -> classify.rs
  -> 文件复制与摘要

标题识别链路
  -> ingest
  -> extract
  -> scoring
  -> rename
  -> history

两条链路共享 settings / models / errors / commands 的基础设施，
但分类链路不调用 extract、scoring、rename、history 或撤销逻辑。
```

## 3. 模块划分

### 3.1 `src-tauri/src/classify.rs`

`classify` 是后端核心业务模块，负责完成一次分类批次。

职责：

- 校验源路径存在、是文件夹且可读取。
- 递归扫描源文件夹及所有子目录。
- 跳过隐藏文件和系统文件，并且不把它们计入摘要。
- 识别普通文件的扩展名是否属于支持格式。
- 根据分类配置和文件名主体生成分类决策。
- 在源文件夹同级创建 `Rustitler 分类输出 YYYY-MM-DD HHmm` 输出目录。
- 当输出目录同名时追加 `(2)`、`(3)` 等序号，避免覆盖。
- 创建分类子目录，包括普通分类以及运行时必需的 `其他`、`待确认`。
- 复制文件到目标分类目录，不保留源目录层级。
- 当目标分类目录内存在同名文件时追加 `(2)`、`(3)` 等序号，保留原扩展名大小写。
- 记录文件级失败并继续处理后续文件。
- 汇总 `ClassificationSummary`。

`classify` 不负责 UI 状态、不保存历史、不撤销输出、不读取文档正文、不执行标题提取或评分。

### 3.2 `src-tauri/src/commands.rs`

`commands` 继续作为 Tauri IPC 桥接层。

新增命令：

```text
classify_folder(source_path, classification_settings) -> ClassificationSummary
```

职责：

- 接收前端传入的源文件夹路径和分类配置快照。
- 调用 `classify` 执行分类。
- 将批次级错误转换为现有结构化错误视图。
- 返回一次性摘要。

第一版不增加分类进度事件，也不把分类批次写入现有历史页。

### 3.3 `src-tauri/src/models.rs`

`models` 新增分类功能的 Rust DTO，并保持 `serde(rename_all = "camelCase")` 与前端类型一致。

核心结构：

```rust
pub struct ClassificationSettings {
    pub categories: Vec<ClassificationCategory>,
}

pub struct ClassificationCategory {
    pub name: String,
    pub keywords: Vec<String>,
    pub system_kind: Option<SystemClassificationKind>,
}

pub enum SystemClassificationKind {
    Other,
    NeedsReview,
}

pub struct ClassifyFolderRequest {
    pub source_path: String,
    pub settings: ClassificationSettings,
}

pub struct ClassificationSummary {
    pub source_path: String,
    pub output_path: String,
    pub total_files: usize,
    pub copied_files: usize,
    pub failed_files: usize,
    pub category_counts: Vec<CategoryCount>,
    pub failures: Vec<ClassificationFailure>,
}

pub struct CategoryCount {
    pub category: String,
    pub count: usize,
}

pub struct ClassificationFailure {
    pub source_path: String,
    pub reason: String,
}
```

`ClassificationSettings` 表示分类运行时使用的配置快照，不参与现有标题评分配置。普通分类由用户维护；`其他` 和 `待确认` 是系统保底分类，运行时需要时自动存在。

### 3.4 `src-tauri/src/settings.rs`

`settings` 继续负责本机设置读写和校验。

新增职责：

- 在 `Settings` 中持久化分类配置列表。
- 提供分类配置默认值：`请示`、`报告`、`通知`、`标准`。
- 保存前清洗分类目录名和关键词。
- 保存前拒绝清洗后的空名称、空关键词、重复分类名和重复关键词。
- 保证同一个关键词不能属于多个普通分类。

分类配置第一版只保存在本机 `settings.json` 中，不纳入现有设置导入/导出扩展能力。

### 3.5 `src/types/ipc.ts`

前端 IPC 类型新增与 Rust DTO 对齐的类型：

```ts
export interface ClassificationSettings {
  categories: ClassificationCategory[];
}

export interface ClassificationCategory {
  name: string;
  keywords: string[];
  systemKind?: "other" | "needsReview";
}

export interface ClassificationSummary {
  sourcePath: string;
  outputPath: string;
  totalFiles: number;
  copiedFiles: number;
  failedFiles: number;
  categoryCounts: CategoryCount[];
  failures: ClassificationFailure[];
}

export interface CategoryCount {
  category: string;
  count: number;
}

export interface ClassificationFailure {
  sourcePath: string;
  reason: string;
}
```

这些类型只服务文件夹分类功能，不替代现有 `BatchState`、`BatchSummary`、`FileJobView` 等标题识别批处理类型。

### 3.6 `src/api/commands.ts`

前端命令封装新增：

```ts
export const classifyFolder = (
  sourcePath: string,
  classificationSettings: ClassificationSettings,
): Promise<ClassificationSummary> =>
  invoke<ClassificationSummary>("classify_folder", { sourcePath, classificationSettings });
```

该函数由主界面“分类文件夹”入口调用。

### 3.7 `src/api/fileDialog.ts`

现有 `selectFolder` 可继续承担文件夹选择职责。分类入口只接受单个源文件夹路径；如果底层文件夹选择返回空列表，前端不发起分类。

### 3.8 `src/stores/settingsStore.ts`

`settingsStore` 继续维护设置页草稿。

新增职责：

- 在设置草稿中维护 `classificationSettings`。
- 支持新增、编辑、删除普通分类。
- 支持编辑分类目录名和关键词。
- 保存时调用现有 `saveSettings`，由后端完成最终清洗与校验。
- 保存失败时展示后端返回的具体原因。

### 3.9 `src/App.tsx`

主界面新增“分类文件夹”入口，并在设置页新增“分类配置”区域。

主界面职责：

- 显示“分类文件夹”按钮。
- 点击后打开文件夹选择对话框。
- 读取当前分类配置快照。
- 调用 `classifyFolder`。
- 执行期间禁用入口或显示处理中状态。
- 完成后展示本次摘要。
- 批次级失败时展示结构化错误信息。

设置页职责：

- 展示分类列表。
- 新增、编辑、删除分类。
- 编辑分类关键词。
- 保存失败时展示校验错误。

## 4. 核心流程

### 4.1 分类执行流程

```text
用户点击“分类文件夹”
  -> 前端选择源文件夹
  -> 前端读取分类配置快照
  -> 调用 classify_folder
  -> 后端校验源文件夹
  -> 后端递归扫描普通文件
  -> 跳过隐藏文件和系统文件
  -> 创建批次输出目录
  -> 对每个文件执行分类决策
  -> 创建目标分类目录
  -> 计算无冲突目标文件名
  -> 复制文件
  -> 汇总成功、失败和分类计数
  -> 返回 ClassificationSummary
  -> 前端展示摘要
```

### 4.2 分类决策流程

```text
输入：文件路径 + ClassificationSettings
  -> 取文件扩展名
  -> 非支持格式：Other
  -> 支持格式：取文件名主体
  -> 对普通分类关键词做不区分大小写匹配
  -> 命中 0 个分类：Other
  -> 命中 1 个分类：该分类
  -> 命中 2 个及以上分类：NeedsReview
```

非支持格式不参与关键词匹配，即使文件名包含分类关键词，也进入 `其他`。

### 4.3 输出目录与文件冲突流程

输出目录创建在源文件夹同级：

```text
Rustitler 分类输出 YYYY-MM-DD HHmm
Rustitler 分类输出 YYYY-MM-DD HHmm (2)
Rustitler 分类输出 YYYY-MM-DD HHmm (3)
```

目标文件复制到分类目录第一层：

```text
Rustitler 分类输出 2026-07-03 1530/
  通知/
    会议通知.pdf
    会议通知 (2).pdf
```

分类输出不保留源目录层级。

## 5. 数据边界

### 5.1 分类设置

分类设置是本机持久化配置的一部分，但和标题评分设置分离。标题评分的 `keywordRules`、`regexRules` 不参与文件夹分类；文件夹分类只使用 `classificationSettings.categories`。

普通分类包含：

- 分类目录名。
- 一个或多个关键词。

系统分类包含：

- `其他`：未命中任何普通分类，或文件格式不支持。
- `待确认`：文件名命中多个普通分类。

系统分类允许在设置界面中不显示或被用户删除，但分类运行时需要时必须自动可用。

### 5.2 分类摘要

`ClassificationSummary` 是前后端之间的完成结果边界。

摘要至少包含：

- 源文件夹路径。
- 输出文件夹路径。
- 参与分类的总文件数。
- 成功复制文件数。
- 失败文件数。
- 各分类数量。
- 单文件失败路径和原因。

隐藏文件和系统文件不进入摘要。

## 6. 错误处理

### 6.1 批次级失败

批次级失败直接返回错误，不创建分类输出目录。

批次级失败包括：

- 源路径不存在。
- 源路径不是文件夹。
- 源文件夹无读取权限。
- 输出目录创建失败。
- 分类配置无效且无法兜底。

### 6.2 文件级失败

文件级失败记录到 `ClassificationSummary.failures`，并继续处理其他文件。

文件级失败包括：

- 文件读取失败。
- 文件复制失败。
- 目标分类目录创建失败。
- 目标文件路径生成失败。

文件级失败增加 `failed_files`，不增加对应分类的成功计数。

### 6.3 结构化错误

批次级错误复用现有 `AppError` / `ErrorCode` / `ErrorCategory` 结构。分类相关错误归入输入、输出、设置或系统类别，不新增与标题提取、评分、OCR、历史、撤销相关的错误语义。

## 7. 离线与权限约束

分类功能不需要网络能力、正文提取能力、OCR 能力或 AI 能力。

实现层必须保持以下隔离：

- 不调用云端 API。
- 不上传用户文件。
- 不调用本地或云端 AI 模型。
- 不读取文档正文内容。
- 不调用 `extract`、`scoring` 或现有标题重命名流程。
- 不写入现有历史页。
- 不注册撤销记录。

## 8. 测试设计

### 8.1 后端测试

后端测试集中覆盖 `classify` 和分类设置校验。

需要覆盖：

- 源目录递归扫描。
- 隐藏文件和系统文件跳过且不计入摘要。
- 支持格式按关键词分类。
- 非支持格式进入 `其他`。
- 未命中关键词进入 `其他`。
- 多分类命中进入 `待确认`。
- 输出目录按时间戳创建。
- 同一分钟输出目录冲突追加 `(2)`、`(3)`。
- 输出不保留源目录层级。
- 目标文件同名冲突追加 `(2)`、`(3)`。
- 单文件复制失败后继续处理其他文件。
- 摘要统计正确。
- 分类配置清洗、空值校验和重复校验。

### 8.2 前端测试

前端测试覆盖入口、命令调用、状态展示和设置编辑。

需要覆盖：

- 主界面展示“分类文件夹”入口。
- 点击入口后调用文件夹选择。
- 用户选择文件夹后调用 `classify_folder`。
- 调用时传入当前分类配置快照。
- 执行期间显示处理中状态或禁用入口。
- 完成后展示源路径、输出路径、总数、成功数、失败数和分类计数。
- 失败时展示错误信息。
- 设置页支持分类配置新增、编辑、删除。
- 设置页保存失败时展示后端校验原因。

## 9. 验收映射

| 需求 | 设计对应 |
| --- | --- |
| 主界面发起“分类文件夹” | `App.tsx` 新增入口 + `selectFolder` |
| 递归扫描源文件夹 | `classify.rs` 递归扫描 |
| 跳过隐藏文件和系统文件 | `classify.rs` 扫描过滤 |
| 按文件名主体匹配关键词 | 分类决策流程 |
| 非支持格式进入 `其他` | 分类决策流程 |
| 多分类命中进入 `待确认` | 分类决策流程 |
| 输出目录创建在源文件夹同级 | 输出目录创建流程 |
| 每次创建新批次目录 | 时间戳目录 + 冲突序号 |
| 不保留源目录层级 | 文件复制到分类目录第一层 |
| 同名文件不覆盖 | 目标文件冲突序号 |
| 单文件失败继续处理 | 文件级失败记录 |
| 完成后展示摘要 | `ClassificationSummary` + 前端摘要区域 |
| 分类配置本机保存 | `settings` + `settingsStore` |
| 不调用标题识别链路 | 模块隔离约束 |
| 完全离线 | 离线与权限约束 |

## 10. 后续扩展边界

以下能力不进入第一版设计的执行范围，只保留为后续扩展方向：

- 执行前预览分类计划。
- 分类批次进度事件。
- 取消正在执行的分类。
- 撤销本次分类复制结果。
- 将分类批次写入历史页面。
- 导入/导出分类配置。
- 手动确认 `待确认` 文件并由软件移动。
- 支持按正文内容或标题识别结果分类。
- 支持正则、优先级、排除词等复杂规则。
- 支持用户选择输出目录。
- 支持保留源目录层级。
