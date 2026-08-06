# 重试请求标记 retryOf（ticket 01）

**Type**: task
**Status**: pending（等待 responses-provider-passthrough 项目收尾——proxy.rs 流式日志结构稳定后基于新结构实现）

## 问题

8/3-8/6 网关统计比 pi 会话偏高（+1.5M/+9.5M/+17.1M/+4.2M），根因是 pi 客户端重试请求被网关逐条记账（未匹配请求 43/248/416/188 个；8/3、8/5 未匹配 token 与总差异分毫不差）。重试特征：同 conversationId 相邻请求、请求体上下文指纹一致、间隔 < 60s、cached 占比 99%。调研确认 pi 客户端不携带请求级幂等标识（无 `x-request-id`；conversationId 来自 `x-conversation-id`/`x-opencode-session` 头或 body `conversation_id`）。

## 方案

- 日志写入路径（流式与非流式）维护同 conversationId 最近一次成功请求的上下文指纹与时间戳；
- 新成功请求满足：同 conversationId、指纹一致、间隔 < 60s → 日志追加 `retryOf` 字段（引用前一条记录的时间戳）；
- 指纹 = 请求体 `messages` 部分序列化哈希（退化为 `promptTokens` 数值一致）；
- 不删除原记录，`retryOf` 可空，旧行不受影响。

## 完成标准

- [ ] 单测：同会话同指纹间隔 < 60s → `retryOf` 非空；间隔超限或指纹不同 → 不标记
- [ ] 日志行含 `retryOf` 字段且向后兼容
- [ ] cargo test 全绿

## Comments
