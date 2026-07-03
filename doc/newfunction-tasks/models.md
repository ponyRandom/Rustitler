# Models Tasks

## 分类 DTO 模块任务

目标文件：`src-tauri/src/models.rs`

- [x] NF-MO-01 定义 `ClassificationSettings` 和 `ClassificationCategory`，字段使用 `serde(rename_all = "camelCase")` 对齐前端（依赖：无）
- [x] NF-MO-02 定义 `SystemClassificationKind::{Other, NeedsReview}`，序列化值为 `other` 和 `needsReview`（依赖：NF-MO-01）
- [x] NF-MO-03 定义 `ClassifyFolderRequest`，包含 `source_path` 和 `settings`（依赖：NF-MO-01）
- [x] NF-MO-04 定义 `ClassificationSummary`、`CategoryCount`、`ClassificationFailure`，覆盖源路径、输出路径、总数、成功数、失败数、分类计数和失败明细（依赖：NF-MO-01）
- [x] NF-MO-05 为新增 DTO 添加 `Debug`、`Clone`、`Serialize`、`Deserialize`、`PartialEq` 派生，保持测试和 IPC 可用（依赖：NF-MO-01 至 NF-MO-04）
- [x] NF-MO-06 添加模型序列化快照测试，验证 camelCase 字段、`systemKind` 枚举值和摘要结构往返一致（依赖：NF-MO-05）
- [x] NF-MO-07 运行后端模型测试：`rtk cargo test --manifest-path src-tauri/Cargo.toml models --lib`（依赖：NF-MO-06）

验收点：

- DTO 字段名与 `doc/newfunction-high-level-design.md` 保持一致。
- 新类型不替换现有批处理、评分、历史或重命名 DTO。
