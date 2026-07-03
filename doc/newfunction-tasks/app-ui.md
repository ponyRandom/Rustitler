# App UI Tasks

## 主界面与设置页任务

目标文件：`src/App.tsx`、`src/App.css`、`src/App.test.tsx`

- [x] NF-UI-01 在主界面新增“分类文件夹”入口按钮（依赖：NF-FD-01）
- [x] NF-UI-02 点击入口后调用文件夹选择，并在取消选择时不发起分类命令（依赖：NF-UI-01）
- [x] NF-UI-03 选择源文件夹后读取当前 `classificationSettings` 快照（依赖：NF-SS-01）
- [x] NF-UI-04 调用 `classifyFolder(sourcePath, classificationSettings)` 执行分类（依赖：NF-AC-03, NF-UI-03）
- [x] NF-UI-05 执行期间禁用分类入口或显示处理中状态（依赖：NF-UI-04）
- [x] NF-UI-06 分类完成后展示摘要：源路径、输出路径、总文件数、成功复制数、失败数和分类计数（依赖：NF-UI-04）
- [x] NF-UI-07 摘要中展示失败文件路径和原因；无失败时不展示空失败列表（依赖：NF-UI-06）
- [x] NF-UI-08 批次级失败时展示结构化错误信息，并恢复入口可用状态（依赖：NF-UI-05）
- [x] NF-UI-09 在设置页新增“分类配置”区域，展示普通分类列表和关键词列表（依赖：NF-SS-01）
- [x] NF-UI-10 在设置页实现新增、编辑、删除普通分类的控件（依赖：NF-SS-02 至 NF-SS-05）
- [x] NF-UI-11 在设置页保存失败时展示后端返回的具体校验原因（依赖：NF-SS-07）
- [x] NF-UI-12 添加主界面测试，覆盖入口展示、选择文件夹、传入配置快照、处理中状态、摘要展示和失败展示（依赖：NF-UI-08）
- [x] NF-UI-13 添加设置页测试，覆盖分类配置新增、编辑、删除和校验错误展示（依赖：NF-UI-11）
- [x] NF-UI-14 运行 UI 测试：`rtk npm test -- src/App.test.tsx`（依赖：NF-UI-12, NF-UI-13）
- [x] NF-UI-15 运行前端构建：`rtk npm run build`（依赖：NF-UI-14）

验收点：

- 第一版不做执行前分类预览。
- 第一版不支持取消正在执行的分类批次。
- UI 不展示与 AI、正文分类、历史写入或撤销相关的能力。
