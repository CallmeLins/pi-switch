# 04 — Stats 页布局与对话名称显示

**What to build:** Stats 页按新布局展示请求明细与对话聚合，并显示可读的对话名称。端到端行为：**单次请求**明细区块位于对话聚合区块上方；对话聚合区块默认折叠（组件内状态，不持久化），点击标题可展开/收起；对话行标题在有名称时显示名称、无名称时回退显示截断 ID，悬停行显示完整对话 ID。旧数据（无名称的对话）照常渲染。

**Blocked by:** 03 — 统计聚合携带对话名称

**Status:** resolved

- [x] 请求明细区块渲染在对话聚合区块上方
- [x] 对话聚合区块默认折叠，点击标题行可展开与收起，状态不持久化（刷新后回到默认）
- [x] 对话行有名称时显示名称，无名称时显示截断 ID（既有格式）
- [x] 有名称的对话行悬停显示完整对话 ID
- [x] 无名称对话（旧数据）正常渲染，不报错
- [x] 测试用 vitest 执行并全绿

## 实施总结
- 提交：`3fa5779` — feat: surface conversation display names in stats (inject, proxy, aggregate, webui)
- 实现的 seams：S10 请求明细 Card 移动到对话聚合 Card 上方；S11 对话聚合标题改为可点击 button（`aria-expanded` + ▸/▾ 指示），`conversationsOpen` 组件内 state 默认 false（默认折叠、不持久化）；S12 对话行 `title={conversationId}` + `name || shortConversationId(conversationId)` 显示
- `types.ts` `ConversationStats` 新增可选 `name?: string`
- 既有测试适配：5 处断言对话行的测试在断言前先点击标题展开（默认折叠生效）
- 测试结果：vitest 64/64 全绿（StatsPanel 32，含新增折叠切换与名称显示测试）；webui tsc 0 错误
