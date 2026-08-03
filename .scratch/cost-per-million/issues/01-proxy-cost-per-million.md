# 01 — 代理侧 costTotal 按每 1M tokens 单价计价

**What to build:** `compute_cost` 把结果除以 `1_000_000.0`（单价语义为每 1M tokens），`costTotal` 写入值随之正确；旧日志行不重算。端到端行为：带 usage 与单价的请求日志行，`costTotal = (uncached×input + cached×cacheRead + completion×output) / 1e6`；分级档位同单位换算。

**Blocked by:** None — can start immediately

**Status:** in-progress

- [ ] `compute_cost` 按每 1M tokens 换算：`(200−120)×2 + 120×0.5 + 30×1 = 250 / 1e6 = 0.00025` 断言成立
- [ ] 分级档位同单位：tier 单价场景断言 `0.000085` 成立
- [ ] `build_log_entry` costTotal 写入断言更新为 0.00025
- [ ] `cargo test --lib` 全绿、`cargo check --lib` 通过
- [ ] CONTEXT.md「消费（Cost）」条目含每 1M tokens 单位说明
- [ ] 旧日志行不重算（无代码改动，spec 显式决策）

## 实施总结

（实现后填写）
