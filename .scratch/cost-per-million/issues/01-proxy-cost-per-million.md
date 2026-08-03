# 01 — 代理侧 costTotal 按每 1M tokens 单价计价

**What to build:** `compute_cost` 把结果除以 `1_000_000.0`（单价语义为每 1M tokens），`costTotal` 写入值随之正确；旧日志行不重算。端到端行为：带 usage 与单价的请求日志行，`costTotal = (uncached×input + cached×cacheRead + completion×output) / 1e6`；分级档位同单位换算。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] `compute_cost` 按每 1M tokens 换算：`(200−120)×2 + 120×0.5 + 30×1 = 250 / 1e6 = 0.00025` 断言成立
- [x] 分级档位同单位：tier 单价场景断言 `0.000085` 成立
- [x] `build_log_entry` costTotal 写入断言更新为 0.00025
- [x] `cargo test --lib` 全绿、`cargo check --lib` 通过
- [x] CONTEXT.md「消费（Cost）」条目含每 1M tokens 单位说明
- [x] 旧日志行不重算（无代码改动，spec 显式决策）

## 实施总结
- 提交：`dd29957` — fix(proxy): scale cost by per-million-token prices
- 实现的 seams：S1 `compute_cost` 结果除以常量 `COST_PER_MILLION_TOKENS = 1_000_000.0`（注释写明单价 per-1M 语义，tier 档位同单位换算）；S2 `build_log_entry` costTotal 断言 250.0 → 0.00025
- 测试结果：`cargo test --lib` 135 全绿（含更新后的 cached 子集折算 0.00025、tier 档位 0.000085、costTotal 写入 0.00025）；`cargo check --lib` 通过
- 遗留 / 后续建议：旧日志行 costTotal 为修复前错误公式写入的大额值，聚合会混合新旧数值；如需对账可在导出层标注行级单价语义版本（本次不重算，spec 显式决策）
