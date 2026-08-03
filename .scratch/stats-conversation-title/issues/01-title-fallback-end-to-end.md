# 01 — 未命名会话回退首条消息标题（端到端）

**What to build:** 扩展在无 `/name` 显式名时，从 session entries 提取第一条非空 user message 文本（sanitize + 截断 60 字符）注入 `x-conversation-name`；代理 `conversation_name_of` 防御性清理控制字符。端到端行为：未命名会话的对话聚合行显示可读的首条消息标题（截断）；显式名仍优先；无 user message 的会话显示截断 UUID。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 纯函数：显式名优先；无显式名回退首条非空 user message 文本；首条为空取下一条
- [x] 纯函数：content 为 string 与 blocks 数组（只拼 text 块）两种形态均支持
- [x] 纯函数：控制字符（\x00-\x1f、\x7f）清理为单行、空白标题不注入、60 字符截断
- [x] handler：mock provider（getSessionName 有/无值 × getEntries 有/无首条消息）注入 `x-conversation-name` 正确；既有 9 个测试用例保持全绿（21/21）
- [x] 代理：`conversation_name_of` 清理 `\t`/换行、空值忽略、无 header → None；既有测试不受影响（136 全绿）
- [x] `node --test "extensions/**/*.test.ts"` 与 `cargo test --lib` 全绿；webui vitest 不回归（64/64）
- [x] 聚合/UI 零改动：stats.rs 最新名语义与 StatsPanel `name || shortConversationId` 不变

## 实施总结
- 提交：`dd1490b` — fix(extensions): fall back conversation title to first user message
- 实现的 seams：S1 扩展纯函数 `firstUserMessageText`（第一条非空 user message；string/blocks 两形态，只拼 text 块）+ `resolveSessionName`（显式名优先；回退 sanitize `[\x00-\x1f\x7f]+→空格` + `slice(0, 60)`，`TITLE_MAX_LEN = 60`）；S2 handler 改为 `resolveSessionName(name, ctx.sessionManager.getEntries())`，`SessionIdProvider` 新增 `getEntries()`，`SessionEntry` 宽松类型（不硬依赖 pi 内部 AgentMessage）；S3 代理 `conversation_name_of` 追加 `replace(['\r','\n','\t'], " ")` + `trim()` 纵深防御
- 测试结果：`node --test` extensions 21/21（新增 10 个：纯函数 7 + handler 集成 3，含既有 mock 适配 getEntries）；`cargo test --lib` 136 全绿（新增 conversation_name 控制字符清理）；webui vitest 64/64 不回归
- 遗留 / 后续建议：compaction 后首条 user message 被 summary 替代时无回退标题（显示 UUID，spec 显式决策）；截断按 UTF-16 code unit（emoji 可能截半，仅显示用途可接受）
