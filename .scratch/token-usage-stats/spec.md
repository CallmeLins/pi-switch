# Token 使用量统计（总量 / 单次对话 / 缓存命中率）

Status: ready-for-agent

## Problem Statement

pi-switch 代理目前只记录请求成败与延迟（`requests.log`），完全不知道 token 消耗。用户想回答三个问题却无从下手：累计烧了多少 token？某次对话用了多少？缓存命中率多高（Anthropic 系与 OpenAI 系都提供缓存，省下的钱直接取决于命中率）？

## Solution

代理在转发流式响应时旁路解析（tee）SSE 帧，把每次请求的输入/输出/命中缓存 token 数与会话标识追加进 `requests.log`；统计聚合（`get_stats`）输出全局 token 总数、缓存命中率、by-provider 明细与按会话聚合的对话列表，展示在 WebUI 与 TUI。全程不缓冲、不打断流式体验。

## User Stories

1. As a pi 用户, I want to see the total tokens consumed across all requests, so that I know my overall spend
2. As a pi 用户, I want to see a specific conversation's token count, so that I can tell which conversations are heavy
3. As a pi 用户, I want to see the cache hit rate as a percentage, so that I can judge whether my setup is getting the caching benefit I paid for
4. As a pi 用户, I want token stats to work for streaming responses, so that I don't have to choose between token-by-token UX and usage tracking
5. As a pi 用户, I want per-provider token totals, so that I can see which provider burns tokens fastest
6. As a pi 用户, I want conversations sorted by most recent activity, so that recent chats are easy to find
7. As a pi 用户, I want requests without a conversation id to still be counted, so that unlabeled usage isn't lost
8. As a pi 用户, I want stats to keep working on logs written before this feature, so that upgrading doesn't break or blank existing history
9. As a pi 用户, I want requests where the upstream never reported usage to be skipped gracefully, so that partial data doesn't poison the aggregates
10. As a pi 用户, I want to see the same totals and cache rate in the TUI, so that I don't have to open the web UI
11. As a pi 用户, I want CSV export to include token columns, so that I can analyse usage in a spreadsheet
12. As a pi 用户, I want a sensible cache rate display when no cache data exists yet, so that an empty dashboard doesn't show a misleading 0%

## Implementation Decisions

- **新模块 `usage.rs`（纯函数，可单测）**：
  - `UsageSummary { prompt_tokens, completion_tokens, cached_tokens }`
  - `extract_usage(&Value) -> Option<UsageSummary>`：从完整响应 JSON 探测字段，顺序为 Anthropic（`cache_read_input_tokens` + `cache_creation_input_tokens` + `input_tokens`/`output_tokens`）> OpenAI 标准（`prompt_tokens_details.cached_tokens`）> DeepSeek 变体（`prompt_cache_hit_tokens`/`prompt_cache_miss_tokens` 或同类顶层字段）
  - `SseUsageParser`：增量喂入 SSE 文本块（`push(&[u8])`），解析 OpenAI 流（usage 出现在 `[DONE]` 之前的 chunk）与 Anthropic 流（`message_start` 事件携带 input/cache 数据，`message_delta` 携带 output 累计），输出 `Option<UsageSummary>`。不完整/无 usage 的流返回 `None`
- **`stream_response` 改为 tee**：响应流一份照常转发客户端，另一份喂给 `SseUsageParser`；流结束后若解析出 usage，把完整日志行（含 usage 与对话标识）异步补写 `requests.log`。流被客户端中途掐断或取不到 usage → 仍写日志行，token 字段留空
- **转换路径（OpenAI→Anthropic，非流式）**：响应已整体在内存（`r.json().await`），直接 `extract_usage` 后随日志行写出，无额外成本
- **对话标识提取**：请求头 `x-conversation-id` 优先，body `conversation_id` 兜底，写入日志行 `conversationId` 字段
- **`log_request` 扩展**：新增可选字段 `promptTokens` / `completionTokens` / `cachedTokens` / `conversationId`。旧行反序列化为 `None`，向后兼容
- **聚合重构**：`stats.rs::get_stats()` 中的聚合逻辑抽出为纯函数 `aggregate(&[RequestLogEntry]) -> UsageStats`；`get_stats()` 只做"读文件 → aggregate"
- **`UsageStats` 扩展**（响应体向后兼容追加，不改 API 路由）：
  - `totalTokens`: `{ input, output, total }`（累计；未知行不计入）
  - `cacheHitRate`: 字符串百分比（`cached ÷ (cached + uncached)`，仅按输入 token 计算；无缓存数据时返回 `"-"` 而非 `"0%"`）
  - `byConversation`: 按 `conversationId` 分组的列表，含请求数、input/output 累计、最近活跃时间；按最近活跃倒序，截取 Top 20，无标识请求合并为 `unlabeled` 一组
  - `ProviderStats` 追加 `promptTokens` / `outputTokens` / `cachedTokens` 累计列
- **仅成功（`ok=true`）且解析到 usage 的行计入 token 统计**；failover/重试产生的中间行（`ok=false` 或 `retry=true`）不计，避免同一请求重复累计
- **WebUI**：`types.ts` 同步扩展 `UsageStats`；`StatsPanel` 新增 Metric 卡片「Tokens（累计 input+output，可读格式化如 12.3M）」与「Cache 率」；by-provider 表格追加 Tokens 列；新增 "By conversation" 卡片（对话短 id、请求数、累计 tokens，unlabeled 合并行）
- **TUI**：stats 视图追加总 input/output tokens 与缓存命中率两行

## Testing Decisions

只测外部行为：喂入文本/JSON 输入，断言提取与聚合结果。实现细节（内部状态机、字段顺序）不作为断言目标。

- **`usage.rs` 单测**（`#[cfg(test)]`，`cargo test`）：
  - OpenAI 流式：usage chunk 位于 `[DONE]` 前 → 提取正确；无 usage chunk → `None`；跨多个 SSE 块切分（每次 push 半帧）仍能解析
  - Anthropic 流式：`message_start` + `message_delta` → 提取正确；缺失任一事件 → `None`
  - 非流式 JSON：Anthropic 字段 / OpenAI `cached_tokens` / DeepSeek 变体 → 探测顺序正确，只认第一个存在的
  - 缓存率分母：`cached / (cached + uncached)`，输出 token 不参与
- **`aggregate` 单测**：多行日志累计；旧行（无 token 字段）不崩、不计入；`unlabeled` 分组与 Top 20 截断；`ok=false` 行不计 token
- **先例**：`src-rust/proxy.rs` 已有 `#[cfg(test)] mod tests`（当前无覆盖测试，本 spec 补上该区域的第一个覆盖）

## Out of Scope

- JS legacy 实现（`src/proxy.js` / `src/stats.js` / `extensions/index.ts`）不同步
- 对话级 UI 交互（删除、重命名、清空对话统计）
- 历史日志回填（升级前的旧行无 token 数据，不追溯）
- 费用估算（token × 单价）与 by-provider 缓存率展示
- 缓存率 per-request 明细展示（数据在日志里有，UI 不列）

## Further Notes

- 需求中的"单次对话的 token 数量"落地为按会话标识聚合；pi 客户端实际发送的标识字段（头还是 body 字段）在实现阶段抓一次真实请求即可验证——两个字段都探测，天然兼容
- 日志写入由"请求开始即写"变为"流结束后补写"，`requests.log` 的落盘有轻微延迟，可接受
- Rust 侧单测入口为 `cargo test`（`src-rust` 是独立 crate）；实现如需构建验证可用 `npm run build:native`
