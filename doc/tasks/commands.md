# Commands Tasks

## Tauri IPC 命令任务

- [x] CO-01 注册 `start_batch` 命令并校验路径列表非空（依赖：BS-02）
- [x] CO-02 注册 `cancel_batch` 命令并转换调度器结果（依赖：BS-17）
- [x] CO-03 注册 `get_batch_state` 命令并返回快照 DTO（依赖：BS-19）
- [x] CO-04 注册 `confirm_pending_output` 命令并校验 `edited_name_stem` 只包含文件名主体（依赖：RN-14, HI-10）
- [x] CO-05 实现手动确认输出后的历史更新和 `FileOutputCreated` 事件发布（依赖：CO-04, HI-17）
- [x] CO-06 注册 `undo_batch` 命令并返回 `UndoResult`（依赖：HI-18 至 HI-22）
- [x] CO-07 注册 `list_history` 命令并校验分页查询参数（依赖：HI-13）
- [x] CO-08 注册 `get_history_batch` 命令并返回批次详情（依赖：HI-14）
- [x] CO-09 注册 `load_settings` 命令（依赖：SE-03）
- [x] CO-10 注册 `save_settings` 命令并返回校验后的设置（依赖：SE-04 至 SE-09）
- [x] CO-11 注册 `import_settings` 命令并复用导入校验（依赖：SE-10）
- [x] CO-12 注册 `export_settings` 命令（依赖：SE-11）
- [x] CO-13 注册 `reset_settings` 命令（依赖：SE-12）
- [x] CO-14 实现统一 `AppError` 到前端错误 DTO 的转换（依赖：CM-14）
- [x] CO-15 实现批处理事件发布器并确保事件包含 `batch_id` 和 `file_job_id`（依赖：CM-12, BS-04 至 BS-18）
- [x] CO-16 添加 IPC 参数校验、错误转换、手动输出、撤销和设置命令测试（依赖：CO-01 至 CO-15）
