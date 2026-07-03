# IPC Types Tasks

## 前端 IPC 类型任务

目标文件：`src/types/ipc.ts`

- [ ] NF-IT-01 新增 `ClassificationSettings` 和 `ClassificationCategory` TypeScript 接口（依赖：NF-MO-01）
- [ ] NF-IT-02 新增 `systemKind?: "other" | "needsReview"` 类型约束（依赖：NF-IT-01）
- [ ] NF-IT-03 新增 `ClassificationSummary`、`CategoryCount`、`ClassificationFailure` 接口（依赖：NF-MO-04）
- [ ] NF-IT-04 确认新增分类类型不替换现有 `BatchState`、`BatchSummary`、`FileJobView` 等类型（依赖：NF-IT-03）
- [ ] NF-IT-05 运行类型检查：`rtk npm run build`（依赖：NF-IT-01 至 NF-IT-04）

验收点：

- 前端字段名使用 camelCase。
- 类型结构与 Rust DTO 一一对应。
