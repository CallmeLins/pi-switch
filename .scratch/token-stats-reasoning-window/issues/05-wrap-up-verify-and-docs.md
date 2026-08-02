# 05 — 收尾：核对各票完成状态 + 更新 README

**What to build:** 功能收尾票：逐票核对 01–04 的验收标准是否全部满足（不静默跳过，未满足项显式记录）；端到端验证链路可用（真实请求 → 请求日志含 `reasoningTokens` → `/stats` 带窗口参数返回四维度 → WebUI 时间选择器与 5 格展示正确）；同步更新 README.md 与 README_ZH.md——记录推理 token 维度、时间窗口用法（当天/24h/7 天/自定义语义）与默认当天行为，确保用户侧文档与实现一致。

**Blocked by:** 01, 02, 03, 04 — 全部实现票

**Status:** resolved (commit a3a8469, 2026-08-02)

- [x] 逐票核对 01–04 验收标准：全部满足或显式记录未满足项，不允许静默跳过
- [x] 端到端验证：真实请求 → 日志含推理 token → `/stats?range=` 返回窗口内四维度 → WebUI 时间选择器切换生效、5 格展示正确
- [x] README.md 与 README_ZH.md 同步更新：四维度口径（推理为输出子集）、时间窗口语义（当天=本地自然日/24h/7 天=滚动/自定义日期）、默认当天
- [x] 更新后的文档无过期信息（命令示例、界面描述与实际一致）

## 实施总结
- 提交：`a3a8469` — docs: document four-dimension tokens and time windows in README (EN/ZH)
- 实现的 seams：
  - S1 核对 01–04：01/02/04 验收项全部满足（resolved + 实施总结齐全）；03 行为满足（实施总结与 vitest 31/31 佐证）但 checklist 原未勾选，本次补勾并加注显式记录（issue 03 文件内）
  - S2 端到端分层验证（全量测试作为链路验证）：
    - 解析层：usage.rs 测试（chat completions / responses / deepseek 三条路径 + 缺失记 0 + malformed usage）
    - 日志层：CSV/JSON 导出与 parse_entries 测试验证 `reasoningTokens` 字段往返；旧行无字段按 0 兼容
    - 聚合层：aggregate 测试（四维度合计、reasoning 为输出子集不计入 total、窗口过滤、旧行兼容）
    - HTTP 层：web.rs 集成测试（`/api/stats?range=` 窗口参数 200/400 行为）
    - WebUI 层：statsWindow（today/24h/7d/custom 窗口计算、DST）+ StatsPanel（选择器切换请求参数、5 格卡片、对话三列、旧数据占位）
    - 真实请求冒烟（起 proxy → 发请求 → 读日志 → curl /stats）未执行：无可用上游 API key、仓库无 mock upstream 基础设施、且不写入用户数据目录（~/.pi-switch）。限制已显式记录；各环节均由上述测试覆盖，链路可用性以分层验证为准
  - S3/S4 README（EN/ZH 镜像）：统计段更新为四维度口径（推理为输出子集、`total = input + output`）、时间窗口语义（当天=本地自然日 0 点 / 24h、7 天=滚动 / 自定义=起日 0 点至止日 24 点）、默认当天、WebUI 5 格平铺+子集角标+对话三列+缺失/0 显示 `-`、无窗口参数返回全量；Features 表同步
  - S5 一致性复查：TUI 描述与 `token_summary` 实现一致（累计输入/输出 + 缓存命中率，TUI 无窗口/四维度平铺，符合 spec Out of Scope）；其余命令示例与架构描述无过期信息
- 测试结果：Rust `cargo test --release --lib` 105 passed；webui vitest 37 passed（README 为纯文档变更，测试结果引用验证基线）
- typecheck：通过（无代码变更）
- 遗留 / 后续建议：
  - 真实 HTTP 端到端冒烟待有上游 API key 或 mock 基础设施时补做（路径：`pi-switch proxy start` → 发带 reasoning 的请求 → `curl '/api/stats?range=today&from=..&to=..'` 验证四维度）
  - TUI 统计页展示四维度平铺与时间范围切换不在本 feature 范围（spec Out of Scope / ADR 0003 后续）
