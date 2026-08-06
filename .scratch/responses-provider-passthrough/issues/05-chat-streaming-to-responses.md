# 05 — Chat Completions Streaming 转 Responses

**What to build:** Chat Completions provider 可以完整处理 Responses streaming 请求，客户端收到 Responses SSE events，包括文本增量、function tool call、reasoning 和最终 usage。

**Blocked by:** 03 — Responses 非流式转换与工具调用

**Status:** resolved

- [x] 支持响应创建、output item、content part、文本 delta、文本完成和 response completed events
- [x] 支持 function tool call 的 arguments delta/done events
- [x] 支持多个并行 tool calls
- [x] `call_id`、函数名和完整 arguments 保持一致
- [x] reasoning 不重复计入 output token
- [x] 最终 usage 映射到 Responses 结构
- [x] upstream 无 usage 时保持 unknown/zero 兼容语义
- [x] 无法映射的结构返回结构化错误，不伪造成功完成
- [x] 转换型 streaming 有端到端测试

## 实施总结

- 提交：`639dab0` — `feat(proxy): Chat Completions streaming to Responses conversion`
- 实现的 seams：S1 `ChatSseToResponses` 状态机（延迟打开 response/message，文本与并行 tool_calls 事件序列，usage 映射，`failed_event`）；S2 流式转换端到端（2 个回归测试 + 2 个主测试）；S3 handler streaming 路由（native 透传优先，convert 候选走转换，`forward_with_failover` 加 `log_stream` 标志防双写日志）
- 验收标准：以上 9 项均已验证并标记为 `- [x]`
- 测试结果：Rust `cargo test` 198 passed（新增 3 状态机单测 + 4 端到端）；WebUI Vitest 94 passed
- typecheck：`cargo check` 通过；新增代码 rustfmt 通过；clippy 无新增 warning
- 文档对齐：无需更新 README（网关特性行已含 Responses ↔ Chat Completions 转换与 SSE streaming）
- 遗留 / 后续建议：① Standards review 发现并修复：message output_index 硬编码 0 的潜在缺陷（tool_calls 先于 text 时 index 错误）、候选筛选三处重复（抽 `filter_profiles`）、item JSON 双份构造（抽 `message_item`/`tool_call_item`）；② 未修（记后续）：`frame_data`/`drain_frames` 与 `SseUsageParser` 的帧解析重复（建议共享逐帧迭代器）、`log_stream` flag argument（可考虑拆函数）、`chat_usage_to_responses_usage` 与 `extract_usage` 归一化重复；③ 混合 native+convert failover 属 issue 06 范围
