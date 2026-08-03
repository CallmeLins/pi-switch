# 03 — webui 时间选择器

**What to build:** 统计页顶部的时间范围选择器：4 个预设（当天/24 小时以内/7 天以内/自定义）+ 自定义时两个日期输入（起止各选一天）。切换即按本地时区把窗口算成 `from`/`to` 毫秒请求 `/stats` 并整体重渲染。默认当天。窗口语义：当天 = 本地自然日 0 点起；24h/7 天 = 滚动；自定义 = 起日 0 点至止日 24 点。当前选中项高亮，自定义日期可清空/重置回预设。

**Blocked by:** 02 — 时间窗口后端（/stats 参数已生效）

**Status:** resolved

## 实施总结
- 提交：`70a6f18` — feat: add time-range picker to usage stats web UI
- 实现的 seams：
  - `computeStatsWindow`（`webui/src/lib/statsWindow.ts`）：today 本地自然日 0 点 / 24h、7d 滚动 / custom 起日 0 点至止日 24 点（`new Date(y, m-1, d+1)`，DST 安全）；`now` 可注入；custom 缺日期抛错
  - StatsPanel 选择器渲染：4 个预设按钮（Today/24h/7d/Custom），默认 Today 高亮（aria-pressed），仅 custom 模式显示 From/To 日期输入
  - StatsPanel 切换行为：任何切换（含初始加载、Refresh）都携带 `range`+`from`/`to` 毫秒请求 `/stats`；`load` 带序号守卫防乱序响应；返回新数据后整体重渲染
  - 自定义校验：止早于起 → 不请求 + "End must be on or after start"；清空任一日期 → 不请求 + "Select both start and end dates"；重进 custom 模式时残留无效窗口不发起请求
  - `api.stats` 签名扩展为 `(range, from, to)`，`StatsRange` 类型从 `lib/statsWindow` 单源导入
- 测试结果：webui 31/31 全绿（statsWindow 7 + StatsPanel 13 + 既有 11）；Rust 105/105 不受影响
- typecheck：通过（webui `tsc --noEmit`）
- 遗留 / 后续建议：
  - vitest 全局固定 TZ=America/New_York（vite.config.ts），DST 用例（2026-03-08 spring-forward）依赖此配置
  - 后端 `/stats` 的 `range` 参数要求必带 `from`/`to`（issue 02 约束），前端恒带三者

- [x] 选择器渲染 4 个预设按钮与自定义日期输入，默认选中"当天"
- [x] 切换预设/自定义后请求携带正确窗口参数，页面整体重渲染
- [x] 当天按本地自然日、24h/7 天按滚动窗口计算（本地时区）
- [x] 自定义范围 `[起日 0 点, 止日 24 点)`，起止日期校验（止不早于起）
- [x] vitest 用例覆盖选择器渲染、切换与请求参数

> 核对（issue 05）：checklist 在收尾票执行时补勾——实施总结与测试（statsWindow 7 + StatsPanel 13 用例，vitest 31/31）已验证全部验收项，原未勾选为遗漏。
