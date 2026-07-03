# Classify Tasks

## 后端核心分类模块任务

目标文件：`src-tauri/src/classify.rs`、`src-tauri/src/lib.rs`

- [x] NF-CL-01 新建 `src-tauri/src/classify.rs`，并在 `lib.rs` 中注册模块（依赖：NF-MO-01）
- [x] NF-CL-02 定义 `ClassificationDecision::{Category, Other, NeedsReview}` 内部决策类型（依赖：NF-CL-01）
- [x] NF-CL-03 实现支持扩展名判断，仅 `.docx`、`.doc`、`.pdf`、`.png`、`.jpg`、`.jpeg` 参与关键词分类（依赖：NF-CL-02）
- [x] NF-CL-04 实现文件名主体提取，分类时忽略扩展名并保留原文件名用于复制（依赖：NF-CL-03）
- [x] NF-CL-05 实现分类决策纯函数：非支持格式进 `其他`，0 命中进 `其他`，1 命中进对应分类，多命中进 `待确认`（依赖：NF-CL-04）
- [x] NF-CL-06 添加分类决策测试，覆盖支持格式、非支持格式、未命中、多命中和大小写不敏感匹配（依赖：NF-CL-05）
- [x] NF-CL-07 实现源文件夹校验：不存在、不是文件夹、不可读均作为批次级失败返回（依赖：NF-CL-01）
- [x] NF-CL-08 实现递归扫描普通文件，跳过隐藏文件和系统文件，且跳过项不计入摘要（依赖：NF-CL-07）
- [x] NF-CL-09 实现批次输出目录命名：`Rustitler 分类输出 YYYY-MM-DD HHmm`（依赖：NF-CL-07）
- [x] NF-CL-10 实现输出目录冲突序号：同分钟已有目录时追加 ` (2)`、` (3)`，不得覆盖（依赖：NF-CL-09）
- [x] NF-CL-11 实现运行时分类目录集合，确保普通分类以及必需的 `其他`、`待确认` 可用（依赖：NF-SE-09）
- [x] NF-CL-12 实现目标分类目录创建，创建失败记录为单文件失败并继续处理后续文件（依赖：NF-CL-11）
- [x] NF-CL-13 实现目标文件名冲突序号：同名文件追加 ` (2)`、` (3)`，保留原扩展名大小写（依赖：NF-CL-12）
- [x] NF-CL-14 实现文件复制，不保留源目录层级，只复制到分类目录第一层（依赖：NF-CL-13）
- [x] NF-CL-15 实现文件级失败记录，复制、读取、目标路径生成失败都进入 `ClassificationSummary.failures` 并继续批次（依赖：NF-CL-14）
- [x] NF-CL-16 实现 `ClassificationSummary` 汇总，包含总文件数、成功数、失败数、分类计数和失败明细（依赖：NF-CL-15）
- [x] NF-CL-17 添加后端集成测试，使用需求文档验收样例验证 6 个文件的输出结构和摘要（依赖：NF-CL-16）
- [x] NF-CL-18 添加隔离测试，确认分类模块不调用 `extract`、`scoring`、`rename`、`history` 或撤销逻辑（依赖：NF-CL-16）
- [x] NF-CL-19 运行分类模块测试：`rtk cargo test --manifest-path src-tauri/Cargo.toml classify --lib`（依赖：NF-CL-17, NF-CL-18）

验收点：

- 源文件只读，不修改、不移动、不删除。
- 分类判断只依赖文件名主体、扩展名和本地分类配置。
- 第一版不读取正文、不调用 AI、不调用云端或联网服务。
