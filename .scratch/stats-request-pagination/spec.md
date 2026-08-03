# Stats 请求明细历史分页

Status: ready-for-agent

## Problem Statement

Stats 页的请求明细（Request Details）表格只展示最近 50 条（后端默认上限 100）请求记录，更早的历史数据在页面上看不到。随着请求日志（Request Log）累积，明细列表退化为截断视图——用户无法回顾、核对统计窗口（Stats Window）内全部请求，与"查询历史所有数据"的需求相悖。

## Solution

在请求明细区块引入传统分页：展示窗口内总条数、当前页码与总页数，提供上一页/下一页、页码跳转和每页条数切换（50/100/200/500，默认 50）。分页请求复用后端已有的 `/stats` `page`/`limit` 查询参数与 `recentRequestTotal` 字段（窗口内全量行数，独立于页码），后端零改动。today/24h/7d 滚动窗口配合自动刷新时数据随时间推进，翻页后若当前页超出最新总页数，自动回退到最后一页。

## User Stories

1. As a pi-switch user, I want to see the total number of request rows in the current 统计窗口, so that I know the scale of the data I'm about to browse
2. As a pi-switch user, I want to page forward through older 请求明细 rows, so that I can review all historical requests inside the window
3. As a pi-switch user, I want to jump directly to a specific page, so that I can reach a known point in history without clicking through
4. As a pi-switch user, I want to know my current page and the total page count, so that I can orient myself while browsing
5. As a pi-switch user, I want to change the rows-per-page between 50/100/200/500, so that I can trade row density against page count
6. As a pi-switch user, I want the previous/next buttons disabled at the first/last page, so that I get clear feedback at the boundaries
7. As a pi-switch user, I want the total count to always reflect the full in-window row count even on later pages, so that pagination math stays consistent
8. As a pi-switch user, I want auto-refresh to keep working while I'm on a later page, so that live data never goes stale — and if the rolling window shrinks the page count, I want to land on the last valid page instead of an empty one
9. As a pi-switch user, I want switching 统计窗口 (preset/自定义日期) to reset to page 1, so that each window starts from the newest rows
10. As a pi-switch user, I want changing rows-per-page to restart from page 1, so that the list never starts mid-stream
11. As a pi-switch user, I want the 请求明细 card to keep its collapse/expand behaviour, so that the new controls don't clutter the stats page
12. As a pi-switch user, I want an empty window or absent 请求明细 data to show no pagination controls, so that the empty state stays clean

## Implementation Decisions

- 复用既有后端分页能力：`/stats` 的 0 基 `page` 与 `limit` 查询参数、响应中的 `recentRequestTotal`（窗口内全量行数，独立于页码）已存在，本次不改后端
- 每页条数档位 50/100/200/500，默认 50（webui 现状默认值）；档位选择在组件会话内保持，页面重载回默认 50
- 分页控件组成：总条数文本 + 上一页/下一页按钮 + 页码按钮组（当前页邻域 + 首末页与省略号）；边界页禁用对应按钮
- 窗口切换（preset、自定义日期变更）与每页条数变更均重置到第 1 页；页码为 0 基
- 滚动窗口漂移处理：任何刷新（手动/自动）后若当前页 ≥ 最新总页数，回退到最后一页（clamp）；空窗口或 totalPages=0 时不显示分页控件
- 聚合指标卡、By provider、By conversation 区块不受影响；分页仅作用于请求明细区块
- 后端默认 limit=100 与 webui 默认 50 的不一致保留：webui 总是显式传 `limit`，不依赖后端默认值

## Testing Decisions

- 只测外部行为：分页控件渲染、翻页/跳页触发正确的 `/stats` 调用（`page`/`limit` 实参）、超页回退、档位切换重置页、空数据不渲染控件、窗口切换回第 1 页
- 被测模块：StatsPanel（webui vitest）；后端无改动不需新增 Rust 测试
- Prior art：既有 StatsPanel 测试已用 mock 断言调用实参（窗口边界测试）与 fake-timer 测自动刷新，翻页断言沿用同一模式；后端 `aggregate_paged` 的 page/limit 切片逻辑已有 Rust 测试覆盖，本次仅消费其结果

## Out of Scope

- 按 provider/model/成功与否过滤请求明细（独立筛选特性）
- By conversation 区块的 20 条截断（既有决策）
- 请求日志聚合性能优化（每次统计全量解析 `requests.log`）
- Export JSON / CSV（已导出全量历史，不改）
- 分页状态与 URL 同步（浏览器前进/后退、深链）

## Further Notes

- 既有先例：后端分页（`page`/`limit`/`recentRequestTotal`）与请求明细折叠已由 cost-stats feature 的 issue 06（`.scratch/cost-stats/issues/06-request-details-collapse-pagination.md`）实现并提交（`4b90c1d`/`f07fe1f` 等）；本 spec 是其遗留部分——请求明细分页 UI——的完整规格，并新增"每页条数四档"决策
- 现有 StatsPanel 测试断言 `api.stats` 调用为 4 参数（range, from, to），引入分页后实参变为 6 个（含 page/limit），既有断言需同步更新
- 两种情况不渲染分页控件：窗口无请求（totalRequests=0）与请求明细数据 absent（旧后端兼容）
- 与既有 ADR 无冲突（本次不触碰会话边界、token 口径等既定决策）
