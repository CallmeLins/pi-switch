# 03 — 收尾：构建验证与文档

**What to build:** 端到端验证：原生模块构建成功、WebUI 构建成功，统计页在真实代理日志下可见请求明细与对话补齐三项；README（EN/ZH）按既有风格补充本 feature 的说明；发布前完成审计复核。

**Blocked by:** 01 — 后端：聚合层产出请求明细与对话缓存率；02 — WebUI：请求明细表格与对话卡片补齐

**Status:** resolved (commit b4a94e3, 2026-08-02)

- [x] 原生模块与 WebUI 均构建成功，无类型错误
- [x] 用真实 `requests.log`（含旧行与带 usage 的新行）人工验证：明细表按窗口过滤、倒序、100 条截断；无 usage 行显 `-`；对话行三项可见
- [x] README（EN/ZH）按既有风格补一句本 feature 的说明
- [x] 更新本 issue 状态为 resolved 并附实现摘要（沿用既有 issue 收尾模式）
- [x] 审计复核通过（完成后按既有惯例运行一次审计）

## 实施总结
- 提交：`b4a94e3` — docs: document request details table and conversation cache rate in README (EN/ZH)
- 实现的 seams：本 issue 为收尾验证与文档任务，无新代码 seam
- 验证结果：
  - 构建：`napi build --release` 成功；`vite build` 成功；`tsc --noEmit` 通过
  - 测试：`cargo test` 115 passed；`cargo clippy` 8 个既有警告（proxy.rs / tui / 旧逻辑，与 issue 01 记录一致，无新增）；vitest 47 passed
  - 人工验证（真实 `~/.pi-switch/requests.log`，21 行含 2 条旧行）：新构建 daemon 下 `/api/stats` 返回 `recentRequests` 21 行全部倒序；旧行 token 字段 null、cacheRate `-`；cached=0 行 `0.0%`；total = input + output；窄窗口 [01:55Z, 02:01Z) 过滤出 7 行；对话级 cacheRate（97.8% / 76.7%）；WebUI bundle 含 Request details 表格与对话行新列
- README：EN/ZH 各新增「Request details / 请求明细」条目，更新「WebUI dashboard / 面板」条目（对话行六项两行布局、cache hit rate 术语与既有文档一致）
- typecheck：通过（tsc --noEmit + cargo test/clippy 编译）
- Code Review：Standards 轴 4 处已修复（术语统一为 cache hit rate、EN 两行布局补 when over-wide 对齐 ZH、去重复枚举、zeroes→zeros）；Spec 轴无缺失、无 scope creep
- 遗留 / 后续建议：审计复核报告见 issue 04（audit-spec-issues-and-readme）
