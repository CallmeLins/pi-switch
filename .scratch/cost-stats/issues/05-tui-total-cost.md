# 05 — TUI 总消费行

**What to build:** TUI Stats 页 Overview 追加总消费行。端到端行为：在终端统计页看到当前统计窗口内的总消费（$ 前缀自适应格式）；全部消费 unknown 时显示 `-`；中英文界面词条同步。

**Blocked by:** 02 — 统计聚合与导出扩展

**Status:** resolved

- [x] Stats 页 Overview 显示总消费行（与现有 Tokens / Cache 行并列）
- [x] 全 unknown（无任何消费数据）时显示 `-` 而非 $0.00
- [x] 金额格式化与 WebUI 同规则（$ 前缀自适应）
- [x] i18n 词条中英文同步
- [x] 相关单测全绿（渲染、全 unknown 显示 `-`、格式化）

## 实施总结
- 提交：`f07fe1f` — feat: add cost tracking to request logs, stats aggregation and dashboards
- 实现的 seams：S12 TUI Stats Overview 总消费行（format_cost：$0.00 / 4 位小数去尾零 / 两位小数 / K-M 缩写；全 unknown → `-`）＋ i18n 词条 `stats_cost`（Cost / 消费）
- 测试结果：Rust 129 全绿（format_cost 精度与 `-` 用例）
- typecheck：通过（cargo check --lib）
- 遗留 / 后续建议：format 规则与 webui formatCost 双端重复（Rust/TS 无法共享），已用注释声明同规则，后续如调整需双端同步
