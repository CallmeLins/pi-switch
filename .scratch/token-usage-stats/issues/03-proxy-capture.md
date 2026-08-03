# 03 — 代理采集：流式旁路解析 + 日志扩展

**What to build:** 代理在转发上游流式响应时，复制响应流：一份照常直通客户端（保持逐 token 体验），另一份喂给 02 的流解析器；流结束后若解析出 Token 使用量，把完整日志行异步补写进请求日志。同时：

- 请求日志行新增可选字段：`promptTokens` / `completionTokens` / `cachedTokens` / `conversationId`，旧行不受影响
- 对话标识提取：请求头 `x-conversation-id` 优先，body `conversation_id` 兜底，随行记录
- 非流式转换路径（OpenAI→Anthropic 转换）：响应已整体读入内存，直接提取后随日志行写出
- 流被客户端掐断或取不到 usage：仍写日志行，token 字段留空

**Blocked by:** 02 — Token 解析模块

**Status:** ready-for-agent

- [ ] 流式请求转发体验不变（逐 token 输出，无缓冲延迟）
- [ ] 流式请求结束后，请求日志行含 token 使用量与对话标识（若有）
- [ ] 转换路径请求的日志行同样含 token 使用量
- [ ] 中断/无 usage 请求：日志行正常写入，token 字段为空
- [ ] 旧日志行与新行共存时读取无异常
