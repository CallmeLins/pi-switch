# 阶段详细定义

## 目录

- [阶段 ①：理解需求](#阶段-①理解需求)
- [阶段 ②：确认 Seams（测试接缝）](#阶段-②确认-seams测试接缝)
- [阶段 ③：TDD 开发循环](#阶段-③tdd-开发循环)
- [阶段 ④：完整测试套件](#阶段-④完整测试套件)
- [阶段 ⑤：Code Review](#阶段-⑤code-review)
- [阶段 ⑥：Commit](#阶段-⑥commit)
- [阶段 ⑦：收尾（文档对齐 + issue 状态 + 实施总结）](#阶段-⑦收尾文档对齐--issue-状态--实施总结)

## 阶段 ①：理解需求

### 入口条件
- 用户提供了 spec 或一组 ticket

### 操作
1. 完整读取 spec/ticket 内容
2. 若存在 `CONTEXT.md` 和 `docs/adr/`，先阅读，确保术语和 ADR 决策不被违背
3. 如有歧义，先向用户澄清再继续

### 出口条件
- 能用自己的话复述需求
- 无未澄清的歧义

### 边界
- 本阶段只澄清需求——实现与测试设计在后续阶段进行

---

## 阶段 ②：确认 Seams（测试接缝）

### 入口条件
- 需求已澄清，无歧义

### 操作
1. 列出所有将要测试的公共接口（seams）
2. 每个 seam 需包含：名称、输入、预期输出
3. 向用户展示 seams 清单并确认
4. 用户确认后才写任何测试代码
5. seams 确认后生成 todo 清单（每 seam 一个 todo，含编号/状态/DoD）——格式与状态机见 [SKILL.md「任务拆分与 Todo 规定」](../SKILL.md#任务拆分与-todo-规定)

### 出口条件
- 用户明确同意了 seams 清单

### 边界
- 一个 seam 对应一个公共接口上的一个待测行为（输入 + 预期输出）：一个 seam = 一个测试 + 一个最小实现 cycle；同一接口的多个行为拆分为多个 seam，而非内部函数

> Seams 定义参考：[tdd 技能](.agents/skills/tdd/SKILL.md#seams--where-tests-go)

---

## 阶段 ③：TDD 开发循环

### 入口条件
- Seams 已确认

### 操作

**红-绿循环前与循环中都查阅 tdd 技能各节**（Every section applies on every cycle）：TDD 语义与测试规则以 [tdd 技能](.agents/skills/tdd/SKILL.md) 为唯一事实源，不再在此重写——好测试标准见 [tdd/tests.md](.agents/skills/tdd/tests.md)，Mock 指南见 [tdd/mocking.md](.agents/skills/tdd/mocking.md)。
本阶段只执行编排：按阶段②生成的 todo 清单逐条推进（大小任务层次与 Subtodo 格式见 [SKILL.md「任务拆分与 Todo 规定」](../SKILL.md#任务拆分与-todo-规定)），每完成一个 todo（红-绿 cycle + typecheck）立即更新其状态为 `done`，再进入下一个 todo。

#### 3a/3b. 红-绿（Red-Green）
红-绿循环的执行规则（Red before green、One slice at a time、Anti-patterns、垂直切片）以 tdd 技能为准，见 [tdd/SKILL.md](.agents/skills/tdd/SKILL.md) 与 [tdd/tests.md](.agents/skills/tdd/tests.md)。

#### 3c. 切换 seam
每完成一个 seam 立即进入下一个 seam，同一回合内串行推进，不等用户“继续”。

#### 3d. Typecheck
- 每个 cycle 结束后运行 typecheck
- 发现问题立即修复，修复后再继续

#### 3e. 回合连续性
- 每个红-绿 cycle 及其 typecheck 必须在一个回合内串行完成：测试 → 分析失败 → 修正 → 重跑 → 全绿，中途不输出、不停止、不等用户“继续”
- **单个 seam 全绿不是回合终点**：它只是阶段③的内部步骤；阶段③的出口是“所有 seams 红-绿完成 + typecheck 通过”，在出口达成前不停顿、不等待确认，直接进入下一个 seam
- 预告下一步后立即执行该步骤，回合终点仅为合规交互点、外部阻塞或阶段出口条件满足
- 进度输出并入工具调用序列，不单独结束回合——输出后继续执行，直到三类终点之一达成
- 输出只发生在：合规交互点（用户确认）、外部阻塞（明确说明所需授权或替代路径）、阶段出口条件满足时
- 外部阻塞（如权限拒绝）时明确请求授权或改用不冲突的路径，不静默等待

#### 3f. 任务分解（Chunking）
- 单次 `write` 超过 ~150 行：先写骨架再分批补全
- 批量 `replace` 超过 ~5 处：分批执行，每批后立即 typecheck 验证

#### 3g. Todo 更新纪律
- 每完成一个红-绿 cycle（含 typecheck），按实际推进更新对应 todo 状态：`in-progress` → `done`
- 更新基于当前实际状态，不基于旧快照重写整个清单；已完成项（done）永不回退

### 出口条件
- 所有 seams 的红-绿循环完成
- Typecheck 通过

### 边界
- 每个 cycle 后运行 typecheck
- 全部 todo 为 done 才进入阶段④
- 测试质量规则（公共接口验证、独立断言、mock 边界、重构归属 review）见 tdd 技能，不在本阶段重写

> Mock 指南：[tdd/mocking.md](.agents/skills/tdd/mocking.md)
> 好测试标准：[tdd/tests.md](.agents/skills/tdd/tests.md)

---

## 阶段 ④：完整测试套件

### 入口条件
- 阶段 ③ 完成，typecheck 通过

### 操作
1. 运行仓库的完整测试套件
2. 检查所有测试是否通过

### 出口条件
- 全部测试通过

### 边界
- 测试失败时回到阶段 ③ 修复，修复后重新运行完整套件——进入 review 前必须全绿

---

## 阶段 ⑤：Code Review

### 入口条件
- 完整测试套件通过

### 操作
1. 调用 `/code-review` skill 审查当前所有改动
2. 审查发现的问题按 [SKILL.md 回退路由](../SKILL.md#回退路由) 处理
### 出口条件
- Code review 通过

### 边界
- 重构在此阶段进行，而非 TDD 循环阶段
- review 通过后才进入 commit
- 审查结果只在对话输出，不生成书面审查报告（不落盘 `review-*.md` 类文件）

---

## 阶段 ⑥：Commit

### 入口条件
- Code review 通过

### 操作
1. 将工作提交到当前分支
2. 附带清晰的 commit message

### 出口条件
- Commit 完成

### 边界
- Commit message 描述变更内容而非过程

---

## 阶段 ⑦：收尾（文档对齐 + issue 状态 + 实施总结）

### 入口条件
- Commit 完成（阶段⑥出口）

### 操作
1. **对齐文档**：检查 README 与 `docs/` 中涉及本次实现的描述（用法、CLI、配置、示例、架构、行为）是否与实现一致；不一致则更新文档，并单独 commit（message 如 `docs: align README with <feature>`）
2. 若本次实现有关联 issue/ticket（`.scratch/<feature-slug>/issues/`）：先审查该 issue——从 issue 提取验收标准（无显式验收标准节时以其正文行为要求为准），逐条转写为 checkbox 清单并逐条验证：通过标 `- [x]`，未通过保留 `- [ ]` 并注明缺口（证据：文件:行号 / 测试名）。全部打勾后才允许下一步：
3. 将 `Status:` 行改为 `resolved`（无该行则追加），不改动 spec 与既有 Comments
4. 在 issue 文件底部追加实施总结（`## 实施总结` 标题）：

   ```
   ## 实施总结
   - 提交：`<commit hash>` — `<commit message>`
   - 实现的 seams：<清单>
   - 验收标准：逐条 `- [x]`（未全绿列出缺口）
   - 测试结果：<全绿 / 数量>
   - typecheck：通过
   - 文档对齐：<更新了哪些文件 / 无需更新>
   - 遗留 / 后续建议：<如有>
   ```

5. 无关联 issue（直接实现用户给的 spec）→ 跳过状态更新，将总结作为会话最终输出
6. **保持目录卫生**：清理本次实现产生的临时产物——`[DEBUG-...]` 标记的调试代码/日志、一次性脚本、临时文件与备份文件；用 `git status` 确认工作区只含预期改动，无残留未跟踪文件后才结束

### 出口条件
- 文档与实现对齐（无相关文档或已更新）
- issue 状态已更新（或确认无 issue）
- 实施总结已落盘 / 输出
- 工作区干净（临时产物已清理，`git status` 无残留未跟踪文件）

### 边界
- 只追加不改写：不修改 spec.md 与既有 Comments 内容
- 文档对齐仅限与本次实现直接相关的描述，不顺手重构无关文档
- 总结写事实（提交 / 测试 / 遗留），不写过程叙述
