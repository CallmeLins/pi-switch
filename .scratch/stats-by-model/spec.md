# Stats 面板按模型统计（Spec）

**状态**: ready-for-agent（2026-08-07）

## Problem Statement

webui Stats 面板目前只有"按供应商"（By provider）统计，列仅为 合计/确定/成功率/Token，无法按模型维度查看 token 明细与成本；且两处都缺少 输入/输出/缓存/缓存率/消费价格 的明细列，与「按对话」表格的字段丰富度不一致。

## 需求

1. **新增「按模型」（By model）统计表格**：与「按供应商」并列展示。
2. **「按供应商」与「按模型」两个表格统一列集**：名称 / 合计 / 确定 / 成功率 / 输入 / 输出 / 缓存 / 总 / 缓存率 / 消费价格。
   - 现有「按供应商」的 `Token` 单列拆分为：输入 / 输出 / 缓存 / 总 / 缓存率 / 消费价格 六列；合计/确定/成功率保留。
   - 「按模型」为新建表格，同列集。

## 字段语义（与现有统计口径一致）

- **输入/输出/缓存**：该维度内成功且非重试、有 usage 的行的 token 累计（`usage_of` 口径，与总量/对话一致）。
- **总**：输入 + 输出。
- **缓存率**：`cached / input`，无输入显示 `-`，0 缓存显示 `0.0%`，否则一位小数百分比（复用 `cache_rate_of`）。
- **消费价格**：该维度内 usage 行 `costTotal` 之和；全部未知（无价格）显示 `-`（`formatCost(null)`），部分未知时按已知行求和（与总量 `totalCost` 语义一致）。
- 请求数/OK/成功率沿用现有 by_provider/by_model 计数。

## 数据与展示

- **Rust 侧**（`src-rust/stats.rs`）：
  - `ModelStats` 扩展：`prompt_tokens / output_tokens / cached_tokens / reasoning_tokens / cache_rate / cost`（序列化 `promptTokens/outputTokens/cachedTokens/reasoningTokens/cacheRate/cost`）。
  - `ProviderStats` 增加 `cost: Option<f64>`（序列化 `cost`，`#[serde(default)]`）。
  - `aggregate_paged` 的 by_model 循环累加 token/cost 并计算 cacheRate；by_provider 循环累加 cost。
  - 排序：by_model 与 by_provider 保持现状（HashMap 迭代序，webui `Object.entries` 渲染，不新增排序要求）。
- **webui 侧**：
  - `webui/src/types.ts`：`ModelStats` 接口扩展 + `ProviderStats.cost`。
  - `webui/src/components/StatsPanel.tsx`：By provider 表格列改造（Token 列 → 六列）+ 新增 By model 表格。
  - i18n：新增 `"By model"` key（按模型）；输入/输出/缓存/总/缓存率/消费价格复用现有 key。

## 作用范围与边界

- 不改 requests.log 字段格式，不改默认统计口径（真实消耗）。
- 不改「按对话」表格与总量指标。
- 旧日志行（无 cost）正常渲染（cost 为 null → `-`）。
- 新增字段均为向后兼容（`#[serde(default)]` / 可选）。

## 验收标准

- [ ] `by_model` 聚合返回 token 明细（prompt/output/cached/reasoning）、cacheRate 字符串、cost 累加
- [ ] `by_provider` 聚合返回 cost（有价格行求和、全未知为 null）
- [ ] 旧行/无 cost 数据渲染 `-`，不影响其他统计
- [ ] webui By provider 表格显示六列明细且数值正确
- [ ] webui By model 表格显示六列明细且数值正确
- [ ] cargo test 全绿、webui vitest 全绿（NODE_ENV=test）
