# 03 — WebUI 消费展示

**What to build:** 统计页展示消费：顶部消费卡片（总消费 + 有 unknown 时的提示）、对话列表消费列、请求明细消费列。端到端行为：打开统计页即可看到当前统计窗口内的总消费、每个对话与每条请求的消费；金额按 `$` 前缀自适应格式显示（$0.00 / $0.0042 / $12.34 / $1.2K）；消费 unknown 显示 `-`。

**Blocked by:** 02 — 统计聚合与导出扩展

**Status:** resolved

- [x] 顶部消费卡片显示总消费；存在 unknown 行时显示 unknown 计数提示
- [x] 对话列表每行显示消费列（unknown 显示 `-`）
- [x] 请求明细每行显示消费列（unknown 显示 `-`）
- [x] 金额格式化：$0.00 / $1 以下 4 位有效小数 / $1 以上两位小数 / 大额 K/M 缩写 / `-`
- [x] 全 unknown（无任何消费数据）时总消费显示 `-` 而非 $0.00
- [x] 与现有 Tokens/Cache 卡片及统计窗口选择器并存不回归
- [x] 相关前端测试全绿（卡片/列渲染、格式化规则）

## 实施总结
- 提交：`f07fe1f` — feat: add cost tracking to request logs, stats aggregation and dashboards
- 实现的 seams：S8 formatCost（$0.00 / 1 以下 4 位小数去尾零 / 两位小数 / K-M 缩写 / `-`）、S9 顶部消费卡片（总消费 + unknown 提示，全 unknown 显示 `-`）、S10 对话列表与请求明细消费列（unknown `-`）
- 测试结果：webui 61 全绿（format.test.ts 25 ＋ StatsPanel.test.tsx 29 等）
- typecheck：通过（tsc --noEmit）
- 遗留 / 后续建议：999.999 四舍五入跨千时显示 `$1000.00` 而非 `$1.0K`（观感，双端一致）；unknown 提示文案为「N unknown cost rows」
