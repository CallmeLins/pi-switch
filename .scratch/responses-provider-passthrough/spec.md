# Responses API Provider 透传与转换

Status: ready-for-agent

## Problem Statement

pi-switch 当前已有 `/v1/responses` 接口，但 streaming 与非 streaming 的行为不一致：

- 非 streaming 请求统一转换为 Chat Completions，再把响应转换回 Responses。
- streaming 请求只支持 `openai-responses` provider 原样转发。
- Chat Completions provider 无法处理 Responses streaming。
- provider 没有独立的 Responses 处理模式，用户无法在 WebUI 中查看或控制行为。
- tools、tool calls、reasoning、usage、错误响应和 failover 在不同路径上的行为不一致。
- 同一组候选 provider 无法按各自能力在透传与转换之间独立切换。

因此，原生 Responses provider 的能力可能被转换层丢失，非原生 provider 又无法完整接入 Responses API，用户也无法明确判断某个 provider 的实际处理方式。

## Solution

为每个 provider 增加 Responses 处理模式：

- `auto`：默认值。`openai-responses` provider 使用 Responses 透传；`openai-completions` provider 使用 Responses ↔ Chat Completions 转换。
- `passthrough`：仅允许原生 Responses provider，原样传递 Responses 请求、响应和 streaming events。
- `convert`：仅允许支持 Chat Completions 的 provider，通过现有转换架构处理 Responses 请求。

Responses handler 按候选 provider 自身的模式逐个处理请求，因此 failover 可以在透传 provider 与转换 provider 之间切换。代理可以旁路解析 routing、usage、reasoning、conversation 和 request log，但不得修改透传内容。

WebUI 在 provider 编辑表单中增加模式选择，在 provider 列表中显示实际生效模式。TUI/CLI 只保留和读取该配置，不新增交互。

## User Stories

1. 作为 pi 用户，我希望通过 `openai-responses` provider 调用 `/v1/responses`，以便保留 upstream 原生 Responses 能力。
2. 作为 pi 用户，我希望通过 `openai-completions` provider 调用 `/v1/responses`，以便让 Responses 客户端继续使用已有 Chat Completions upstream。
3. 作为 provider 管理员，我希望为每个 provider 配置 Responses 模式，以便独立控制其路由行为。
4. 作为 provider 管理员，我希望旧配置缺失模式字段时自动按 `auto` 处理，以便现有 profile 无需迁移即可继续工作。
5. 作为 provider 管理员，我希望 `auto` 默认让原生 Responses provider 走透传，以便默认保留 upstream 能力。
6. 作为 provider 管理员，我希望 `auto` 默认让 Chat Completions provider 走转换，以便非原生 provider 继续可用。
7. 作为 provider 管理员，我希望系统拒绝不兼容的模式/API 组合，以便避免 provider 静默进入无效状态。
8. 作为 WebUI 用户，我希望在编辑 provider 时选择 Responses 模式，以便无需手动修改 JSON。
9. 作为 WebUI 用户，我希望在 provider 列表看到实际生效模式，以便了解每个 provider 如何处理请求。
10. 作为 Responses 客户端，我希望透传模式保持 request JSON 不变，以便 Responses 专用字段继续按原语义工作。
11. 作为 Responses 客户端，我希望透传模式保留 `input`、`instructions`、`tools`、`tool_choice`、`reasoning`、`parallel_tool_calls` 和 `metadata`，以便使用原生 Responses 能力。
12. 作为 Responses 客户端，我希望透传模式保持原生 SSE event 名称和 payload，以便 streaming 客户端收到预期协议。
13. 作为 Responses 客户端，我希望只有转换模式的 provider 才执行 Responses 到 Chat Completions 的转换。
14. 作为 Responses 客户端，我希望转换后的文本输出恢复为 Responses output item，以便客户端无需理解 upstream 协议。
15. 作为 Responses 客户端，我希望转换模式支持 function tools，以便工具型 agent 可以使用 Chat Completions provider。
16. 作为 Responses 客户端，我希望支持多个 function tools，以便完整传递工具集合。
17. 作为 Responses 客户端，我希望保留并行 tool calls，以便多个工具调用不会被合并或丢失。
18. 作为 Responses 客户端，我希望转换时保留 `tool_choice`，以便继续控制工具选择。
19. 作为 Responses 客户端，我希望 tool call 保留 `call_id`、函数名和 arguments，以便正确关联工具结果。
20. 作为 Responses 客户端，我希望不支持的 tool 类型返回明确的 `not_supported` 错误，以便不会发生静默丢弃。
21. 作为 Responses 客户端，我希望 reasoning usage 映射到 Responses reasoning details，以便观察推理 token。
22. 作为统计用户，我希望 reasoning token 作为 output token 子集记录，以便总量不重复计算。
23. 作为统计用户，我希望缓存输入 token 保留在 Responses usage 中，以便缓存指标准确。
24. 作为 streaming 客户端，我希望转换型文本增量映射为 Responses SSE events，以便 Chat Completions upstream 也支持流式体验。
25. 作为 streaming 客户端，我希望转换型 tool-call 增量和完成事件可用，以便流式 function call 正常工作。
26. 作为 streaming 客户端，我希望转换后的最终事件包含 Responses usage，以便请求结束后完成统计。
27. 作为 streaming 客户端，我希望透传模式保留 upstream usage events，以便原生 provider 的 usage 细节不被重写。
28. 作为客户端，我希望透传模式保留 upstream HTTP 错误，以便获得 provider 原始诊断信息。
29. 作为客户端，我希望代理和转换错误使用 Responses 错误结构，以便统一进行机器处理。
30. 作为客户端，我希望转换失败不会被报告为成功完成，以便 agent 正确处理失败。
31. 作为客户端，我希望 streaming 开始前发生 upstream 故障时可以 failover，以便请求能够切换到下一个 provider。
32. 作为 streaming 客户端，我希望已经开始的 stream 不被重放，以便避免重复输出和重复工具调用。
33. 作为运维人员，我希望 streaming 失败被单独记录，以便诊断不完整的 upstream 响应。
34. 作为 provider 管理员，我希望透传模式保留客户端业务 headers，以便 provider-specific metadata 继续可用。
35. 作为 provider 管理员，我希望 provider 认证 header 优先于客户端 header，以便客户端不能覆盖 provider 凭证。
36. 作为 provider 管理员，我希望复制、导入和旧版本 profile 在缺失字段时安全使用默认值，以便新增字段不破坏现有流程。

## Implementation Decisions

- 以现有 Responses request handler 作为核心行为 seam。
- handler 必须按候选 provider 独立读取和应用 `responsesMode`，不能为整个请求固定一个全局模式。
- 缺失配置或新建 provider 的模式默认为 `auto`。
- `openai-responses` 是原生 Responses API provider 类型；`openai-completions` 是 Chat Completions provider 类型。
- `passthrough` 只允许用于 `openai-responses`。
- `convert` 只允许用于 `openai-completions`。
- 不做运行时能力探测，以 provider 的 API 声明作为能力依据。
- 透传模式保持 request body、Responses response body、SSE events、HTTP status、错误 body 和非 hop-by-hop headers 不变。
- 代理可以解析透传流量以完成 routing、usage、reasoning、conversation 和日志，但不得修改发送内容。
- 客户端业务 headers 在移除 hop-by-hop headers 后保留；provider 认证和 provider 自定义 headers 按既定优先级合并。
- 转换模式必须支持文本输出、function tools、多个并行 tool calls、`tool_choice`、reasoning details、缓存 usage 和 Responses usage。
- 转换模式遇到不支持的 tool 类型时，返回结构化 `not_supported` 错误。
- streaming 转换必须支持响应创建、output item、content part、文本增量、tool-call arguments、完成和失败等既定 Responses events。
- 在响应 headers 或 SSE events 发出前失败时可以切换候选 provider；streaming 输出开始后不得重放或切换。
- 非 streaming 继续使用现有可重试 status 和 failover 规则。
- 透传错误保持 upstream 错误；路由、转换、代理和 failover 错误使用统一 Responses 错误结构。
- usage 缺失时沿用现有 unknown/zero 统计兼容规则，不影响正文成功返回。
- provider 配置新增 `responsesMode` 字段；缺失字段的旧配置仍然有效。
- WebUI 在现有 provider 编辑表单增加模式选择，在现有 provider 列表增加实际模式 Badge。
- TUI 和 CLI 读取并保留该字段，但本 issue 不新增配置交互。

## Testing Decisions

测试只验证外部可观察行为，不测试私有 helper 的实现细节。

Rust proxy 测试覆盖：

- 缺失模式字段默认为 `auto`。
- 合法和非法的模式/API 组合。
- 透传请求和响应 body 保持不变。
- 透传 headers 和认证优先级。
- 非 streaming Responses → Chat Completions 转换。
- 文本响应转换回 Responses。
- function tools、多个 tool calls、并行调用和 `tool_choice`。
- 不支持的 tool 类型返回错误。
- reasoning 和 cached usage 映射。
- 透传 streaming event 保持不变。
- Chat → Responses streaming event 转换。
- usage 缺失。
- upstream HTTP 错误。
- 转换错误。
- streaming 开始前 failover。
- streaming 输出开始后不重放。
- 透传与转换 provider 混合候选时的 failover。

配置测试覆盖：

- 旧 profile 反序列化。
- `auto` 默认行为。
- profile 复制。
- profile 导入。
- `responsesMode` 往返持久化。

WebUI 测试覆盖：

- 模式选择器初始化与保存。
- provider 列表实际模式 Badge。
- 兼容性校验及可见错误状态。
- `openai-responses` 与 `openai-completions` 下的 `auto` 说明。

测试使用现有 Rust library test 约定和 WebUI Vitest 约定。WebUI 测试必须使用 `NODE_ENV=test` 运行。测试 fixture 不依赖真实外部 provider。

## Out of Scope

- 新增 TUI 或 CLI 的 Responses 模式配置控件。
- 运行时探测 upstream 能力。
- 转换模式支持非 function Responses tools，包括 web search、file search 和 computer-use tools。
- upstream 失败后重放已经部分输出的 stream。
- 重新设计公开的 `/v1/responses` endpoint。
- 修改无关的 Chat Completions、Anthropic Messages 或 Gemini 行为。
- 建立适用于其他 API 的通用协议转换框架。
- 修改 provider 认证存储或 credential resolution 语义，但透传所需的 header 优先级除外。
- 新增统计页面或改变现有消费计算规则。

## Further Notes

- `Responses 透传模式` 已加入 `CONTEXT.md` glossary。
- ADR `0004-responses-provider-passthrough.md` 已记录 provider 声明式透传/转换决策。
- 现有非 streaming 转换行为是兼容基线；实现应扩展现有路由架构，不建立并行路由体系。
- 发布时应添加 `ready-for-agent` triage label。
- 实现应遵循已确认的 seams：Responses handler 是核心行为 seam，Provider 配置是持久化 seam，WebUI provider form/list 是交互 seam。
