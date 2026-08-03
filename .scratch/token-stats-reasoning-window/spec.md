# 推理 token 维度 + 统计时间窗口

Status: ready-for-agent

## Problem Statement

统计页已能看 token 总量、缓存命中率与单次对话统计，但有两个缺口。其一：使用推理型模型（DeepSeek-R1 等）时，推理 token 往往占大头，现有三列（输入/输出/缓存）看不到"思考花了多少"。其二：所有数字都是全量累计，用户无法回答最基本的问题——"今天烧了多少？最近 7 天呢？"

## Solution

token 统计扩展为四维度（输入/输出/缓存/推理），全局总计与单次对话列表平铺展示，缓存、推理以角标注明子集关系（推理是输出的子集，总数不重复累加）。统计页新增时间选择器：当天（本地自然日）/ 24 小时以内（滚动）/ 7 天以内（滚动）/ 自定义日期区间，窗口作用于整个页面的所有聚合；默认显示当天。时间窗口由前端按本地时区算成 `from/to` 毫秒传给 `/stats` API，后端保持无时区逻辑。

## User Stories

1. As a pi 用户, I want to see the reasoning tokens of a single conversation, so that I can tell how much of my spend went to model thinking
2. As a pi 用户, I want the global token totals to include reasoning tokens, so that I can see the same four dimensions everywhere
3. As a pi 用户, I want cached tokens visible in the totals, so that the four dimensions are complete (input/output/cache/reasoning)
4. As a pi 用户, I want providers that don't report reasoning tokens to count as 0, so that mixed-provider data stays comparable
5. As a pi 用户, I want the total to stay input+output, so that reasoning (a subset of output) is never double-counted
6. As a pi 用户, I want to filter stats to today, so that I can check my daily usage at a glance
7. As a pi 用户, I want a rolling last-24-hours filter, so that "since this time yesterday" is one click away
8. As a pi 用户, I want a rolling last-7-days filter, so that weekly trends don't need a custom range
9. As a pi 用户, I want to pick an arbitrary start/end date, so that I can inspect any historical period
10. As a pi 用户, I want "today" to mean my local calendar day, so that it matches my intuition of what today is
11. As a pi 用户, I want the time window to apply to every number on the page (requests, success rate, latency, providers, models, tokens, conversations), so that the page is internally consistent
12. As a pi 用户, I want today to be the default view, so that opening the stats page answers my most common question first
13. As a pi 用户, I want cached/reasoning counts labelled as subsets of input/output, so that I don't misread the totals
14. As a pi 用户, I want the conversation list to show cache and reasoning columns too, so that per-conversation analysis matches the global view
15. As a pi 用户, I want CSV/JSON export to include the reasoning column, so that spreadsheet analysis stays complete
16. As a pi 用户, I want stats to keep working on log lines written before this change, so that history is not blanked or double-counted
17. As a pi 用户, I want log lines with unparseable timestamps to be excluded from windowed stats, so that garbage data can't poison a window
18. As a pi 用户, I want to switch presets without reloading the page, so that comparing today vs last 7 days is instant

## Implementation Decisions

- **`UsageSummary` 扩展**：新增 `reasoning_tokens: u64` 字段
- **`extract_usage` 推理 token 探测路径**：`usage.completion_tokens_details.reasoning_tokens`（Chat Completions / DeepSeek）优先，其次 `usage.output_tokens_details.reasoning_tokens`（Responses API）；都不存在记 0。Anthropic 无此字段，天然记 0
- **`SseUsageParser`**：OpenAI 流式 usage 帧同样探测 `completion_tokens_details`；Anthropic 流无推理字段，记 0
- **日志行扩展**：`build_log_entry` 追加 `reasoningTokens` 字段；`RequestLogEntry` 加 `reasoning_tokens: Option<u64>`（`#[serde(default)]`，旧行反序列化为 None）
- **聚合口径**：内部 `TokenUsage` 加 `reasoning`；`usage_of` 保持"成功且非 retry 且解析到 usage 才计入"，推理缺失按 0 计入
- **`TokenTotals` 扩展**：新增 `cached`、`reasoning` 字段（`total = input + output` 不变，缓存与推理均为子集）
- **`ConversationStats` 扩展**：新增 `cachedTokens`、`reasoningTokens`；合计列由前端计算（= input + output）
- **`ProviderStats` 扩展**：新增 `reasoningTokens`，与既有三列对齐
- **`aggregate` 签名扩展**：新增时间窗口参数（`from_ms`/`to_ms`，毫秒，左闭右开）。窗口过滤在循环前对 entries 执行：`ts` 缺失或 RFC3339 解析失败的 entry 视为窗口外（排除）；窗口内的 entry 参与全部聚合（请求数/成功率/延迟/provider/模型/token/对话）
- **`/stats` API**：新增 query 参数 `range=today|last24h|last7d|custom` 与 `from`/`to`（毫秒，custom 时必填）。后端只做窗口透传与过滤，不做时区计算
- **窗口语义**（由前端按本地时区计算）：当天 = [本地 0 点, now)；24h = [now-24h, now)；7 天 = [now-7×24h, now)；自定义 = [起日 0 点, 止日 24 点)，止日可选今天（右界不裁剪）
- **webui**：StatsPanel 顶部时间选择器——4 个预设按钮（当天/24h/7天/自定义）+ 自定义时两个 date 输入；默认当天；切换即重新请求并整体重渲染。token 卡片平铺 5 格（输入/输出/缓存/推理/合计），缓存与推理带子集角标；对话列表加缓存/推理/合计三列；`format.ts` 新增/扩展格式化函数
- **导出**：`export_logs_json` / `csv_of` 追加 `reasoningTokens` 列（旧行缺失按 0）；导出 API 不引入时间过滤参数（保持全量导出）

## Testing Decisions

- 好的测试 = 只测外部行为：给定 entries 集合与窗口参数，断言聚合结果的窗口边界、维度累加与兼容性——不测内部实现细节
- **src-rust/stats.rs**（核心 seam，先例丰富：`aggregate_sums_tokens_only_for_successful_non_retry_entries_with_usage`、`aggregate_cache_hit_rate_is_cached_over_total_input` 等 15+ 用例）：
  - 时间窗口：窗口内/外/边界（from 包含、to 排除）、脏 ts 排除、无窗口参数时全量行为不变
  - reasoning 累加：总计数与会话计数、`total` 不随 reasoning 增加、缺失按 0
  - 旧行兼容：无 reasoningTokens 字段的日志行
  - 导出：CSV/JSON 含 reasoningTokens 列
- **src-rust/usage.rs**（先例：`extract_usage` 字段探测顺序 3 组用例、`sse_parser_matches_whole_stream_result...`）：推理字段三条路径（chat completions / responses / 缺失）
- **webui**（先例：`StatsPanel.test.tsx` 4 用例、`format.test.ts`）：时间选择器渲染与切换请求参数、5 格平铺渲染、格式化函数
- 覆盖工具：Rust 侧 `cargo test --lib`；webui 侧 vitest（沿用现有配置）

## Out of Scope

- TUI 时间范围切换（TUI 保持全量展示；`aggregate` 参数化为 TUI 预留了能力但不做 UI）
- byModel 的 token 维度
- 对话详情视图（对话内按 model/provider 拆分）
- 导出的时间过滤
- 费用换算

## Further Notes

- 术语已更新：`CONTEXT.md` 扩展 Token 使用量为四部分，新增"推理 token"、"统计窗口"条目
- 决策已记录：`docs/adr/0003-reasoning-tokens-subset-of-output.md`
- 上一 feature `.scratch/token-usage-stats/`（总量/单次对话/缓存命中率）已完成并审计通过，本 spec 在其之上扩展
- **UI 原型**：`prototype/stats-window` 分支（throwaway，未合并）含三个统计页变体（A 分段+卡片、B 左栏+堆叠条、C 下拉+紧凑密度），实现 UI 前可 `git show prototype/stats-window:webui/src/components/StatsPanelPrototype.tsx` 参考；原型未定论具体形态
