# Stats 页对话显示名称与布局调整（Conversation Name）

Status: ready-for-agent

## Problem Statement

Stats 页的**对话（Conversation）**聚合行只显示不可读的截断 UUID（pi 注入的 Session UUIDv7），用户无法辨认每次对话对应哪次 pi 会话；且对话聚合区块排在请求明细上方、常显占屏，与"先看请求明细、再看对话汇总"的浏览习惯相反。用户需要：请求明细在上、对话聚合在下且默认折叠、对话行显示可读名称。

## Solution

pi 扩展在注入 `x-conversation-id`（对话边界，不变）的同时注入 `x-conversation-name`（当前 pi 会话的自定义显示名，无名称时不注入）；代理把名称记入**请求日志（Request Log）**，统计聚合时随**对话（Conversation）**携带（取该对话最新一条日志的名称）；Stats 页调整布局——**单次请求**明细区块移到对话聚合上方，对话聚合区块默认折叠、可展开，行标题显示名称（无名称回退截断 ID），悬停显示完整 ID。

## User Stories

1. As a pi 用户, I want the request details table above the conversation list, so that I see individual requests first and per-dialogue rollups below
2. As a pi 用户, I want the conversation section collapsed by default, so that the stats page shows request details without a tall dialogue list pushing them down
3. As a pi 用户, I want to expand the conversation section with one click, so that I can still inspect per-dialogue tokens and cost
4. As a pi 用户, I want each conversation row to show my pi session's display name, so that I can tell which dialogue burned how many tokens without decoding a UUID
5. As a pi 用户, I want conversations without a custom session name to keep showing the short id, so that the row stays readable either way
6. As a pi 用户, I want to hover a named conversation to see its full conversation id, so that I can still correlate it with logs/exports
7. As a pi 用户, I want the display name to not affect conversation grouping, so that ADR-0002's identity rules stay untouched
8. As a pi 用户, I want the expand/collapse state to reset on refresh and window changes, so that behavior is predictable without persistence
9. As a pi 用户, I want the injected name header to be harmless to upstream providers, so that direct-to-provider requests keep working
10. As a pi 用户, I want old log rows without a name to keep working, so that historical data never breaks the page

## Implementation Decisions

- **注入契约**：扩展在 `before_provider_headers` 同时注入 `x-conversation-id`（既有，对话边界）与新增 `x-conversation-name`；名称取 `sessionManager.getSessionName()`，为空/空白时不注入且不动既有同名头；有值时覆盖既有同名头（与 id 的覆盖语义一致）。纯函数边界保持：id 与 name 注入均为纯函数，会话信息提供方接口扩展为同时暴露 id 与 name
- **header 契约**：新增请求头 `x-conversation-name`；仅 header 来源，无 body 回退（body 无对应字段），不参与对话边界识别（ADR-0002 三源识别不变）
- **日志契约**：请求日志新增可选字段 `conversationName`，旧行缺失 → null，向后兼容
- **聚合语义**：对话聚合新增名称字段，取该对话窗口内最新一条日志（按时间戳）的名称；全窗口无名称 → null
- **UI 布局**：请求明细区块移至对话聚合区块上方；对话区块默认折叠（组件内 state，不持久化），标题行可点击展开/收起；行标题显示名称（有名称显示名称、无名称回退截断 ID），悬停 title 显示完整对话 ID
- **范围纪律**：CSV 导出不加名称列；请求明细表不显示对话 ID/名称；对话聚合的排序规则（按最近活跃倒序、截断 20 条）不变

## Testing Decisions

- 好的测试 = 只测外部行为：扩展测注入后的 headers、代理测日志 JSON、聚合测返回的统计数据、UI 测渲染结果，均不依赖内部实现
- **扩展（node --test 既有先例）**：注入名称并覆盖既有值；名称为空/空白不注入且保留原值；id 与 name 独立生效；不污染调用方对象
- **代理（cargo test 既有先例）**：`x-conversation-name` 探测（非空取值、空值忽略、无该头 → None）；日志条目含 `conversationName`
- **统计聚合（cargo test 既有先例）**：同一对话多行取最新名称；无名称 → null；旧日志（无字段）兼容不 panic；名称不影响对话分组键
- **WebUI（vitest 既有先例）**：区块顺序断言（请求明细在对话上方）；对话区块默认折叠、点击展开/收起；有名称显示名称、无名称显示截断 ID、悬停 title 为完整 ID

## Out of Scope

- CSV 导出增加名称列
- 请求明细表显示对话 ID / 名称
- 折叠状态的 localStorage 持久化
- 名称参与对话边界识别（ADR-0002 三源不变）
- 对话聚合排序规则与条数上限调整
- unlabeled 组显示名称

## Further Notes

- 落地 conversation-id-inject spec 中 Out of Scope 的"对话显示名可读化（注入可读名称）"路径；名称只是显示属性，对话边界仍由 ID 决定
- 术语对齐：CONTEXT.md 的"对话（Conversation）"；本 feature 中的"会话名称"指 pi 侧 Session 的自定义显示名（`getSessionName()`），经注入后成为对话行的显示名
- seams：核心为统计聚合（Rust）——对话名称数据出口；扩展注入（TS 纯函数）与 Stats 页渲染（React）为既有测试 seam，无新建 seam
