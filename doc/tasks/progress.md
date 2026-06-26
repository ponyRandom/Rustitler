# Rustitler MVP Progress

## 总控模块清单

- [x] `dependency-spikes`：验证关键离线依赖可用性（依赖：无）
- [x] `core-models`：定义跨模块数据结构、错误模型和 IPC DTO 基础（依赖：`dependency-spikes` 的接口结论）
- [x] `settings`：实现设置持久化、校验、导入导出和快照（依赖：`core-models`）
- [x] `diagnostics`：实现结构化运行日志、Debug 诊断输出和清理入口（依赖：`core-models`, `settings`）
- [x] `scoring`：实现候选标题生成、分类评分和置信度决策（依赖：`core-models`, `settings`）
- [x] `rename`：实现输出目录、副本复制、文件名清洗和冲突处理（依赖：`core-models`）
- [x] `history`：实现 SQLite 历史、候选明细、撤销记录和重复检测（依赖：`core-models`, `settings`）
- [ ] `ingest`：实现输入扫描、格式识别、队列初始化和文件指纹（依赖：`core-models`, `history` 的重复检测接口）
- [ ] `extract`：实现 PDF、Word、DOC、图片和扫描 PDF 的离线提取（依赖：`core-models`, `dependency-spikes`, `diagnostics`）
- [ ] `batch-scheduler`：实现批次调度、并发限流、取消、事件流和状态快照（依赖：`ingest`, `extract`, `scoring`, `rename`, `history`, `diagnostics`）
- [ ] `commands`：实现 Tauri IPC 命令桥接、参数校验和事件发布（依赖：`batch-scheduler`, `settings`, `history`, `rename`）
- [ ] `ui`：实现主界面、待处理编辑、历史页、设置页和前端状态镜像（依赖：`commands` 的 DTO 和事件协议）
- [ ] `packaging-offline`：实现 Tauri 权限收敛、离线依赖内置、跨平台打包和离线验收（依赖：`dependency-spikes`, `extract`, `commands`）
- [ ] 50 份样本文档验收：验证自动命名准确率、待处理行为、撤销、设置和离线运行（依赖：全部模块）

## 当前状态

- 当前模块：`ingest`。
- `dependency-spikes` 已完成；DS-01 至 DS-18 均有验证结论。
- `core-models` 已完成；Rust/TypeScript IPC DTO、错误模型、批次事件、历史 DTO、设置结构和序列化快照测试已落地。
- `settings` 已完成；`settings.json` 路径解析、默认创建、原子写入、完整校验、导入导出、恢复默认和设置快照已落地。
- `diagnostics` 已完成；结构化 JSONL 日志、大小/日期轮转、普通模式脱敏、Debug 提取结果/详细日志保存和清理接口已落地。
- `scoring` 已完成；纯函数评分入口、PDF/图片/Word 候选生成、文本质量/版式/位置/关键词/正则规则、保守降权、阈值决策和规则明细测试已落地。
- `rename` 已完成；源目录旁 `Rustitler 输出` 路径、输出目录创建、文件名清洗、保留扩展名、冲突序号、临时复制后不覆盖落位、手动待处理复用流程和错误转换测试已落地。
- `history` 已完成；`history.sqlite` 路径、SQLite schema 和版本记录、设置快照、批次与文件结果、候选和规则明细、分页列表、批次详情、重复检测、撤销记录和安全撤销测试已落地。
- DS-09/DS-10 已在当前 macOS 环境通过 LibreOffice `soffice` 验证。
- DS-12 已在 GitHub Actions Windows runner 验证通过：run `28216523178` 的 `DS-12 Tesseract Chinese data` job 成功执行 `cargo test --release --features spikes -- spikes::ds11_tesseract::tesseract_chi_sim_loads -- --nocapture`；同一 run 的 `Windows Tauri package` job 也成功完成 Windows bundle 构建和 artifact 上传。
- 下一个可启动模块：`ingest`。
