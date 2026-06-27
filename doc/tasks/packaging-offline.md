# Packaging Offline Tasks

## 离线与打包任务

- [x] PK-01 收敛 Tauri 权限配置，移除无关网络能力（依赖：CO-01 至 CO-15）
- [x] PK-02 审查 Rust 依赖，确认核心模块不引入网络客户端（依赖：DS-18）
- [x] PK-03 审查前端依赖，确认运行时不依赖联网服务（依赖：UI-01 至 UI-28）
- [ ] PK-04 配置 macOS 打包内置 Tesseract 可执行文件（依赖：DS-11）
- [ ] PK-05 配置 macOS 打包内置简体中文语言包（依赖：PK-04）
- [ ] PK-06 配置 Windows 打包内置 Tesseract 可执行文件（依赖：DS-12）
- [ ] PK-07 配置 Windows 打包内置简体中文语言包（依赖：PK-06）
- [x] PK-08 配置 OCR 语言包运行时路径解析（依赖：PK-05, PK-07, EX-11）
- [x] PK-09 配置 `.doc` 转换组件随包分发（依赖：DS-08 至 DS-10）
- [ ] PK-10 验证 macOS 打包产物可离线处理图片 OCR（依赖：PK-04, PK-05, PK-08）
- [ ] PK-11 验证 Windows 打包产物可离线处理图片 OCR（依赖：PK-06, PK-07, PK-08）
- [ ] PK-12 验证 macOS 打包产物可离线处理 `.doc` 转换（依赖：PK-09）
- [ ] PK-13 验证 Windows 打包产物可离线处理 `.doc` 转换（依赖：PK-09）
- [ ] PK-14 验证 PDF、Word、OCR、设置和历史在断网环境下可运行（依赖：PK-01 至 PK-13）
- [x] PK-15 记录 Tesseract、语言包、`.doc` 转换组件和核心依赖许可清单（依赖：PK-02, PK-09）
- [ ] PK-16 记录安装包体积并标注 Tesseract 和 `.doc` 转换组件占比（依赖：PK-10 至 PK-13）
