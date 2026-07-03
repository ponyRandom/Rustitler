# Settings Store Tasks

## 前端分类设置状态任务

目标文件：`src/stores/settingsStore.ts`、`src/stores/settingsStore.test.ts`

- [x] NF-SS-01 在设置草稿中加入 `classificationSettings`，加载设置时保留后端返回的分类配置（依赖：NF-IT-01, NF-SE-01）
- [x] NF-SS-02 添加新增普通分类的 store 操作，默认新分类包含一个可编辑关键词（依赖：NF-SS-01）
- [x] NF-SS-03 添加编辑分类目录名的 store 操作（依赖：NF-SS-01）
- [x] NF-SS-04 添加编辑分类关键词列表的 store 操作（依赖：NF-SS-01）
- [x] NF-SS-05 添加删除普通分类的 store 操作，删除时同步移除该分类关键词（依赖：NF-SS-01）
- [x] NF-SS-06 保存设置时继续调用现有 `saveSettings`，由后端完成最终清洗和校验（依赖：NF-SS-02 至 NF-SS-05）
- [x] NF-SS-07 保存失败时保留后端校验错误信息，供设置页展示具体原因（依赖：NF-SS-06）
- [x] NF-SS-08 添加 store 测试，覆盖加载、增删改分类、关键词编辑、保存成功和保存失败保留错误（依赖：NF-SS-07）
- [x] NF-SS-09 运行 store 测试：`rtk npm test -- src/stores/settingsStore.test.ts`（依赖：NF-SS-08）

验收点：

- 分类配置与标题评分规则分离。
- 第一版不把分类配置接入设置导入/导出。
