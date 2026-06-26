# Extract Tasks

## 文档提取模块任务

- [x] EX-01 定义统一提取器接口和按 `FileType` 分派逻辑（依赖：CM-05 至 CM-07）
- [x] EX-02 实现 `.docx` 到 `undoc` 纯文本段落提取（依赖：DS-05, DS-06, EX-01）
- [x] EX-03 实现前 10 个非空 Word 段落收集（依赖：EX-02）
- [x] EX-04 定义 `DocConverter` 抽象接口和 `ConvertedDoc` 结构（依赖：DS-09）
- [x] EX-05 实现 `.doc` 转换后进入文本提取流程（依赖：EX-02, EX-04）
- [x] EX-06 实现 `.doc` 转换失败到 `DocConvertFailed` 错误转换（依赖：EX-05, CM-14）
- [x] EX-07 实现 PDF 首页 `liteparse` 文本块提取（依赖：DS-01 至 DS-04, EX-01）
- [x] EX-08 实现 PDF 页面尺寸、原始坐标和归一化坐标转换（依赖：EX-07）
- [x] EX-09 实现 PDF 文本为空或提取失败时的 OCR 兜底判定（依赖：EX-07, CM-14）
- [x] EX-10 实现 PDF 前 3 页原生文本提取入口（依赖：EX-07）
- [x] EX-11 实现图片 OCR 文本块提取（依赖：DS-11 至 DS-13, EX-01）
- [x] EX-12 实现 OCR 文本块坐标归一化和置信度映射（依赖：EX-11）
- [x] EX-13 实现扫描 PDF 首页临时栅格化并进入 OCR（依赖：DS-14, EX-09, EX-11）
- [x] EX-14 实现扫描 PDF 前 3 页 OCR 兜底入口（依赖：DS-15, EX-13）
- [x] EX-15 实现批次隔离临时目录创建和清理（依赖：EX-13）
- [x] EX-16 实现提取结果 Debug 诊断保存调用（依赖：DG-07, EX-01）
- [x] EX-17 添加 DOCX、DOC 转换失败、PDF 原生文本、图片 OCR、扫描 PDF 兜底和临时目录清理测试（依赖：EX-01 至 EX-16）
