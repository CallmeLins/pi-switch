# 02 — 时间窗口后端（聚合过滤 + API 参数）

**What to build:** `/stats` 支持时间窗口查询：`range=today|last24h|last7d|custom` 与 `from`/`to`（毫秒，左闭右开，custom 时必填）。窗口作用于全部聚合（请求数/成功率/延迟/by-provider/by-model/总 token/缓存命中率/对话列表），后端不做时区计算（窗口由调用方算好传入）。`ts` 缺失或解析失败的日志行视为窗口外。不带参数时行为与现在完全一致（全量）。

**Blocked by:** None — can start immediately

**Status:** resolved (commit 93158e3, 2026-08-02)

- [x] 聚合函数接受时间窗口参数，窗口内/外/边界（from 含、to 不含）的 entry 过滤正确
- [x] `ts` 缺失或不可解析的 entry 在窗口过滤时被排除；无窗口参数时全量行为不变（现有用例全绿）
- [x] `/stats` 解析 `range`/`from`/`to` 查询参数，custom 缺参时优雅拒绝（非 500）
- [x] 窗口内全部聚合维度（含 token 四维度与对话列表）一致重算
- [x] `cargo test --lib` 全绿（105 passed）

## 实施总结
- 提交：`93158e3` — feat: support time-window queries in usage stats
- 实现的 seams：
  - `stats::aggregate` 签名扩展 `window: Option<(u64, u64)>`（from 含、to 不含）；窗口内 entry 参与全部聚合维度
  - `stats::ts_epoch_ms` / `stats::in_window`：RFC3339 → epoch 毫秒，ts 缺失/不可解析视为窗口外
  - `stats::parse_window_query`：`range`/`from`/`to` 解析；custom 缺参、无效 range、非数字、from>=to 均 Err
  - `stats::get_stats` / `service::stats_value` 透传窗口；`web.rs` 的 `get_stats` 解析 query，Err → 400
  - lib.rs napi / TUI 保持全量（传 `None`）
- 测试结果：`cargo test` 105 passed 全绿（新增 11 个：窗口过滤边界、脏 ts 排除、全维度重算、parse_window_query 各分支、web 集成测试验证 400 而非 500）
- typecheck：通过（cargo test 编译）
- 遗留 / 后续建议：
  - stats.rs 内含 issue 01（reasoning token）的字段声明与聚合部分，与窗口过滤在同一函数内无法行级分离，经确认随本次提交一并进入；issue 01 的其余改动（usage.rs / proxy.rs / pages.rs）仍留在工作区未提交
  - `/stats` 预设 range（today/last24h/last7d）同样要求 from/to 齐全（后端不做时区计算，窗口由前端算好传入）
