# API Commands Tasks

## 前端命令封装任务

目标文件：`src/api/commands.ts`、`src/api/commands.test.ts`

- [x] NF-AC-01 从 `src/types/ipc.ts` 引入分类设置和分类摘要类型（依赖：NF-IT-03）
- [x] NF-AC-02 新增 `classifyFolder(sourcePath, classificationSettings)` 封装（依赖：NF-AC-01）
- [x] NF-AC-03 调用 Tauri `invoke("classify_folder", { sourcePath, classificationSettings })`，参数名保持与后端命令一致（依赖：NF-CO-01）
- [x] NF-AC-04 添加命令封装测试，验证 invoke 名称、源路径参数和分类配置快照参数（依赖：NF-AC-03）
- [x] NF-AC-05 运行前端命令测试：`rtk npm test -- src/api/commands.test.ts`（依赖：NF-AC-04）

验收点：

- 前端只在用户选择到源文件夹后调用命令。
- 命令封装不保存历史、不处理撤销、不订阅进度事件。
