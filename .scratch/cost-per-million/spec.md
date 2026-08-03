# Cost 按每 1M tokens 单价计价（Cost per Million Tokens）

Status: ready-for-agent

## Problem Statement

`compute_cost`（src-rust/proxy.rs）把 `tokens × 单价` 直接写入 `costTotal`，但 profile 模型配置的单价语义是**每 1M tokens 的价格**（行业惯例，如 `input: 2.0` 表示 $2 / 1M tokens）。当前实现未做单位换算，金额被放大 1e6 倍——如 `(200−120)×2 + 120×0.5 + 30×1 = 250` 实际应为 `$0.00025`。

## Solution

`compute_cost` 在汇总 `uncached×input + cached×cacheRead + completion×output` 后除以常量 `COST_PER_MILLION_TOKENS = 1_000_000.0`；函数注释写明单价语义。旧日志行（按错误公式写入的大额 costTotal）**不重算**——append-only 日志 + 单价定格原则，历史数据保留原值。

## User Stories

1. As a pi 用户, I want cost calculated with per-1M-token unit prices, so that the displayed cost matches the real spend
2. As a pi 用户, I want tiered pricing honored with the same unit semantics, so that thresholds never change the unit

## Implementation Decisions

- `compute_cost` 结果除以 `1_000_000.0`（常量 `COST_PER_MILLION_TOKENS`）；tier 档位的 input/output/cacheRead 单价同为 per-1M 语义，一并换算
- 旧日志 costTotal 不重算（append-only + 单价定格；修复只影响修复后写入的新行）
- 显示层（webui `formatCost` / TUI `format_cost`）自适应精度规则不变；`$0.00025` 四舍五入显示 `$0.0003` 属既有行为
- CONTEXT.md「消费（Cost）」条目补充"单价按每 1M tokens 计"

## Testing Decisions

- 只测外部行为：给定带 usage 与单价的输入，断言 `compute_cost` 返回值与 `costTotal` 写入值
- 更新 3 处断言：`compute_cost_converts_cached_subset_at_cache_read_price`（250.0 → 0.00025）、`compute_cost_uses_tier_price_when_input_tokens_reach_threshold`（85.0 → 0.000085）、`log_entry_writes_cost_total_when_model_has_price`（250.0 → 0.00025）

## Out of Scope

- 历史日志 costTotal 迁移/重算
- 显示精度调整（亚美分坍缩、四舍五入）
- 配置迁移（现有 profile 单价值即按 per-1M 语义解读）

## Further Notes

- 单价来源与定格逻辑（请求完成时按当时 profile 重载查价）不变
- 与 token 统计同口径（仅成功且非 retry 且解析到 usage 的行计入）不变
