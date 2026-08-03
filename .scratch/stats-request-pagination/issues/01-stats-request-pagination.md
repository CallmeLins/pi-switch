# 01 — Stats 请求明细分页

**What to build:** Stats 页请求明细（Request Details）区块支持传统分页，翻看统计窗口内全部历史请求：显示总条数与当前页码/总页数，提供上一页/下一页与页码跳转；每页条数可在 50/100/200/500 间切换（默认 50）；滚动窗口配合自动刷新时若当前页越界自动回退到最后一页；切换统计窗口或每页条数后回到第 1 页；空窗口或无明细数据时不显示分页控件。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 请求明细区块渲染分页控件：总条数（来自窗口内全量行数）+ 上一页/下一页 + 页码按钮组（含首末页与省略号）；第 1 页/末页禁用对应按钮
- [x] 分页请求携带 `page`（0 基，默认 0）/`limit`（默认 50）实参调用统计接口；翻页/跳页触发新请求，聚合指标卡不受翻页影响
- [x] 每页条数四档 50/100/200/500（默认 50），切换后从第 1 页重新加载；档位选择在组件会话内保持，页面重载回默认 50
- [x] 切换统计窗口（预设、自定义日期变更）后重置到第 1 页
- [x] 自动/手动刷新后若当前页超出最新总页数，自动回退到最后一页（仅实际越界时触发）；空窗口或无明细数据时不渲染分页控件
- [x] 请求明细折叠/展开行为保持；既有 StatsPanel 测试的统计接口调用断言同步更新（4 参数 → 5 参数），新增分页相关测试全绿（vitest）

## 实施总结
- 提交：`c39a6ba` — feat: add pagination to stats request details
- 实现的 seams：S1 分页控件渲染（总条数/页码按钮组/边界禁用）｜S2 翻页与跳页请求（page/limit 实参、聚合卡不变）｜S3 每页条数四档切换（默认 50、重置第 1 页）｜S4 窗口切换重置第 1 页（预设与自定义日期）｜S5 越界 clamp 回退 + 空态不渲染控件
- 验收标准：6 条全部 `- [x]`（见上，由 T1–T5 共 5 个新测试 + 既有测试同步更新覆盖；每个 seam 均经突变验证确认测试有效）
- 测试结果：webui vitest 70 passed（StatsPanel.test.tsx 38）｜Rust cargo test 136 passed
- typecheck：通过（`tsc --noEmit`）
- 遗留 / 后续建议：`.scratch/cost-stats/issues/06-request-details-collapse-pagination.md` 在本会话期间丢失（未跟踪、无 git 痕迹），其规格已由本 spec（`.scratch/stats-request-pagination/spec.md`）完整承接；后端默认 limit=100 与 webui 默认 50 的不一致保留（webui 总是显式传 limit）

## 后续备注（2026-08-04 部署验证）
- 部署后 UI 未出现分页控件（diagnosing-bugs 定位）：根因是后端分页实现（`recentRequestTotal` / `aggregate_paged` / `/api/stats` page-limit 解析）仅存在于**未提交的工作区改动**中，会话期间被外部 git 操作清除——git 历史从未包含该代码，rebuild 的 .node 无此字段，webui 的 `totalRows` 恒为 undefined。
- 已重新实现后端分页并提交 `157c254`：`UsageStats.recentRequestTotal`（窗口内全量行数）、`aggregate` 委托 `aggregate_paged`（默认 page 0/limit 100 保持旧行为）、`get_stats_paged`、`/api/stats` 解析 page/limit、service 透传 + 3 个新 Rust 测试（分页切片/默认 100/路由字段）。
- 验证：本地 native `recentRequestTotal=1589`；webui API `page=0&limit=50` → 50 条 + total=1591，page=31 → 41 条（末页），越界 → 0 条，limit=500 → 500 条。Rust 139 passed。
