# 守则：Grill-to-Spec 产出物格式细则

三类产出物（Glossary / ADR / Spec）格式的**唯一细节出处**，由 SKILL.md 直链引用。SKILL.md 只保留流程、导航与不可协商规则，本文件不重复流程内容。

## Glossary 守则

- 懒创建：首个术语解析时才建 `CONTEXT.md`；多上下文时先确认归属，归属不清则询问
- 只是 glossary：零实现细节，不当 spec/scratch pad
- 只收本上下文特有术语，通用编程概念不收
- 定义 WHAT 非 HOW，1-2 句；opinionated，同义词列 `_Avoid_`；术语解析即 inline 更新，不批量

## ADR 守则

- 三条件全满足才提议（难逆转 / 无上下文费解 / 真实权衡）；`docs/adr/` 懒创建
- 格式：标题 + 1-3 句正文；可选节（Status/Considered Options/Consequences）按需，大多数不需要
- 编号：`0001-slug.md` 顺序递增，扫描最高号 +1
- 草稿经用户显式确认后落盘，任何情况无例外（见 SKILL.md 流程① 与不可协商规则）

## Spec 守则

- 完整七节模板逐节不缺：Problem Statement / Solution / User Stories / Implementation Decisions / Testing Decisions / Out of Scope / Further Notes
- User Stories：长编号列表，`As an <actor>, I want a <feature>, so that <benefit>` 格式
- Implementation Decisions：不含文件路径/代码片段；例外——原型产出的决策密集片段可 inline，注明来源并裁剪至决策部分
- 全文贯穿 glossary 词汇；尊重所触区域既有 ADR
- seams：既有优先于新建、取最高、理想数量 1，与用户确认
- 发布后标 `ready-for-agent`，triage 状态以 issue 文件顶部 `Status:` 行记录

## 反模式（不做什么）

- 不把守则当逐条朗读的检查清单——守则约束产出物格式，不约束对话节奏
- 不把 ADR 当 glossary 一样 inline 更新（见 SKILL.md 不可协商规则）
- 不产出守则之外的文件：产出物只有三种（Glossary / ADR / Spec）
- 不在本文件之外重复守则细节——SKILL.md 与 references 之间信息只在一处存在
