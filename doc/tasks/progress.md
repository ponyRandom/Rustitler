# Rustitler MVP Progress

## 总控模块清单

- [x] `dependency-spikes`：验证关键离线依赖可用性（依赖：无）
- [ ] `core-models`：定义跨模块数据结构、错误模型和 IPC DTO 基础（依赖：`dependency-spikes` 的接口结论）
- [ ] `settings`：实现设置持久化、校验、导入导出和快照（依赖：`core-models`）
- [ ] `diagnostics`：实现结构化运行日志、Debug 诊断输出和清理入口（依赖：`core-models`, `settings`）
- [ ] `scoring`：实现候选标题生成、分类评分和置信度决策（依赖：`core-models`, `settings`）
- [ ] `rename`：实现输出目录、副本复制、文件名清洗和冲突处理（依赖：`core-models`）
- [ ] `history`：实现 SQLite 历史、候选明细、撤销记录和重复检测（依赖：`core-models`, `settings`）
- [ ] `ingest`：实现输入扫描、格式识别、队列初始化和文件指纹（依赖：`core-models`, `history` 的重复检测接口）
- [ ] `extract`：实现 PDF、Word、DOC、图片和扫描 PDF 的离线提取（依赖：`core-models`, `dependency-spikes`, `diagnostics`）
- [ ] `batch-scheduler`：实现批次调度、并发限流、取消、事件流和状态快照（依赖：`ingest`, `extract`, `scoring`, `rename`, `history`, `diagnostics`）
- [ ] `commands`：实现 Tauri IPC 命令桥接、参数校验和事件发布（依赖：`batch-scheduler`, `settings`, `history`, `rename`）
- [ ] `ui`：实现主界面、待处理编辑、历史页、设置页和前端状态镜像（依赖：`commands` 的 DTO 和事件协议）
- [ ] `packaging-offline`：实现 Tauri 权限收敛、离线依赖内置、跨平台打包和离线验收（依赖：`dependency-spikes`, `extract`, `commands`）
- [ ] 50 份样本文档验收：验证自动命名准确率、待处理行为、撤销、设置和离线运行（依赖：全部模块）

## 当前状态

- 当前模块：`core-models`。
- `dependency-spikes` 已完成；DS-01 至 DS-18 均有验证结论。
- DS-09/DS-10 已在当前 macOS 环境通过 LibreOffice `soffice` 验证。
- DS-12 已在 GitHub Actions Windows runner 验证通过：run `28216523178` 的 `DS-12 Tesseract Chinese data` job 成功执行 `cargo test --release --features spikes -- spikes::ds11_tesseract::tesseract_chi_sim_loads -- --nocapture`；同一 run 的 `Windows Tauri package` job 也成功完成 Windows bundle 构建和 artifact 上传。
- 下一个可启动模块：`core-models`。
