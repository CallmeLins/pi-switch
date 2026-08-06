# pi-switch 转发日志统计对齐修复（Spec）

**状态**: ready-for-agent（2026-08-07，to-spec 综合：请求级对账调查结论 + 用户确认测试接缝）

**实施状态（2026-08-07 更新）**: 方案 1（SSE usage 提取修复）已由并行项目 responses-provider-passthrough 的 commit `5356120`（native Responses streaming passthrough）实现——`SseUsageParser` 已支持 `response.completed` 事件嵌套 usage 提取并接入请求日志，含单测。本 spec 剩余实现项为方案 2（`retryOf` 重试标记，落点在 proxy.rs 需等该并行项目收尾后基于新流式日志结构实现）、方案 3（stats 排除选项，stats.rs 无冲突可并行）、方案 4（字段语义文档）。

## Problem Statement

用户拿 pi-switch 网关统计与 token-analyzer（pi 会话）对比，8/3-8/7 每天都有差异且方向不一致，无法信任任一数字。2026-08-07 请求级对账（fork 去重 + 消息级 + CST 按天 + ±10s 时间戳最近邻匹配）确认：

1. **8/3-8/6 网关偏高**（+1.5M / +9.5M / +17.1M / +4.2M）：pi 客户端重试请求被网关逐条记账。未匹配请求 43/248/416/188 个，8/3、8/5 未匹配 token 与总差异分毫不差。重试请求特征：时间戳与会话消息不匹配、cached 占比 99%（中位数 0.993）、prompt 逐次递增（全量上下文重发）、间隔中位数 3.9s。
2. **8/7 网关偏低**（−30.4M）：openai-responses passthrough 路径（`proxy.rs` L1540）以 `upstream.bytes()` 整体缓冲响应后 `serde_json::from_slice` 解析，SSE 流式响应非合法 JSON → usage 提取失败 → 289 条 `oc-res/gpt-5.6-luna` 请求 `usage:null`（约 36.6M token 未记账）。pi 客户端自行解析 SSE 成功，会话侧有完整 usage。
3. **字段语义陷阱**：网关 `promptTokens` 为「含缓存命中的总输入」（≡ pi 的 `input+cacheRead`），若对账时按 `prompt+cached+completion` 三和相加，缓存被重复计入，虚增约 1.05B。

（8/2 覆盖缺失为网关启用时间差的历史事实，非缺陷，不在本 spec 范围。）

## Solution

1. **SSE usage 提取修复**：passthrough 路径按响应 `content-type` 分流——`text/event-stream` 改为**流式透传**（边收边转、不整体缓冲），旁路喂现有 `SseUsageParser` 提取 usage，流结束时落日志；非流式响应保持现状（`extract_usage` 整体解析）。
2. **重试去重**：调研确认 pi 客户端不携带请求级幂等标识（请求头仅 auth + provider 配置，无 `x-request-id`；conversationId 来自 `x-conversation-id`/`x-opencode-session` 头或 body `conversation_id`）。网关侧实现启发式标记：同 `conversationId` 的相邻成功请求，若请求体上下文指纹一致且时间间隔 < 60s，日志追加 `retryOf` 字段（引用前一条记录）；统计默认含全部（真实消耗口径），提供排除重试的统计选项。
3. **字段语义文档化**：requests.log 字段语义写入网关文档（`promptTokens` = 含缓存总输入，网关总量 = `promptTokens + completionTokens`）；不改字段名，保持向后兼容。

## User Stories

1. 作为用户，我想 gpt-5.6-luna 等 responses 上游模型的 usage 被网关完整记录，以便网关统计不低于 pi 会话统计。
2. 作为用户，我想在网关统计中识别并可选排除 pi 重试请求，以便与 pi 会话口径对齐（8/3-8/6 差异可解释为纯重试）。
3. 作为用户，我想在对账时不会因 `promptTokens` 语义误解而算错总量，以便快速核对两工具数字。
4. 作为用户，我想流式请求不被网关整体缓冲，以便长生成不占内存、不产生转发延迟。
5. 作为用户，我想每天统计差异都能在日志层面定位（重试标记 / usage 缺失），以便信任统计口径。
6. 作为用户，我想网关日志字段语义有权威文档（含字段映射表），以便第三方工具正确消费。
7. 作为用户，我想重试标记的判定有明确参数（间隔阈值、指纹算法），以便理解误判边界。
8. 作为用户，我想现有 190 个 Rust 测试保持全绿，以便修复不破坏已有转发路径。

## Implementation Decisions

- **测试接缝**（用户确认）：复用现有 `SseUsageParser`（push-based，已有 6 个单测），passthrough 流式路径接入该组件；不新增解析器。
- **流式透传改造**：转发链路从 `upstream.bytes()` + `buffered_response` 改为 chunk 流式 copy 到下游；每个 chunk 同时 `parser.push()`；`on_finish` 回调携带 `parser.finish()` 写日志（日志从「转发时写」延迟到「流结束时写」）。
- **重试标记**：指纹 = 请求体 `messages` 部分序列化哈希（或退化为 `promptTokens` 数值一致）；判定窗口 = 同 conversationId 相邻成功请求、指纹一致、间隔 < 60s。标记字段 `retryOf`（前一条记录的时间戳），不删除原记录。
- **统计选项**：网关 stats 增加 `--exclude-retries`（或等价 webui 参数），排除 `retryOf` 非空条目；默认不过滤（真实消耗口径不变）。
- **兼容性**：requests.log 新增字段仅 `retryOf`（可空），旧行不受影响；`promptTokens` 等既有字段名不改。
- **范围边界**：不修改 pi 客户端（幂等键方案留待 pi 侧支持后迭代）；不修改 token-analyzer（其统计无缺陷，消费 `retryOf` 属后续 ticket）。

## Testing Decisions

- 好的测试标准：只测外部行为——「流式响应转发给客户端后，日志里 usage 字段非 null 且数值正确」；不测解析器内部状态机细节。
- **usage.rs 单测**：`SseUsageParser` 补 Responses API 事件流用例（`response.completed` 嵌套 usage 提取成功；无 usage 事件流 `finish()` 返回 None）。先例：usage.rs `mod tests` 现有 6 个 parser 测试。
- **proxy.rs 集成测试**：mock 上游返回 SSE 流式响应，断言日志 `promptTokens/completionTokens` 非 null 且与 mock 一致、客户端收到完整流字节。先例：`native_responses_records_usage_and_conversation_in_log`。
- **重试标记单测**：同 conversationId + 同指纹 + 间隔 < 60s → `retryOf` 非空；间隔超限或指纹不同 → 不标记。
- 回归：`cargo test --lib` 全绿（现有 190 通过）。

## Out of Scope

- pi 客户端（pi-coding-agent）改动与请求级幂等键方案
- token-analyzer 侧消费 `retryOf` / webui 展示重试标记
- 8/2 网关启用前的覆盖缺失（历史事实）
- 网关内部重试（`ok:false, retry:true` 行）——已有失败标记，不影响 ok 统计
- 请求级去重的历史数据回填（`retryOf` 只对修复后新记录生效）

## Further Notes

- **对账正确口径**：网关总量 = `promptTokens + completionTokens`；`promptTokens` ≡ pi `input + cacheRead`、`completionTokens` ≡ pi `output`。
- 修复后预期：8/7 类差异（usage 缺失）归零；8/3-8/6 类差异可通过 `--exclude-retries` 归零（8/3、8/5 已证明差异 100% 由未匹配请求构成）。
- 8/7 会话文件仍在增长，当日数字会随会话写入变动。
- 现有 passthrough 路径的 `buffered_response` 有内存与延迟隐患，流式透传改造一并消除。
