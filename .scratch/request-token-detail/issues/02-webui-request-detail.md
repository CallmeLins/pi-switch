# 02 — WebUI：请求明细表格与对话卡片补齐

**What to build:** 统计页「By conversation」卡片下方新增请求明细表格，窗口内最近 100 条请求一行一条，能看到单次请求的输入/输出/缓存/推理/缓存率/合计；无 token 数据的行显示 `-`，失败行显示状态/错误。同时「By conversation」每行补齐输入、输出、缓存率三项，与全局卡片口径一致。

**Blocked by:** 01 — 后端：聚合层产出请求明细与对话缓存率

**Status:** resolved (commit `8cdc9b1`, 2026-08-02)

- [x] 类型同步：明细行接口、`UsageStats.recentRequests`、对话统计新增 `cacheRate` 字段（后端字段缺失时前端不崩）
- [x] 请求明细表格渲染：列 = 时间 / provider / model / 状态 / 输入 / 输出 / 缓存 / 推理 / 缓存率 / 合计；按响应顺序显示（已倒序）
- [x] 无 usage 行：token 列与缓存率显示 `-`；失败行显示 status + error（截断）；成功行状态列显示 status
- [x] token 列复用现有格式化函数（可读缩写如 12.3K），缓存率直接显示后端字符串
- [x] 「By conversation」行补齐输入、输出、缓存率三项：行内项数超宽时改两行布局（上行会话短 id + 请求数，下行六项 token 维度平铺），保留 Cached / Reasoning / Total
- [x] 明细为空数组时不渲染该卡片或显示空态，不报错
- [x] 组件测试覆盖：有 usage 行与无 usage 行渲染、缓存率与 `-` 显示、对话行新增三项、空数组不崩；vitest 全绿

## 实施总结

- commit `8cdc9b1`：`webui/src/types.ts` 新增 `RecentRequest` 接口、`UsageStats.recentRequests`、`ConversationStats.cacheRate`（可选）；`webui/src/lib/format.ts` 新增 `formatRequestTime`（时间列，缺失/非法 → `-`）与 `formatRequestToken`（token 列：null → `-`，真实 `0` 显示 `0`，区分无 usage 行与零使用量）；`StatsPanel.tsx` 在「By conversation」下方新增请求明细表格（10 列、状态列 status+error 截断、按响应顺序），「By conversation」行改为两行布局并补齐 Input / Output / Rate 三项。
- TDD：4 个 seam（时间格式化 / 明细表格有 usage 行 / 无 usage 与失败行 / 对话行三项），每 seam 一个红-绿循环。
- Review 修复：Spec 轴发现真实 `0` 被 `formatTokenDimension` 渲染为 `-`（违反「0 是确切测量」），新增 `formatRequestToken` 区分；Standards 轴修复表格列重复、`requestStatus` 双调用与命名、`key={i}` 漂移、types.ts 镜像注释。
- 验证：vitest 47 passed（StatsPanel 22 + format 20 + statsWindow 7），`tsc --noEmit` 通过。
