# 02 — Token 解析模块（usage 提取 + SSE 流解析）

**What to build:** 新建纯函数模块，把上游响应解析成规范化的 Token 使用量摘要（输入/输出/命中缓存）。两部分能力：
1. 非流式提取：给定完整响应 JSON，按 Anthropic > OpenAI 标准 > DeepSeek 变体的顺序探测缓存字段（含 `cache_read_input_tokens`、`prompt_tokens_details.cached_tokens`、`prompt_cache_hit_tokens` 等），取第一个存在的
2. 流式解析：增量喂入 SSE 文本块（兼容任意 chunk 切分位置），识别 OpenAI 流（usage 帧在结束标记前）与 Anthropic 流（起始事件携带输入/缓存数据、增量事件携带输出累计），流末输出摘要；拿不到 usage 返回空

**Blocked by:** None — can start immediately

**Status:** ready-for-agent

- [ ] 非流式提取覆盖三种字段风格，探测顺序正确，只认第一个存在的
- [ ] 流式解析覆盖 OpenAI 与 Anthropic 格式；跨块任意切分（每次喂半帧）结果一致
- [ ] 无 usage 数据（厂商不报、流中断、缺失事件）→ 返回空，不 panic
- [ ] 全部为纯函数，无文件系统/网络副作用
- [ ] 单测全绿（缓存率分母口径：命中缓存输入 ÷ 总输入，输出 token 不参与）
