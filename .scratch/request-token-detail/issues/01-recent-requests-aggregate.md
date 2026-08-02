# 01 — 后端：聚合层产出请求明细与对话缓存率

**What to build:** `/stats` 响应携带最近 100 条请求的明细（受统计窗口过滤、按时间倒序），每条含输入/输出/缓存/推理/合计与后端算好的缓存率字符串；同时每条对话统计带上对话级缓存率。旧日志行（无 token 字段）不崩、显示为无数据。聚合层能力落地并有测试覆盖。

**Blocked by:** None — can start immediately

**Status:** resolved (commit 4b90c1d, 2026-08-02)

- [x] `/stats` 响应新增 `recentRequests` 数组：窗口内按时间倒序、截断 100 条；ts 缺失的行排最后；窗口为空时为空数组
- [x] 明细行字段：ts / provider / model / ok / status / error / promptTokens / completionTokens / cachedTokens / reasoningTokens / totalTokens / cacheRate；无 Token 使用量的行（失败、重试中间行、上游未报告）token 字段为 null
- [x] cacheRate 规则：无 usage 或 input=0 显 `-`；cached=0 显 `0.0%`；否则 `{:.1}%`（与全局缓存命中率同公式 `cached ÷ input`）
- [x] totalTokens = input + output（推理 token 是输出子集，不重复累加）
- [x] `ConversationStats` 新增 `cacheRate`：按该对话聚合后的输入/缓存按同一规则计算
- [x] 既有聚合行为不变（请求数/成功率/窗口过滤/对话 Top20 等无回归）
- [x] 新增聚合层用例覆盖：窗口过滤、倒序截断 100、ts 缺失排最后、无 usage 行 null 字段、cacheRate 三态、对话级 cacheRate、旧行兼容；`cargo test` 全绿

## 实施总结
- 提交：`4b90c1d` — feat: add recent request details and per-conversation cache rate to usage stats
- 实现的 seams（每个 seam 一个红-绿循环，7 个全部完成）：
  - S1 空输入 → `recentRequests` 空数组（Rust 字段 + 序列化 `"recentRequests":[]`）
  - S2 有 usage 行完整字段：元数据透传、5 个 token 字段数值、`totalTokens = input + output`（推理不重复累加）、cacheRate 正常百分比
  - S3 cacheRate 三态：cached=0 → `0.0%`（确切测量）、input=0 → `-`、正常 → `{:.1}%`；提炼 `cache_rate_of(input, cached)` helper
  - S4 无 usage 行（失败 / retry / 旧行无 token 字段）：仍出现在明细、token 字段全 null、cacheRate `-`，不崩
  - S5 窗口过滤：明细与既有聚合共用同一次 `in_window` 遍历，仅窗口内行入列；窗口排除全部 → 空数组
  - S6 倒序与截断：`cmp_ts_desc`（RFC3339 字符串倒序、缺失排最后）排序后 `truncate(100)`
  - S7 对话级 cacheRate：`ConversationStats.cacheRate` 在聚合完成后基于对话 input/cached 计算，三态与行级同口径
- 测试结果：`cargo test` 115 passed（新增 9 个聚合层用例）；clippy 无新增警告（8 个既有警告均在 proxy.rs / tui / 旧逻辑）
- typecheck：通过（cargo test + clippy 编译全量目标）
- Code Review：Standards 轴无 hard violation，3 处 judgement call 已修复（提取 `cmp_ts_desc` 复用两处排序、明细构造去重复为 base+填充）；Spec 轴无缺失、无 scope creep、无实现错误
- 遗留 / 后续建议：
  - WebUI 消费 `recentRequests` / 对话 `cacheRate` 属 issue 02（webui-request-detail）
  - 全局 `cacheHitRate` 与行级/对话级 `cacheRate` 语义差异（cached=0 时 `-` vs `0.0%`）为 spec 设计，非缺陷
