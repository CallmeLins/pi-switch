# 消费统计（Cost Stats）+ 统计页实时刷新

Status: ready-for-agent

## Problem Statement

pi-switch 统计页已能看 token 使用量（总量、单次对话、单次请求、缓存、推理），但看不到"花了多少钱"——用户无法回答"今天烧了多少钱、这个对话花了多少、刚才那条请求值多少"。同时统计页只有手动刷新，盯着实时数据时需反复点击。本 feature 补上消费（Cost）维度（总消费 / 对话消费 / 请求消费）与统计页实时刷新（5s / 30s / 5min）。

## Solution

代理请求完成时，按当时的 profile 模型单价（含分级档位）把消费折算后写入请求日志行（单价定格，历史不受后续改价影响）；统计聚合沿用现有 token 口径把消费求和到总消费、按对话聚合、按请求明细展示。模型未配置单价的行消费记为 unknown，展示 `-`。统计页新增消费卡片（总消费 + unknown 提示）、对话列表消费列、请求明细消费列，并新增实时刷新四档（Off / 5s / 30s / 5min，默认 Off）；TUI Stats 页 Overview 同步总消费行。

## User Stories

1. As a pi 用户, I want to see the total cost of all requests in the current stats window, so that I can answer "how much did I spend" at a glance
2. As a pi 用户, I want to see the cost of a single conversation, so that I can tell how much one dialogue cost
3. As a pi 用户, I want to see the cost of a single request, so that I can tell what one call was worth
4. As a pi 用户, I want cost calculated with the model's unit price at request time, so that later price changes never rewrite history
5. As a pi 用户, I want tiered pricing honored, so that models with input thresholds are billed correctly
6. As a pi 用户, I want requests whose model has no configured price to show "-", so that I never mistake missing pricing for free
7. As a pi 用户, I want explicitly zero prices to show $0.00, so that free models stay recognizable
8. As a pi 用户, I want cost formatted with $ and adaptive precision, so that both tiny and large amounts stay readable
9. As a pi 用户, I want cost totals to use the same rows as token totals, so that cost and token numbers always agree
10. As a pi 用户, I want old log lines without cost fields to keep working, so that history is not broken by this change
11. As a pi 用户, I want the stats page to auto-refresh every 5 seconds, so that I can watch live usage
12. As a pi 用户, I want a 30-second interval, so that live-ish updates cost less traffic
13. As a pi 用户, I want a 5-minute interval, so that long monitoring sessions stay cheap
14. As a pi 用户, I want auto-refresh off by default, so that the page does not poll unless I ask
15. As a pi 用户, I want failed auto-refreshes to keep the current data, so that a transient error never blanks the page
16. As a pi 用户, I want the refresh interval to keep my selected stats window, so that auto-refresh respects my filters
17. As a pi 用户, I want the TUI stats overview to show total cost, so that I can check spend without opening the web UI

## Implementation Decisions

- **请求日志扩展**：日志行新增 `costTotal` 字段（`Option<f64>`；旧行反序列化为 None）。消费 = (prompt − cached)×input 单价 + cached×cacheRead 单价 + completion×output 单价；cacheWrite 单价无对应 token 数据，不参与；分级档位按本次请求输入 token 量选择
- **单价来源**：请求完成时重载的 profile 配置（沿用现有 per-request reload），按日志行的 provider + model 查单价；查不到 → 该行消费 unknown
- **聚合口径**：与 token 完全同口径（仅成功且非 retry 且解析到 usage 的行）——消费求和沿用同一过滤，保证 cost/token 数字自洽
- **`UsageStats` 扩展**：新增 `totalCost`（已知行消费总和）与 `costUnknown`（unknown 行数）；`ConversationStats` 追加 `cost`；`RecentRequest` 追加 `cost`；webui `types.ts` 镜像同步
- **WebUI 展示**：统计页顶部消费卡片（总消费 + unknown 提示）；对话列表消费列；请求明细消费列；格式化函数 `formatCost`（$ 前缀自适应：$0.00 / $0.0042 / $12.34 / $1.2K / `-`）
- **实时刷新**：统计页新增 Off / 5s / 30s / 5min 四档，默认 Off；切换档位即启停定时器（卸载清理）；复用现有请求防竞态机制；自动刷新失败保留旧数据（不清空）；刷新沿用当前统计窗口参数
- **TUI**：Stats 页 Overview 追加总消费行（全 unknown 显示 `-`）；i18n 词条同步
- **导出**：CSV / JSON 导出追加消费列（旧行缺失按空处理）

## Testing Decisions

- 好的测试 = 只测外部行为：给定带单价配置与 usage 的日志行，断言消费计算与聚合结果——不测内部实现细节
- **Rust proxy 侧**（先例：`stream_tee_*`、`build_log_entry` 相关）：消费计算（含 cached 子集折算、分级档位选择、缺单价 unknown）、costTotal 写入
- **Rust stats 侧**（先例：`aggregate_sums_tokens_only_for_*`、旧行兼容测试）：totalCost 求和、unknown 计数、byConversation 消费、旧行 None 兼容、CSV/JSON 导出列
- **webui**（先例：`StatsPanel.test.tsx`、`format.test.ts`）：消费卡片/列渲染、`formatCost` 格式规则、刷新档位切换与定时器启停、刷新失败保留旧数据
- **TUI**（先例：`token_summary_*`）：总消费行渲染、全 unknown 显示 `-`
- 覆盖工具：Rust `cargo test --lib`；webui vitest

## Out of Scope

- 按 provider / model 拆分的消费列（数据已含，可后续加）
- cacheWrite 单价计入（无对应 token 数据）
- 对话消费的端到端正确性依赖 conversation-id-inject 插件（已另有 spec + tickets）；本 feature 只交付聚合能力并声明该依赖
- 货币换算 / 多币种支持
- 消费分解明细审计（input / output / cached 分别多少钱）
- 实时刷新期间的过渡动画 / 闪烁抑制（静默更新即可）

## Further Notes

- 术语已更新：CONTEXT.md 新增「消费（Cost）」条目——token 使用量 × 模型单价折算，请求时定格，缺单价记 unknown
- 与 token 统计同口径，保证 cost/token 数字自洽
- 上一 feature `conversation-id-inject`（`.scratch/conversation-id-inject/`）提供对话归组标识，本 feature 的对话消费依赖其生效后自动正确
