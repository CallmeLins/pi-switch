# 对话边界由客户端携带的会话标识决定，代理不自行发明

代理本身无状态，请求之间没有天然关联。决定："一次对话"由客户端请求携带的标识界定，识别顺序固定为三源（与 CONTEXT.md 的「对话（Conversation）」定义一致）：

1. `x-conversation-id` 请求头（最高优先级；pi 扩展 `conversation-id-inject` 主动注入当前 Session UUID）
2. `x-opencode-session` 请求头（pi / open-code 客户端自带）
3. body `conversation_id` 字段（兜底）

空值或非字符串忽略。三源均缺失的请求降级为单请求统计并归入 `unlabeled`。不做时间窗口等启发式分组——并行对话会互相污染，不可靠。
