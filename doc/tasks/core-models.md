# Core Models Tasks

## 跨模块模型任务

- [ ] CM-01 定义 `BatchId`、`FileJobId`、`SourceFingerprint` 基础类型（依赖：无）
- [ ] CM-02 定义 `FileType` 和扩展名映射规则（依赖：CM-01）
- [ ] CM-03 定义 `FileStatus`、`ProcessingStage` 和批次状态枚举（依赖：CM-01）
- [ ] CM-04 定义 `FileJob`、`FileJobView` 和批次摘要结构（依赖：CM-01 至 CM-03）
- [ ] CM-05 定义 `ExtractedDocument`、`ExtractedPage`、`LayoutBlock` 和坐标结构（依赖：DS-04）
- [ ] CM-06 定义 `ParagraphBlock` 和 Word 文本提取边界结构（依赖：DS-06）
- [ ] CM-07 定义 `ExtractMethod` 枚举并覆盖 PDF、Word、DOC、图片和扫描 PDF（依赖：CM-05, CM-06）
- [ ] CM-08 定义 `ScoringProfile`、关键词规则和正则规则结构（依赖：CM-02）
- [ ] CM-09 定义 `ScoringResult`、`CandidateTitle`、`CategoryScores`、`RuleDetail` 和 `ScoreDecision`（依赖：CM-08）
- [ ] CM-10 定义 `Settings`、默认设置和设置版本字段（依赖：CM-08）
- [ ] CM-11 定义 `HistoryBatchPage`、`HistoryBatchDetail`、历史文件结果和撤销结果结构（依赖：CM-04, CM-09）
- [ ] CM-12 定义 `BatchEvent` 全量事件枚举（依赖：CM-04, CM-09, CM-11）
- [ ] CM-13 定义 `BatchState` 快照结构（依赖：CM-04, CM-12）
- [ ] CM-14 定义 `AppError`、`ErrorCode`、`ErrorCategory` 和可重试标记（依赖：CM-03）
- [ ] CM-15 为跨 IPC 结构添加序列化和反序列化能力（依赖：CM-01 至 CM-14）
- [ ] CM-16 添加模型序列化快照测试，覆盖事件、错误、评分结果和设置结构（依赖：CM-15）

