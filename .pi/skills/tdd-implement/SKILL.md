---
name: tdd-implement
description: "Implement from spec/ticket via strict TDD red-green loop, then typecheck, review, commit, update the issue status, and write an implementation summary."
---

# TDD Implement

整合 **implement** + **tdd** 的完整实现流程：每个 seam 一个红-绿循环，直到 commit。TDD 语义（红-绿循环、seam 定义、好测试标准）以 [tdd 技能](.agents/skills/tdd/SKILL.md) 为唯一事实源——测试标准详见 [tdd/tests.md](.agents/skills/tdd/tests.md)，Mock 边界见 [tdd/mocking.md](.agents/skills/tdd/mocking.md)；本技能只编排阶段与运行时规则。

本技能是**长程任务**（Long-Horizon Skill）：多阶段串行执行，自带**回合连续性**（Turn Continuity）与**任务分解**（Chunking）规则（见 [stages.md](references/stages.md) 阶段③ 3e/3f）。术语定义见 `CONTEXT.md`，技能设计规则见 `docs/agents/skill-design.md`。

## 流程速览

```
① 理解需求 → ② 确认 Seams → ③ TDD 开发循环 → ④ 完整测试套件 → ⑤ Code Review → ⑥ Commit → ⑦ 收尾（文档对齐 + issue 状态 + 实施总结）
```

每阶段的入口条件、操作与边界规则见 [`stages.md`](references/stages.md)——进入任一阶段前先读取该阶段的定义。③ TDD 开发循环的红-绿规则见 [tdd 技能](.agents/skills/tdd/SKILL.md)，不在本文件重写。

阶段要点：
- ⑤ Code Review：审查结果只在对话输出，不生成书面审查报告（不落盘 `review-*.md` 类文件）
- ⑦ 收尾：先对齐文档——检查 README 与 docs/ 中涉及本次实现的描述与实现是否一致，不一致则更新并 commit；再更新 issue 状态（有关联 issue 时，其验收标准逐条转写为 checkbox 清单并打勾——全部 `- [x]` 才允许标 `resolved`）；最后保持目录卫生（清理 `[DEBUG-...]` 调试残留与临时产物，`git status` 确认工作区干净）

## 回合连续性规则

每个逻辑单元（一次红-绿循环、一次 typecheck、一次测试失败修复）必须**在一个回合内连续执行完毕后才输出**：测试 → 分析失败 → 修正 → 重跑 → 全绿 整条链一气呵成，中间不停顿、不等用户说“继续”。

输出只允许发生在以下三种情况：
- **合规交互点**：技能要求的用户确认（如阶段② seams 清单确认）——此时提问并等待
- **外部阻塞**：权限拒绝、缺失授权、依赖不可用——此时明确说明需要什么授权或替代路径，不静默停止
- **阶段完成**：整个阶段的出口条件满足（如阶段③的所有 seams 红-绿完成 + typecheck 通过、commit 完成）——单个 seam 全绿只是阶段③的内部步骤，不是回合终点

预告下一步后立即执行该步骤，回合终点仅为合规交互点、外部阻塞或阶段出口条件满足。输出进度/预告本身不结束回合——输出后继续执行，直到三类终点之一达成。

## 不做什么

- 不重写 TDD 语义：红-绿循环、seam 定义、好测试标准、mocking 边界一律查 [tdd 技能](.agents/skills/tdd/SKILL.md)，本技能只编排阶段与运行时规则
- 不把重构塞进红-绿循环：重构归阶段⑤ Code Review
- 不在阶段间停顿：单 seam 全绿、单次 typecheck 通过都不是回合终点（见回合连续性规则）
- 不生成书面审查报告：阶段⑤审查结果只在对话输出，不落盘 `review-*.md` 类文件
- 不手写超大改动：单次 `write` 超 ~150 行先写骨架再分批；批量 `replace` 超 ~5 处分批执行（见 [stages.md](references/stages.md) 3f）
- 不跳步：阶段出口未达成不进入下一阶段（见路由规则）

## 任务拆分与 Todo 规定

### 拆分层级（大小任务层次）

1. **大任务**：Goal/Ticket——整个实现单元，对应一次完整的 tdd-implement 流程
2. **中任务**：Seam（阶段②确认）——一个红-绿循环单元，每 seam 一个 Todo
3. **小任务**：Todo——seam 内可独立验证、可勾选的执行单元（T1/T2/T3…）
4. **执行步**：Subtodo——Todo 内的串行步骤（红 → 绿 → typecheck），回合内逐步勾选推进

### Todo 清单格式
阶段② seams 确认后立即生成 todo 清单，每个 seam 一个 todo：
- 编号：`T1`、`T2`、`T3`…
- 描述：seam 名称 + 输入 + 预期输出
- 状态：`pending` / `in-progress` / `done` / `blocked`
- 完成标准（DoD）：该 seam 测试全绿 + typecheck 通过 + 既有测试不受影响
- 执行步（Subtodo）：`T1-R` 红（写失败测试）→ `T1-G` 绿（最小实现）→ `T1-T` typecheck

### Todo 状态机
```
pending → in-progress → done
                ↘ blocked（外部阻塞）→（授权/替代路径）→ in-progress
```
- Subtodo 不单独设 `blocked`——阻塞状态归父 Todo，Subtodo 跟随父状态

### 粒度与回合归属
- 一个 todo = 一个 seam 的红-绿 cycle + typecheck，不可再拆
- 一个 todo 必须在一个回合内完成（红→绿→typecheck→全绿）
- Subtodo 是 todo 内的执行步：每完成一步立即进入下一步（`T1-R` → `T1-G` → `T1-T`），禁止停在步间预告
- 每完成一个 todo 立即更新其状态，再进入下一个
- todo 状态只按实际推进更新（pending → in-progress → done），不基于旧快照重写整个清单；已完成项（done）永不回退
- 全部 todo 为 done 才进入阶段④

### 阻塞处理
- 外部阻塞（权限拒绝、缺失授权、依赖不可用）→ 标记 `blocked`，记录所需授权或替代路径
- 不静默停止；恢复后回到 `in-progress` 继续

## 路由规则

### 正常流转

| 当前阶段 | 出口条件 | 下一阶段 |
|----------|----------|----------|
| ① 理解需求 | 需求已澄清，无歧义 | → ② 确认 Seams |
| ② 确认 Seams | 用户确认 seams 清单 | → ③ TDD 开发 |
| ③ TDD 开发 | 所有 seams 红-绿完成，typecheck 通过 | → ④ 完整测试套件 |
| ④ 完整测试套件 | 全部测试通过 | → ⑤ Code Review |
| ⑤ Code Review | 审查通过 | → ⑥ Commit |
| ⑥ Commit | commit 完成 | → ⑦ 收尾 |
| ⑦ 收尾 | issue 状态已更新 + 实施总结已写 | ✅ 结束 |

### 回退路由

| 当前阶段 | 回退条件 | 回退目标 |
|----------|----------|----------|
| ③ TDD 开发 | typecheck 失败 | → ③ 修复类型错误 |
| ④ 完整测试套件 | 测试失败 | → ③ 修复失败测试 |
| ⑤ Code Review | 实现错误 | → ③ 修复实现 |
| ⑤ Code Review | seams 遗漏 | → ② 补充 seams |
| ⑤ Code Review | 需求偏差 | → ① 澄清需求 |

## 引用

- TDD 核心规则：[tdd 技能](.agents/skills/tdd/SKILL.md)
- 测试标准：[tdd/tests.md](.agents/skills/tdd/tests.md)
- Mock 指南：[tdd/mocking.md](.agents/skills/tdd/mocking.md)
- Issue tracker 约定：[issue-tracker.md](../../docs/agents/issue-tracker.md)
