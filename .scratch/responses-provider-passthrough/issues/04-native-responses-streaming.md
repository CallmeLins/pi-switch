# 04 — 原生 Responses Streaming 透传

**What to build:** 原生 Responses provider 可以处理 streaming 请求，客户端收到未经转换的 SSE events；连接建立前的故障可以 failover，stream 开始后的故障不会重放请求。

**Blocked by:** 01 — Provider Responses 模式配置与 WebUI 控制

**Status:** resolved

- [x] 原生 Responses provider 的 SSE event 名称、顺序和 payload 保持不变
- [x] `stream: true` 请求 body 原样发送
- [x] upstream usage event 原样保留并可旁路解析
- [x] headers/event 发出前失败时切换下一个候选 provider
- [x] 已发送 streaming 输出后失败时不重放、不切换
- [x] stream failure 被记录为可诊断的失败类型
- [x] passthrough streaming 有端到端测试

## 实施总结

- 提交：`5356120` — `feat(proxy): native Responses streaming passthrough`
- 实现的 seams：S1 `SseUsageParser` 带 type 的 Responses usage 帧解析；S2 streaming 透传端到端（SSE 保真 + usage 旁路 + body 原样）；S3 streaming failover（headers 前切换、流开始后不重放、failure 日志）
- 验收标准：以上 7 项均已验证并标记为 `- [x]`
- 测试结果：Rust `cargo test` 191 passed（新增 3 端到端 + 2 单元）；WebUI Vitest 94 passed
- typecheck：`cargo check` 通过；新增代码 rustfmt 通过；clippy 无新增 warning
- 文档对齐：无需更新 README（gateway 特性行已含 native Responses passthrough）
- 遗留 / 后续建议：① 候选筛选统一走 `is_native_responses_passthrough`（responsesMode auto/passthrough，未知模式被剔除）；② mid-stream 上游错误在传输层不可靠时可能表现为截断流（hyper 行为），此时日志按正常结束记录——`StreamTee` 单测与 `stream_log_entry` 单测保证错误传播路径可诊断；③ Chat→Responses SSE 转换由 ticket 05 继续
