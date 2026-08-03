# 单次请求 Token 使用量明细

Status: ready-for-agent

## Problem Statement

统计页已经能看全局 Token 使用量、缓存命中率与按对话聚合的统计，但粒度停在"对话"这一层：用户想回答"刚才那条请求到底烧了多少 token、缓存命中率多高"，翻遍页面找不到单次请求的明细。上一 feature 明确把"缓存率 per-request 明细展示"划出范围（数据在请求日志里有，UI 不列）——本 feature 补上这一层：统计窗口内的每个 request 一行明细，输入 / 输出 / 缓存 / 推理 / 缓存率 / 合计一目了然。

## Solution

统计页新增「请求明细」表格：受统计窗口过滤，按时间倒序截取最近 100 条请求，每行展示时间、provider、model、状态、输入、输出、缓存、推理、缓存率（百分比）与合计。有 Token 使用量的行显示真实数值（缓存率为 0 时显示 0.0%）；无使用量（失败、重试中间行、上游未报告）的行 token 列与缓存率显示 `-`，失败行以状态码/错误信息区分。数据由 `/stats` 响应体携带，不新增 API。

同时补齐「By conversation」卡片：每行追加输入、输出与缓存率三项（现有 Cached / Reasoning / Total 保留），对话级缓存率与行级同口径。

## User Stories

1. As a pi 用户, I want to see each request's prompt tokens, so that I can tell how much context a single call consumed
2. As a pi 用户, I want to see each request's completion tokens, so that I can judge the size of a single response
3. As a pi 用户, I want to see each request's cached input tokens, so that I can see how much of the context hit the cache
4. As a pi 用户, I want to see each request's reasoning tokens, so that I can tell how much a single call spent on model thinking
5. As a pi 用户, I want to see each request's cache hit rate as a percentage, so that I can judge caching benefit per request
6. As a pi 用户, I want to see each request's total tokens, so that one glance answers "how much did this call cost"
7. As a pi 用户, I want the request list filtered by the current stats window, so that the table matches the rest of the page's numbers
8. As a pi 用户, I want the list capped at the 100 most recent requests, so that the page stays fast without pagination
9. As a pi 用户, I want requests without token data to show "-" instead of 0, so that I never mistake missing data for a zero measurement
10. As a pi 用户, I want failed requests to still appear with their status/error, so that the table accounts for every request in the window
11. As a pi 用户, I want a cache rate of 0.0% to show as a real measurement, so that "zero hits" is distinguishable from "no data"
12. As a pi 用户, I want the total to be input + output, so that reasoning (a subset of output) is never double-counted
13. As a pi 用户, I want log lines written before this feature to render gracefully, so that upgrading doesn't break the table
14. As a pi 用户, I want the table to sit under the conversation list, so that the page reads from broad aggregates down to request detail
15. As a pi 用户, I want each conversation row to show its input tokens, so that I can compare context size across conversations at a glance
16. As a pi 用户, I want each conversation row to show its output tokens, so that I can see which conversations produced the most
17. As a pi 用户, I want each conversation row to show its cache hit rate as a percentage, so that I can judge which conversations benefit from caching

## Implementation Decisions

- **`UsageStats` 扩展**：新增 `recentRequests: Vec<RecentRequest>`（窗口内按时间倒序、截断 100 条；空窗口为空数组）。响应体向后兼容追加，`/stats` API 与 service 透传层零改动
- **`RecentRequest` 响应形状**（token 字段为 `null` 表示该行无 Token 使用量，前端显示 `-`；`cacheRate` 为后端算好的字符串）：

```json
{
  "ts": "2026-08-02T10:00:00.000Z",
  "provider": "deepseek",
  "model": "deepseek-chat",
  "ok": true,
  "status": 200,
  "error": null,
  "promptTokens": 1234,
  "completionTokens": 567,
  "cachedTokens": 890,
  "reasoningTokens": 100,
  "totalTokens": 1801,
  "cacheRate": "72.1%"
}
```

- **收集逻辑在 `aggregate` 内**：循环中对每个通过 `in_window` 的 entry 构造 `RecentRequest`，与既有聚合共用同一次遍历；token 字段取自 `usage_of` 结果（成功且非 retry 且解析到 usage），无 usage 时为 `null`；排序按 `ts` 倒序（RFC3339 字符串比较），`ts` 缺失的行排最后，再 `truncate(100)`
- **`cacheRate` 规则**（与全局 `cacheHitRate` 同公式 `cached ÷ input`，显示规则不同，全局汇总缓存为 0 显示 `-`，行级有 usage 时 0 是确切测量）：
  - 无 usage → `"-"`
  - 有 usage 且 input = 0 → `"-"`（防御除零）
  - 有 usage 且 cached = 0 → `"0.0%"`
  - 其余 → `"{:.1}%"`（如 `"72.1%"`）
- **`ConversationStats` 扩展**：新增 `cacheRate` 字段，聚合该对话全部可计数的 Token 使用量后按与行级相同的规则计算（input=0 → `-`，cached=0 → `0.0%`，否则 `{:.1}%`）；`inputTokens`/`outputTokens` 字段已存在，无需动聚合逻辑
- **`totalTokens`** = input + output，沿用推理 token 是输出子集、合计不重复累加的既有口径；无 usage 为 `null`
- **WebUI**：`types.ts` 新增 `RecentRequest` 接口与 `UsageStats.recentRequests` 字段、`ConversationStats` 追加 `cacheRate`；`StatsPanel` 在「By conversation」卡片下方新增请求明细表格——列：时间 / provider / model / 状态（成功显示 status，失败显示 status + error 截断）/ 输入 / 输出 / 缓存 / 推理 / 缓存率 / 合计；token 列与缓存率复用现有格式化函数（`-` 处理与现有卡片一致），时间列复用现有时间格式化；无 usage 行整行 token 列与缓存率显示 `-`
- **「By conversation」行扩展**：追加输入、输出、缓存率三项展示（现有 requests / Cached / Reasoning / Total 保留）；追加后行内项数超宽，改为两行布局——上行会话短 id + requests 数，下行六个 token 维度（输入 / 输出 / 缓存 / 推理 / 缓存率 / 合计）平铺，沿用现有紧凑样式；缓存率直接显示后端 `cacheRate` 字符串
- 旧日志行（无 token 字段）反序列化为 `null`，天然兼容

## Testing Decisions

只测外部行为：给定 entries 集合与窗口参数，断言明细集合的行数、排序、字段值与缓存率字符串——不测收集顺序等内部实现细节。

- **`aggregate` 用例**（src-rust/stats.rs，先例丰富：`aggregate_*` 15+ 用例）：
  - 窗口过滤：窗口外 entry 不进明细；无窗口参数时全量行为不变
  - 排序与截断：按 ts 倒序、恰好截到 100 条；ts 缺失的行排最后且不崩
  - 无 usage 行（失败 / retry / 上游未报告）：token 字段为 null、cacheRate 为 `"-"`
  - 有 usage 行：cacheRate 规则三态（`0.0%` / 百分比 / input=0 时 `-`）、`totalTokens` = input + output
  - 对话级 cacheRate：聚合后 input=0 → `-`、cached=0 → `0.0%`、正常百分比
  - 旧行兼容：无 token 字段的日志行产出 null 字段
- **WebUI 用例**（先例：`StatsPanel.test.tsx`、`format.test.ts`）：表格渲染有 usage 行与无 usage 行、缓存率与 `-` 的显示、空明细数组不崩、对话行渲染新增三项

## Out of Scope

- TUI 请求明细（TUI 保持全量展示，聚合层能力已具备，将来可加）
- 分页 / 滚动加载 / 排序选项（固定最近 100 条倒序）
- 对话详情视图（对话内按 model/provider 拆分请求，仍是既有范围外）
- 独立 `/requests` endpoint（明细随 `/stats` 响应携带）
- 导出变化（CSV/JSON 导出已含 token 列，本 feature 不改）
- 请求明细按 provider/model 筛选

## Further Notes

- 上一 feature（`.scratch/token-usage-stats/`）的 out-of-scope「缓存率 per-request 明细展示（数据在日志里有，UI 不列）」由本 feature 覆盖
- 「By conversation」的输入/输出/缓存率补齐源自实现后复查：对话级数据后端早已聚合，仅 UI 未展示
- 术语无新增：「Token 使用量」「缓存命中率」「统计窗口」「请求日志」等既有术语覆盖全部表述；单次请求的缓存率是「缓存命中率」在行级的实例化
- 不新增 ADR：本 feature 全部决策均可低成本逆转，且无上下文费解之处
