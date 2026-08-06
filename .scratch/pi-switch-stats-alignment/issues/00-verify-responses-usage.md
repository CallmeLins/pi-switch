# 验证 Responses 流式 usage 提取（ticket 00）

**Type**: task
**Status**: ready-for-agent
**Blocked by**: responses-provider-passthrough/issues/04（native streaming）已 resolved（commit 5356120）

## 问题

调查确认 8/6-8/7 的 `oc-res/gpt-5.6-luna` 289 条请求 usage 全 null（约 36.6M token 未记账），根因是 passthrough 路径用整体 JSON 解析 SSE 流失败。并行项目已提交 `5356120` 修复（SseUsageParser 支持 `response.completed` 嵌套 usage + 流式透传 + 日志记录），本 ticket 验证该修复确实解决调查发现的问题。

## 验证步骤

1. 代码审查：确认流式路径在每个 chunk 调用 `parser.push()`，流结束时以 `parser.finish()` 写入日志（`build_log_entry` 的 usage 参数非 None）；非流式路径不受影响。
2. 单测：`cargo test --lib` 全绿（含新增 `sse_parser_extracts_responses_usage_from_completed_event`）。
3. 运行期验证（部署后）：对 openai-responses 上游（oc-res）发起流式请求，断言 requests.log 新行 `promptTokens/completionTokens/cachedTokens` 非 null 且与上游 usage 一致；对照 token-analyzer 会话侧同请求 usage 可对上。
4. 回归确认：8/7 类「网关偏低」差异在新记录上不再出现。

## 完成标准

- [ ] 审查确认流式路径 usage 落日志
- [ ] cargo test 全绿
- [ ] 运行期一条流式 responses 请求日志 usage 非 null

## Comments
