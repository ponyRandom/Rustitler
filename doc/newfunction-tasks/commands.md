# Commands Tasks

## Tauri IPC 分类命令任务

目标文件：`src-tauri/src/commands.rs`、`src-tauri/src/lib.rs`

- [ ] NF-CO-01 新增 `classify_folder(source_path, classification_settings)` Tauri 命令签名（依赖：NF-MO-04）
- [ ] NF-CO-02 在命令中校验 `source_path` 非空，空路径返回结构化输入错误（依赖：NF-CO-01）
- [ ] NF-CO-03 调用 `classify` 模块执行一次分类批次，并传入前端分类配置快照（依赖：NF-CL-16）
- [ ] NF-CO-04 将分类批次级错误转换为现有 `AppError` / `ErrorCode` / `ErrorCategory` 结构（依赖：NF-CO-03）
- [ ] NF-CO-05 注册 `classify_folder` 到 Tauri invoke handler（依赖：NF-CO-01）
- [ ] NF-CO-06 添加 IPC 命令测试，覆盖成功返回摘要、空路径错误和分类模块错误转换（依赖：NF-CO-05）
- [ ] NF-CO-07 运行命令测试：`rtk cargo test --manifest-path src-tauri/Cargo.toml commands --lib`（依赖：NF-CO-06）

验收点：

- 第一版不增加分类进度事件。
- 第一版不把分类批次写入历史页。
- 第一版不注册撤销记录。
