# 04 — WebUI 实时刷新四档

**What to build:** 统计页新增实时刷新选择：Off / 5s / 30s / 5min 四档，默认 Off。端到端行为：用户选择档位后统计页按间隔自动重新拉取当前统计窗口的数据；切回 Off 停止轮询；组件卸载时定时器清理；自动刷新失败时保留现有数据不清空。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 四档选择（Off / 5s / 30s / 5min）渲染于统计页，默认 Off
- [x] 选择非 Off 档位即按间隔自动刷新，且沿用当前统计窗口（时间范围）参数
- [x] 切回 Off 停止轮询；切换档位重置计时
- [x] 组件卸载时定时器清理，无泄漏
- [x] 自动刷新失败保留现有数据（不清空页面），不打断用户操作
- [x] 与现有手动 Refresh 按钮并存（Off 档下手动刷新仍可用）
- [x] 相关前端测试全绿（档位切换、定时器启停、失败保留数据）

## 实施总结
- 提交：`f07fe1f` — feat: add cost tracking to request logs, stats aggregation and dashboards
- 实现的 seams：S11 刷新四档 Off/5s/30s/5min（默认 Off）；切换档位重建定时器（重置计时）；Off 停止轮询；卸载清理；失败保留现有数据；沿用当前统计窗口（range/from/to）参数；与手动 Refresh 并存
- 测试结果：webui 61 全绿（档位渲染、定时刷新、Off 停止、失败保留、卸载清理 5 项测试）
- typecheck：通过（tsc --noEmit）
- 遗留 / 后续建议：无
