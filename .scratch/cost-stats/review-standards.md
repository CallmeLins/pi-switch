# Cost-stats 标准合规审查

基线: HEAD d3ee7d8; 无 CODING_STANDARDS.md, 唯一文档标准为 CONTEXT.md 词汇。

## 文档标准 (CONTEXT.md)

**无硬性违规。** 核对:
- 「消费（Cost）」术语合规: i18n `stats_cost` = "Cost"/"消费" (i18n.rs:189); webui 卡片/列均用 "Cost"; 注释用 cost 而非费用/花费/金额/账单 (diff 全量 grep 无 Avoid 词)。
- 「unknown」用于消费语义获 CONTEXT.md 背书 ("模型未配置单价时该请求的消费记为 unknown"), `cost_unknown`/`costTotal: null` 用法一致; 对话归组仍用 `unlabeled`, 未误用 unknown。
- 「统计窗口」口径: 自动刷新复用 `computeStatsWindow` 当前窗口, 与「窗口作用于整个统计页」一致 (StatsPanel.tsx 轮询 effect)。
- 消费聚合与 token 同口径 (`usage_of` 过滤成功且非 retry), 满足 "cost/token 数字自洽"。

## 基线气味 (judgement calls)

### src-rust/proxy.rs
- **Duplicated Code**: 测试内同两句连写两次 —
  `let total = super::compute_cost(&usage, &cost); assert_eq!(total, 85.0, "tier prices: ...")` (proxy.rs tests, ×2 逐字重复)。
- **Long Parameter List / Middle Man**: `log_request` 增至 8 个位置参数, 并逐字段重建 `StreamLogFields` (proxy.rs:1657); 本 diff 追加第 8 个 `cost` 参数加剧。
- **性能/Feature Envy (judgement)**: `lookup_model_cost` 每次请求 `serde_json::from_value(config.profiles.get(provider)?.clone())` 全量反序列化 ProviderProfile 只为取一个 `m.cost` (proxy.rs:1574)。

### src-rust/stats.rs
- **Duplicated Code**: 求和惯用法 `Some(x.unwrap_or(0.0) + c)` 在 total (stats.rs:370) 与 per-conversation (stats.rs:407) 重复, 可抽 helper。
- **Duplicated Code**: 测试 `let with_cost = r#"{"ok":true,"costTotal":0.25}"#; ... assert_eq!(...)` 逐字 ×2 (deserializes_legacy 测试)。
- **Data Clump / Primitive Obsession**: 消费表示为两个共变原始量 `total_cost: Option<f64>` + `cost_unknown: u64`, Rust/TS 双端镜像。
- **不对称 (judgement)**: 总量统计 unknown 行数, 对话级 unknown 行静默丢弃且无提示 (stats.rs:404 vs 370)。

### src-rust/tui/ui/pages.rs
- **Duplicated Code / Divergent Change (跨语言)**: `format_cost` 与 webui `formatCost` 复刻同一套自适应精度规则 (注释自认 "sharing the web UI rules", pages.rs:653; format.ts:46); 双端漂移风险。
- **精度 (judgement)**: `format_token_count(cost as u64)` 截断 (pages.rs:672), 与 TS 端小数行为偶合。

### webui/src/lib/format.ts
- 同上跨语言重复; 内部复用 `formatTokenCount` 良好。

### webui/src/components/StatsPanel.tsx
- **Flag Argument (judgement)**: `load(range, from, to, keepOnError = false)` 布尔开关改变错误语义 (清空 vs 保留), 两处调用 (StatsPanel.tsx:39)。
- 测试硬编码 dash 计数 `getAllByText("-").length).toBe(7)` (Magic Number, 延续既有 6→7 模式)。

### webui/src/types.ts
- 与 serde rename 镜像一致 (costTotal/totalCost/costUnknown), 无问题。
