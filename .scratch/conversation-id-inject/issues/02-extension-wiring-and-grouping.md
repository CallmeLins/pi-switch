# 02 — 扩展接线与对话归组端到端生效

**What to build:** 注册 pi 扩展：在每次 provider 请求发出前，把当前对话的 Session UUID 注入 `x-conversation-id` 请求头，并在 pi 扩展登记中登记该模块。端到端行为：经 pi-switch 本地代理的真实 pi 会话中，每次请求携带当前对话的标识，统计页的"对话（Conversation）"聚合按 UUID 归组——`/resume` 同一对话保持同一标识、`/new` 开启新对话换新标识，请求不再落入"未标记（Unlabeled）"。

**Blocked by:** 01 — 对话标识注入纯函数与单元测试

**Status:** ready-for-agent

- [ ] pi 启动后扩展被加载（扩展登记生效），provider 请求携带 `x-conversation-id` = 当前 Session UUID
- [ ] 经 pi-switch 本地代理的会话中，统计的 byConversation 出现 UUID 标识的对话，而非全部归入 unlabeled
- [ ] `/resume` 同一 session 文件时标识保持不变，对话持续累计
- [ ] `/new` 开启新对话时标识更换，新请求归入新对话
- [ ] 无有效 Session UUID 的场景（如内存会话）不注入垃圾头，请求行为不受影响
- [ ] 人工端到端验收：真实对话产生请求后，`/piswitch stats` 可见对应 UUID 对话及其 token 统计
