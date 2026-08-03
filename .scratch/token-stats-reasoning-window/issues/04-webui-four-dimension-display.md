# 04 — webui 四维度展示

**What to build:** 统计页的 token 展示扩展为四维度：全局 token 卡片平铺 5 格（输入/输出/缓存/推理/合计），缓存、推理以角标注明子集关系（缓存含于输入、推理含于输出）；单次对话列表加缓存/推理/合计三列；类型定义与格式化函数同步扩展；无数据时保持既有占位展示，旧数据不回归。

**Blocked by:** 01 — 推理 token 全链路（/stats 响应已含四维度字段）

**Status:** resolved (commit 386724e, 2026-08-02)

- [x] token 卡片平铺 5 格：输入/输出/缓存/推理/合计，子集角标清晰
- [x] 单次对话列表显示缓存、推理、合计三列
- [x] 格式化函数处理四维度（含 0 与缺失 → 占位），总量展示不变
- [x] 旧数据（无缓存/推理字段）不回归，空状态保持
- [x] vitest 用例覆盖 5 格渲染与格式化

## 实施总结
- 提交：`386724e` — feat: show four-dimension token breakdown in usage stats web UI
- 实现的 seams：
  - `format.ts::formatTokenDimension(count?: number)`：0 或缺失 → `-` 占位，非 0 → `formatTokenCount`（四维度共用）
  - `StatsPanel` token 卡片平铺 5 格（Input/Output/Cached/Reasoning/Total），Cached 带 `⊆ Input`、Reasoning 带 `⊆ Output` 子集角标；Total 复用 `formatTotalTokens`（总量展示不变）；原指标行 "Tokens" 卡片移除（合计并入 5 格），指标行网格 6→5 列
  - 对话列表每行新增 Cached / Reasoning / Total（= input+output）三列，统一走占位规则（0 → `-`）
  - `types.ts`：`TokenTotals` 加 `cached`/`reasoning`，`ProviderStats` 加 `reasoningTokens`，`ConversationStats` 加 `cachedTokens`/`reasoningTokens`（与 Rust struct 对齐）
- 测试结果：webui vitest 37 passed 全绿（新增 6 个：formatTokenDimension 3 个、5 格渲染+角标、对话三列、旧数据兼容）；Rust `cargo test` 105 passed 不受影响；`tsc --noEmit` 通过
- Review 修正：fixture 四维度数据自洽（provider/全局/对话聚合一致）；`⊂` 改 `⊆`（缓存可全命中等于输入）；对话合计列 0 值改占位
- 遗留 / 后续建议：`legacyStats()` fixture 命名保留（表示历史全 0 数据）；`formatTotalTokens`/`formatTokenDimension` 的 dash 守卫重复为可接受的轻微重复，未强行抽象
