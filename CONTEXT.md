# pi-switch Context

pi-switch 是 pi 客户端（Perplexity 的模型客户端）的轻量 profile 切换器：管理多个 provider profile，并提供一个本地代理，把客户端请求路由到当前 profile 的上游服务，支持同模型 failover 与 OpenAI/Anthropic 格式互转。

## Language

**Token 使用量（Token Usage）**：
单次请求消耗的 token 数，分四部分：输入（prompt）、输出（completion）、命中缓存输入（cached）、推理（reasoning）。由上游响应中的 usage 数据解析得出；取不到时该请求记为 unknown。
_Avoid_: 用量、消耗、费用

**消费（Cost）**：
由 token 使用量乘以模型单价折算的估算费用。单价按**每 1M tokens** 计（行业惯例，如 `input: 2.0` 表示 $2 / 1M tokens），来自 profile 模型配置（input/output/cacheRead/cacheWrite，可分级），在请求完成时定格并写入请求日志；模型未配置单价时该请求的消费记为 unknown。
_Avoid_: 费用、花费、金额、账单
**推理 token（Reasoning Tokens）**：
输出 token 中用于模型推理思考的部分，取自 `completion_tokens_details.reasoning_tokens`（Chat Completions / DeepSeek）或 `output_tokens_details.reasoning_tokens`（Responses）；是输出 token 的子集，总数不重复累加。上游不报告的（如 Anthropic）记 0。
_Avoid_: 思考 token、思维链 token

**统计窗口（Stats Window）**：
统计页的时间过滤范围。预设四种：当天（本地时区自然日）、24 小时以内（滚动）、7 天以内（滚动）、自定义日期区间（`[起日 0 点, 止日 24 点)`）。窗口作用于整个统计页的所有聚合。
_Avoid_: 时间段、筛选、时间范围

**缓存命中率（Cache Hit Rate）**：
命中缓存的输入 token ÷ 总输入 token，以百分比显示。分母只含输入 token，不含输出。
_Avoid_: 缓存率百分比、token 节省率

**对话（Conversation）**：
由客户端在请求中携带的会话标识（`x-conversation-id` 请求头、`x-opencode-session` 请求头（pi / open-code 客户端发送）或 body 的 `conversation_id` 字段）标识的一组请求，按标识聚合成一次对话的 token 统计。
_Avoid_: 会话、thread、session

**未标记（Unlabeled）**：
没有携带会话标识的请求，统计时归入名为 `unlabeled` 的单一组。
_Avoid_: unknown、无会话

**请求日志（Request Log）**：
`requests.log`，每行一个 JSON 的追加式文件，记录每次代理请求的元数据（时间、成败、provider、model、延迟、token 使用量、消费、会话标识）。
_Avoid_: 日志文件、usage log

**请求明细（Request Details）**：
Stats 页中按时间倒序逐条展示请求日志记录的表格区块，分页浏览，可覆盖窗口内全部历史。
_Avoid_: 请求记录、明细表、recent requests
