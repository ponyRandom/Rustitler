# Rustitler MVP Progress

## 总控模块清单

- [ ] `dependency-spikes`：验证关键离线依赖可用性（依赖：无）
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

## 当前阻塞

- 当前模块：`dependency-spikes`。
- 阻塞任务：DS-12；因此 DS-18 和父模块暂不能勾选完成。
- DS-09/DS-10 已在当前 macOS 环境通过 LibreOffice `soffice` 验证。
- DS-12 需要 Windows 环境验证 Tesseract 简体中文语言包本地加载；当前 macOS 环境无法真实验证。OrbStack 已启动且 Docker 可用，但 Docker server 是 `OSType=linux Architecture=aarch64`，`orb create` 仅支持 Linux machines，不能提供 Windows 本地运行环境。
- 已补充 DS-12 的真实 Windows runner 设置和验证命令，并新增 `.github/workflows/windows-ci.yml`。本次 Windows CI 日志显示 DS-12 debug profile 会命中 `tesseract-rs 0.2.0` 的 Windows debug 库名不匹配，已改为 release profile；随后日志显示 Windows PowerShell runner 没有 `HOME`，已将测试路径解析改为优先使用 `%APPDATA%`；等待修复后的 GitHub Actions Windows runner 执行 `windows-spike` 通过后才能勾选 DS-12。
