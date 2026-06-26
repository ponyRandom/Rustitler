# Dependency Spikes Tasks

## 技术验证任务

- [x] DS-01 验证 `liteparse` 能读取中文 PDF 文本内容（依赖：无）
- [x] DS-02 验证 `liteparse` 能提供页面尺寸、页码和文本块坐标（依赖：DS-01）
- [x] DS-03 验证 `liteparse` 提取失败或空文本时能返回可分类错误（依赖：DS-01）
- [x] DS-04 记录 `liteparse` 输出到 `LayoutBlock` 所需字段映射（依赖：DS-02）
- [x] DS-05 验证 `undoc` 能提取 `.docx` 中文纯文本（依赖：无）
- [x] DS-06 验证 `undoc` 能保留可用段落边界（依赖：DS-05）
- [x] DS-07 验证 `undoc` 对损坏或权限受限 Word 文件的错误形态（依赖：DS-05）
- [x] DS-08 调研 `.doc` 离线转换组件候选并记录许可、包体积和跨平台支持（依赖：无）
- [x] DS-09 验证 `.doc` 转换组件可静默转换到 `undoc` 可读取的中间格式（依赖：DS-08）
- [x] DS-10 验证 `.doc` 转换失败时可返回稳定错误码和错误文本（依赖：DS-09）
- [x] DS-11 验证 Tesseract 简体中文语言包在 macOS 本地可加载（依赖：无）
- [ ] DS-12 验证 Tesseract 简体中文语言包在 Windows 本地可加载（依赖：DS-11）
- [x] DS-13 验证 Tesseract 能返回文本块位置和 OCR 置信度（依赖：DS-11）
- [x] DS-14 验证扫描 PDF 临时栅格化方案可离线生成首页图片（依赖：无）
- [x] DS-15 验证扫描 PDF 前 3 页栅格化的临时文件隔离和清理方式（依赖：DS-14）
- [x] DS-16 验证 SQLite 依赖、应用数据目录路径和基本读写能力（依赖：无）
- [x] DS-17 验证大文件输出副本哈希计算的性能边界（依赖：DS-16）
- [ ] DS-18 形成依赖验证结论文档，列出接口约束和不可用替代方案（依赖：DS-01 至 DS-17）

## 当前验证结论

- DS-01 至 DS-04：`cargo test --features spikes -- spikes::ds01_liteparse -- --nocapture` 使用本机 `cupsfilter` 生成中文 PDF，验证 `liteparse` 可读取中文文本、返回页面尺寸、页码、文本块坐标，并对损坏 PDF 返回 `Pdf(InvalidFormat)`。
- DS-04 字段映射：`TextItem.text -> LayoutBlock.text`；`x/y/width/height -> RawBox`，再按 `page_width/page_height` 归一化为 `NormalizedBox`；`font_size/font_height -> font_size`；`font_weight/font_flags -> bold` 推断来源；`confidence -> ocr_confidence`；`ParsedPage.page_number/page_width/page_height -> ExtractedPage`。
- DS-05 至 DS-07：`textutil` 生成 DOCX，`undoc` 可提取中文纯文本并保留至少 3 个 paragraph-like blocks；损坏 DOCX 返回 `ZipArchive("invalid Zip archive: Could not find EOCD")`，可映射为 `WordExtractFailed`。
- DS-08：`.doc` 转换候选首选 LibreOffice headless (`soffice --headless --convert-to docx --outdir ...`)。官方帮助文档记录命令行参数和 `--convert-to`（https://help.libreoffice.org/latest/he/text/shared/guide/start_parameters.html），系统需求文档标注 Windows 最高约 1.5 GB、macOS 最高约 800 MB 磁盘占用（https://wiki.documentfoundation.org/Documentation/System_Requirements），许可页说明 LibreOffice 以 MPLv2 为主并包含多种第三方开源许可（https://www.libreoffice.org/licenses/）。备选 `antiword` 文本抽取能力较窄且不满足“转为 undoc 可读取中间格式”的任务目标。
- DS-09/DS-10：已安装 LibreOffice 26.2.4.2，`soffice` 位于 `/opt/homebrew/bin/soffice`；`cargo test --features spikes -- spikes::ds09_doc_conversion -- --nocapture` 验证 `textutil` 生成的 legacy `.doc` 可经 `soffice --headless --convert-to docx --outdir ...` 静默转为 DOCX，并可被 `undoc` 读取中文标题；无效 `.doc` 返回稳定 spike 错误 `docConvertFailed`，错误文本以 `LibreOffice did not produce converted DOCX` 开头。
- DS-11：已下载官方 `tessdata_fast` 的 `chi_sim.traineddata` 到本机 tesseract-rs tessdata 目录，`tesseract-rs` 可在 macOS 初始化 `chi_sim`。Tesseract 文档说明 4.00+ 官方 traineddata 分为多个仓库（https://tesseract-ocr.github.io/tessdoc/Data-Files.html），`tessdata_fast` 是官方 fast integer model 仓库（https://github.com/tesseract-ocr/tessdata_fast）。
- DS-12 阻塞：当前环境不是 Windows，无法真实验证 Windows 本地语言包加载。OrbStack 已启动且 Docker 可用，但 `docker info --format 'OSType={{.OSType}} Architecture={{.Architecture}} ServerVersion={{.ServerVersion}}'` 返回 `OSType=linux Architecture=aarch64 ServerVersion=29.4.0`，`orb create --help` 仅列出 Linux 发行版和 `arm64`/`amd64` Linux machines；容器内也无 `/dev/kvm`，不能补成 Windows 本地运行环境。需 Windows runner 或 Windows 机器放置 `chi_sim.traineddata` 到 `TESSDATA_PREFIX` 或 `%APPDATA%\tesseract-rs\tessdata`，再执行 `cargo test --features spikes -- spikes::ds11_tesseract::tesseract_chi_sim_loads -- --nocapture`。
- DS-12 Windows runner 设置方案：在真实 Windows 机器或 Windows CI runner 上安装 Rust 1.83+、CMake、MSVC C++ Build Tools，并从 Developer PowerShell 进入仓库后执行：

  ```powershell
  Set-Location .\src-tauri
  $tessdata = Join-Path $env:APPDATA 'tesseract-rs\tessdata'
  New-Item -ItemType Directory -Force $tessdata | Out-Null
  Invoke-WebRequest `
    -Uri 'https://github.com/tesseract-ocr/tessdata_fast/raw/main/chi_sim.traineddata' `
    -OutFile (Join-Path $tessdata 'chi_sim.traineddata')
  cargo test --features spikes -- spikes::ds11_tesseract::tesseract_chi_sim_loads -- --nocapture
  ```

  通过判定：命令退出码为 0，输出 `DS-11 PASS: chi_sim language data loaded from ...\tesseract-rs\tessdata`。注意 `tesseract-rs` 默认 `build-tesseract` 会在构建期下载/编译 Tesseract 和 Leptonica；这是 spike 期验证路径，后续 `packaging-offline` 仍需把 Windows OCR 资产改为可离线内置或预缓存。
- DS-12 CI 路径：已新增 `.github/workflows/windows-ci.yml`，包含 `windows-spike` 和 `windows-package` 两个 Windows runner job。`windows-spike` 执行上述 DS-12 语言包加载验证；`windows-package` 执行前端依赖安装、前端生产构建、Rust 格式/测试/Clippy 和 Tauri Windows bundle 构建并上传 artifacts。等待 GitHub Actions 跑绿后才能将 DS-12 勾选完成。
- DS-13：`tesseract-rs` 可返回 OCR 文本、词级 bbox 和 confidence；后续 `extract` 应优先使用 `get_iterator()` / `get_current_word()` 或同级 iterator API 生成 OCR `LayoutBlock`。
- DS-14/DS-15：扫描 PDF 栅格化方案确定为复用 `liteparse::screenshot(..., Some(vec![1, 2, 3]))`，避免同时绑定 `pdfium-render` 与 `liteparse` 自带 PDFium 动态库导致符号不匹配；临时文件用批次独立 tempdir，结束后清理。
- DS-16/DS-17：`rusqlite` bundled SQLite 可内存和文件读写；10 MB SHA-256 计算小于 2 秒，可作为撤销输出改动检测的基础。
