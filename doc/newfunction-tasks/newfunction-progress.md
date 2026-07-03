# New Function Progress

## 文件夹分类模块清单

- [ ] `models`：定义 Rust 分类 DTO、系统分类枚举和摘要结构（依赖：无）
- [ ] `settings`：实现分类配置默认值、持久化、清洗和重复校验（依赖：`models`）
- [ ] `classify`：实现后端递归扫描、分类决策、输出目录、复制、冲突处理和摘要（依赖：`models`, `settings`）
- [ ] `commands`：实现并注册 `classify_folder` Tauri IPC 命令（依赖：`classify`）
- [ ] `ipc-types`：定义前端分类 IPC 类型（依赖：`models`）
- [ ] `api-commands`：封装前端 `classifyFolder` 调用（依赖：`commands`, `ipc-types`）
- [ ] `file-dialog`：复用并验证单源文件夹选择行为（依赖：无）
- [ ] `settings-store`：维护前端分类配置草稿和保存错误（依赖：`settings`, `ipc-types`）
- [ ] `app-ui`：实现主界面入口、摘要展示和设置页分类配置区域（依赖：`api-commands`, `file-dialog`, `settings-store`）

## 端到端验收清单

- [ ] 默认规则能将 `请示`、`报告`、`通知`、`标准` 支持格式文件复制到对应目录。
- [ ] 非支持格式统一进入 `其他`，即使文件名包含分类关键词。
- [ ] 多个普通分类命中时进入 `待确认`。
- [ ] 输出目录创建在源文件夹同级，并在同分钟冲突时追加 ` (2)`、` (3)`。
- [ ] 输出不保留源目录层级。
- [ ] 目标分类目录内同名文件冲突时追加 ` (2)`、` (3)`，不覆盖已有文件。
- [ ] 单文件失败记录到摘要并继续处理其他文件。
- [ ] 隐藏文件和系统文件不参与分类且不计入摘要。
- [ ] 分类功能不调用正文提取、评分、重命名、历史、撤销、AI 或云端服务。

## 推荐执行顺序

1. `models`
2. `settings`
3. `classify`
4. `commands`
5. `ipc-types`
6. `api-commands`
7. `file-dialog`
8. `settings-store`
9. `app-ui`
