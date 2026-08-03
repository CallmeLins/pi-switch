# 01 — 推理 token 全链路（解析 → 日志 → 聚合 → 导出）

**What to build:** 后端四维度就绪：usage 解析能提取推理 token（Chat Completions / DeepSeek 的 `completion_tokens_details.reasoning_tokens`、Responses 的 `output_tokens_details.reasoning_tokens`，缺失记 0），流式与转换路径都覆盖；请求日志追加 `reasoningTokens` 列且旧行兼容；全局总计、单次对话、by-provider 的 token 统计都含缓存与推理维度（合计仍为输入+输出，推理不重复累加）；CSV/JSON 导出含推理列。验证方式是 `/stats` 响应与导出中出现四维度字段，且 total 不因推理而增加。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] `extract_usage` 与 SSE 流式解析产出 `reasoning_tokens`，三条路径（chat completions / responses / 缺失记 0）有测试覆盖
- [x] 请求日志行含 `reasoningTokens`；旧日志行（无此字段）反序列化正常，聚合不报错
- [x] 全局总计含 cached 与 reasoning 字段，`total = input + output` 不变（推理是输出子集，不重复累加）
- [x] 单次对话统计含 cachedTokens 与 reasoningTokens；by-provider 含 reasoningTokens
- [x] CSV 与 JSON 导出含 reasoningTokens 列，旧数据缺失按 0
- [x] `cargo test --lib` 全绿

## 实施总结
- 提交：`39dc6df` — feat: track reasoning tokens through usage stats pipeline（usage.rs / proxy.rs / stats.rs CSV / tui 测试辅助）
- 注：聚合/字段声明部分（RequestLogEntry.reasoning_tokens、TokenTotals.cached/reasoning、ConversationStats/ProviderStats、aggregate 累加）随外部会话的时间窗口 commit `93158e3` 一并入库
- 实现的 seams：S1 `extract_usage`（chat completions / responses / 缺失记 0 + responses cached）、S2 SSE 流式、S3 `build_log_entry`、S4 `chat_response_to_responses` 转换穿透、S5 aggregate 四维度 + 旧行兼容、S6 CSV/JSON 导出
- 测试结果：全绿，`cargo test --lib` 105 passed
- typecheck：通过
- 遗留 / 后续建议：CSV/JSON 对缺失字段输出空串/null（与既有 cached 列口径一致，聚合侧按 0）；TUI 展示四维度平铺未在本 issue 范围（见 ADR 0003 后续）；`chat_response_to_responses` 与 `extract_usage` 的嵌套 details 探测存在同形重复（判断项，未提取）
