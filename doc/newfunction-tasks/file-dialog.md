# File Dialog Tasks

## 文件夹选择任务

目标文件：`src/api/fileDialog.ts`、`src/api/fileDialog.test.ts`

- [ ] NF-FD-01 复用现有 `selectFolder` 能力作为分类源文件夹选择入口（依赖：无）
- [ ] NF-FD-02 确认分类入口只接受单个文件夹路径；如果底层返回多个路径，只取第一个有效路径（依赖：NF-FD-01）
- [ ] NF-FD-03 确认用户取消选择时返回空结果，前端不得调用 `classify_folder`（依赖：NF-FD-01）
- [ ] NF-FD-04 补充或更新文件夹选择测试，覆盖成功选择、取消选择和多路径归一行为（依赖：NF-FD-02, NF-FD-03）
- [ ] NF-FD-05 运行文件夹选择测试：`rtk npm test -- src/api/fileDialog.test.ts`（依赖：NF-FD-04）

验收点：

- 不新增执行前预览。
- 不要求源文件夹名称为 `Rustitler 输出`。
