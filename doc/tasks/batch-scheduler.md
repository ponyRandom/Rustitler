# Batch Scheduler Tasks

## 批处理调度模块任务

- [x] BS-01 定义批次运行时状态容器（依赖：CM-13）
- [x] BS-02 实现 `start_batch(paths, settings_snapshot)` 调度入口（依赖：IN-09, SE-13）
- [x] BS-03 实现批次 ID 创建和批次历史记录初始化（依赖：BS-02, HI-09）
- [x] BS-04 实现 `BatchStarted` 和 `FileQueued` 事件生成（依赖：BS-03, CM-12）
- [x] BS-05 实现普通解析任务有界并发池（依赖：BS-04, EX-01）
- [x] BS-06 实现 OCR 任务单独限流（依赖：BS-05, EX-11）
- [x] BS-07 实现 `.doc` 转换任务单独限流（依赖：BS-05, EX-04）
- [x] BS-08 实现文件级阶段进度事件 `FileProgress`（依赖：BS-05）
- [x] BS-09 串联提取成功后的 `FileExtracted` 事件（依赖：EX-01, BS-08）
- [x] BS-10 串联评分成功后的 `FileScored` 事件（依赖：SC-01, BS-09）
- [x] BS-11 实现高置信度文件自动调用输出复制（依赖：SC-15, RN-11, BS-10）
- [x] BS-12 实现低置信度文件进入待处理状态（依赖：SC-15, BS-10）
- [x] BS-13 实现不支持格式和疑似重复处理项跳过或待处理流转（依赖：IN-05, IN-10）
- [x] BS-14 实现单文件错误捕获并继续处理其他文件（依赖：CM-14, BS-05）
- [x] BS-15 实现文件结果、候选和错误写入历史（依赖：HI-10, HI-11, BS-10 至 BS-14）
- [x] BS-16 实现批次级 cancellation token（依赖：BS-05）
- [x] BS-17 实现 `cancel_batch(batch_id)` 取消入口（依赖：BS-16）
- [x] BS-18 实现取消后的 `BatchCancelled` 或完成后的 `BatchCompleted` 事件（依赖：BS-17）
- [x] BS-19 实现 `get_batch_state(batch_id)` 快照读取（依赖：BS-01, CM-13）
- [x] BS-20 实现处理结束后的批次临时目录清理（依赖：EX-15, BS-18）
- [x] BS-21 添加并发限流、取消、单文件失败不影响批次、状态快照和事件顺序测试（依赖：BS-01 至 BS-20）
