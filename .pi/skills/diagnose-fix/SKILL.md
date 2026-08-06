---
name: diagnose-fix
description: "Complete diagnosis→fix→regression channel for bugs: diagnose, then fix via a TDD red-green loop with a hard gate (no fix code before a failing regression test). Use when the user says diagnose/debug/fix this, or reports something broken/throwing/failing/slow — prefer this over diagnosing-bugs when a fix is wanted, not just a diagnosis."
---

# Diagnose Fix

诊断 **bug** 并修复的编排技能：诊断语义以 [diagnosing-bugs](.agents/skills/diagnosing-bugs/SKILL.md) 为唯一事实源，修复语义以 [tdd 技能](.agents/skills/tdd/SKILL.md) 为唯一事实源；本技能只编排三个阶段并设一道**硬门槛**，不重写两个上游技能的规则。

本技能是**长程任务**（Long-Horizon Skill）：诊断 → 修复 → 回归在**一个回合内串行完成**，自带**回合连续性**（Turn Continuity）规则（见下文）。术语定义见 `CONTEXT.md`，技能设计规则见 `docs/agents/skill-design.md`。

## 流程速览

```
① 诊断 → ② TDD 修复（硬门槛）→ ③ 回归验证
```

## ① 诊断

按 [diagnosing-bugs](.agents/skills/diagnosing-bugs/SKILL.md) 的 Phase 1-4 执行：

1. **反馈回路**（Phase 1）：构建能对 _这个 bug_ 变红的紧致 pass/fail 信号——优先失败测试，其次 curl/CLI/浏览器脚本/重放/一次性 harness 等；没有回路不进入假设。
2. **复现 + 最小化**（Phase 2）：跑回路看它红，确认失败模式与用户描述一致；逐步删减输入/调用方/配置，只保留 load-bearing 元素。
3. **假设**（Phase 3）：生成 3-5 个可证伪的排名假设，先展示给用户。
4. **探针**（Phase 4）：一次只改一个变量，临时探针用 `[DEBUG-...]` 前缀标记。

**出口条件**：反馈回路已红（已实际跑过并确认捕捉到该 bug）、复现已最小化。诊断完成前不写任何修复代码。

## ② TDD 修复（硬门槛）

**TDD 语义（红-绿循环、seam 定义、好测试标准、anti-patterns）以 [tdd 技能](.agents/skills/tdd/SKILL.md) 为唯一事实源**——本技能不重写；进入本阶段前先读取 tdd 技能。

**硬门槛**：写任何修复代码之前，必须已存在一个**失败**的回归测试——把最小复现转写为正确 seam 上的测试，先运行看它红，然后才允许写修复代码让它变绿。

- **无逃生舱**：不存在正确 seam 时，**本身即 finding**——向用户明确说明"架构阻止锁定该 bug"，请求 seam 决策或记录为架构改进建议（可转交 `/improve-codebase-architecture`）；**不得**绕过测试直接改代码。
- **轻量声明**：本技能不套用 tdd-implement 的重流程——不做逐 todo 的循环编排、不设 seams 确认步骤、无 typecheck/commit 前置门禁；单 seam 修复场景直走红-绿。

**出口条件**：回归测试先红 → 写最小修复 → 回归测试变绿。

## ③ 回归验证

1. 重跑诊断阶段①的**原始反馈回路**（未最小化场景）确认症状消失。
2. 清理：删除所有 `[DEBUG-...]` 标记的临时探针与一次性 harness（`grep` 前缀确认无残留）。
3. 在 commit / PR 消息中写明**验证正确的假设**（诊断阶段哪个假设被证实），让下一个调试者受益。

**出口条件**：原始症状消失 + 回归测试绿 + 临时探针清理完成。

## 反模式（不做什么）

完整反模式清单见 [references/anti-patterns.md](references/anti-patterns.md)——正文各阶段规则是正面约束，反模式清单是负向边界；细节只在一处存在，本文件不重复。

## 回合连续性规则

诊断 → 修复 → 回归**在一个回合内串行完成**，不等用户"继续"：构建回路 → 复现 → 假设 → 探针 → 失败测试 → 修复 → 回归 → 清理整条链一气呵成，中途不停顿。

输出只允许发生在以下三种情况：
- **合规交互点**：技能要求的用户确认——阶段①假设清单展示、阶段②无 seam finding 上报或 seam 决策请求
- **外部阻塞**：权限拒绝、缺失授权、依赖不可用——明确说明所需授权或替代路径，不静默停止
- **阶段出口**：整个阶段的出口条件满足（阶段①回路已红 + 复现最小化；阶段②失败测试已红 → 修复变绿；阶段③症状消失 + 回归绿 + 清理完成）

预告下一步后立即执行该步骤，回合终点仅为合规交互点、外部阻塞或阶段出口条件满足。进度输出本身不结束回合——输出后继续执行，直到三类终点之一达成。

## 引用

- 诊断：[diagnosing-bugs](.agents/skills/diagnosing-bugs/SKILL.md)
- TDD 修复：[tdd 技能](.agents/skills/tdd/SKILL.md)、[tdd/tests.md](.agents/skills/tdd/tests.md)、[tdd/mocking.md](.agents/skills/tdd/mocking.md)
