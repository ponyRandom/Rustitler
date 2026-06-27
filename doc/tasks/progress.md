# Rustitler MVP Progress

## 总控模块清单

- [x] `dependency-spikes`：验证关键离线依赖可用性（依赖：无）
- [x] `core-models`：定义跨模块数据结构、错误模型和 IPC DTO 基础（依赖：`dependency-spikes` 的接口结论）
- [x] `settings`：实现设置持久化、校验、导入导出和快照（依赖：`core-models`）
- [x] `diagnostics`：实现结构化运行日志、Debug 诊断输出和清理入口（依赖：`core-models`, `settings`）
- [x] `scoring`：实现候选标题生成、分类评分和置信度决策（依赖：`core-models`, `settings`）
- [x] `rename`：实现输出目录、副本复制、文件名清洗和冲突处理（依赖：`core-models`）
- [x] `history`：实现 SQLite 历史、候选明细、撤销记录和重复检测（依赖：`core-models`, `settings`）
- [x] `ingest`：实现输入扫描、格式识别、队列初始化和文件指纹（依赖：`core-models`, `history` 的重复检测接口）
- [x] `extract`：实现 PDF、Word、DOC、图片和扫描 PDF 的离线提取（依赖：`core-models`, `dependency-spikes`, `diagnostics`）
- [x] `batch-scheduler`：实现批次调度、并发限流、取消、事件流和状态快照（依赖：`ingest`, `extract`, `scoring`, `rename`, `history`, `diagnostics`）
- [x] `commands`：实现 Tauri IPC 命令桥接、参数校验和事件发布（依赖：`batch-scheduler`, `settings`, `history`, `rename`）
- [x] `ui`：实现主界面、待处理编辑、历史页、设置页和前端状态镜像（依赖：`commands` 的 DTO 和事件协议）
- [ ] `packaging-offline`：实现 Tauri 权限收敛、离线依赖内置、跨平台打包和离线验收（依赖：`dependency-spikes`, `extract`, `commands`）
- [ ] 50 份样本文档验收：验证自动命名准确率、待处理行为、撤销、设置和离线运行（依赖：全部模块）

## 当前状态

- 当前模块：`packaging-offline`。
- `dependency-spikes` 已完成；DS-01 至 DS-18 均有验证结论。
- `core-models` 已完成；Rust/TypeScript IPC DTO、错误模型、批次事件、历史 DTO、设置结构和序列化快照测试已落地。
- `settings` 已完成；`settings.json` 路径解析、默认创建、原子写入、完整校验、导入导出、恢复默认和设置快照已落地。
- `diagnostics` 已完成；结构化 JSONL 日志、大小/日期轮转、普通模式脱敏、Debug 提取结果/详细日志保存和清理接口已落地。
- `scoring` 已完成；纯函数评分入口、PDF/图片/Word 候选生成、文本质量/版式/位置/关键词/正则规则、保守降权、阈值决策和规则明细测试已落地。
- `rename` 已完成；源目录旁 `Rustitler 输出` 路径、输出目录创建、文件名清洗、保留扩展名、冲突序号、临时复制后不覆盖落位、手动待处理复用流程和错误转换测试已落地。
- `history` 已完成；`history.sqlite` 路径、SQLite schema 和版本记录、设置快照、批次与文件结果、候选和规则明细、分页列表、批次详情、重复检测、撤销记录和安全撤销测试已落地。
- `ingest` 已完成；直接文件接收、文件夹第一层扫描、子文件夹不递归跳过、支持格式识别、不支持格式跳过、来源路径记录、规范化路径、大小/修改时间指纹、`FileJob` 初始化、UUID 分配和重复检测标记测试已落地。
- `extract` 已完成；统一提取服务接口、按 `FileType` 分派、DOCX 前 10 个非空段落、DOC 转换抽象和错误映射、PDF 原生文本块坐标归一化、PDF OCR 兜底编排、图片 OCR 编排、批次临时目录清理和 Debug 提取诊断保存测试已落地。`extraction-deps` feature 下已提供 `undoc`、LibreOffice 和 `liteparse` 适配；`extraction-ocr`/`offline-bundle` feature 下已接入 `tesseract-rs` 静态 OCR 运行时、PNG/JPEG 解码和离线 tessdata 路径。
- `batch-scheduler` 已完成；批次运行时状态容器、`start_batch_with_services` 调度入口、批次 ID 和历史初始化、`BatchStarted`/`FileQueued`/`FileProgress`/`FileExtracted`/`FileScored`/`FileOutputCreated`/`FilePending`/`FileSkipped`/`FileFailed`/`BatchCompleted`/`BatchCancelled` 事件、有界 worker pool、OCR 与 `.doc` 独立限流、单文件错误隔离、取消 token、`cancel_batch`、`get_batch_state`、最终历史记录和临时目录清理均已落地。当前调度核心通过 trait 注入提取、输出、历史和事件 sink，后续 `commands` 负责接入 Tauri IPC 与真实事件发布。
- `commands` 已完成；Tauri `AppState` 初始化、`start_batch`/`cancel_batch`/`get_batch_state`/`confirm_pending_output`/`undo_batch`/`list_history`/`get_history_batch`/`load_settings`/`save_settings`/`import_settings`/`export_settings`/`reset_settings` 均已注册到 invoke handler。命令层已实现空路径、分页和手动文件名主体校验，复用 `AppError` 序列化作为前端错误 DTO，通过 `batch-event` 发布批处理事件，并为手动确认输出写入历史、撤销记录和 `FileOutputCreated` 事件。命令测试覆盖参数校验、错误转换、事件发布、历史读取、手动输出、撤销和设置命令；同时修复了历史批次 upsert 误用 `INSERT OR REPLACE` 导致 `file_results` 被级联删除的问题。默认构建下真实提取依赖未启用时会返回明确的提取依赖不可用错误，`offline-bundle` 构建下命令层接入真实 DOC/PDF/OCR 适配器和 Tauri `resource_dir` 资产。
- `ui` 已完成；前端 IPC DTO、Tauri 命令封装、`batch-event` 订阅、拖放启动、批次状态镜像、快照修复、待处理手动确认、取消状态、主界面双栏队列与详情、候选/分类分数/规则明细、历史列表/详情/撤销、设置加载/编辑/保存/导入/导出/恢复默认和 Vitest 覆盖均已落地。
- `packaging-offline` 已部分完成；Tauri capability 已审查为仅使用 core path/event/window/default 权限，Rust 与前端运行时依赖审计已记录，`offline-bundle` feature 已接入 PDF/DOCX/DOC/OCR 运行时适配器，`tesseract-rs` 静态 OCR 运行时与简体中文 `chi_sim.traineddata` 已配置为 macOS/Windows 打包资源，Tessdata 与 LibreOffice 运行时资源路径解析已落地并接入 Tauri `resource_dir`，`resources/tessdata` 和 `resources/libreoffice` 已配置为 bundle resources，许可清单已建立。新增 `.github/workflows/offline-package.yml` 会在 macOS/Windows 构建真实 Tauri 离线包、运行 packaged binary 的 `--offline-smoke-test` 验证 OCR/设置/历史，并上传安装包体积报告；PK-12、PK-13、PK-14 的 `.doc` 与完整 PDF/Word 断网验收仍需真实 LibreOffice 运行时资源和 release 后人工体验。
- DS-09/DS-10 已在当前 macOS 环境通过 LibreOffice `soffice` 验证。
- DS-12 已在 GitHub Actions Windows runner 验证通过：run `28216523178` 的 `DS-12 Tesseract Chinese data` job 成功执行 `cargo test --release --features spikes -- spikes::ds11_tesseract::tesseract_chi_sim_loads -- --nocapture`；同一 run 的 `Windows Tauri package` job 也成功完成 Windows bundle 构建和 artifact 上传。
- 下一个可启动任务：`packaging-offline` 的 PK-12 至 PK-14（真实 macOS/Windows 离线包体 `.doc`、PDF/Word 完整验收），以及最终 50 份样本文档验收。
