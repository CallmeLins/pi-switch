# 为 pi 请求注入对话标识（Conversation ID）

Status: ready-for-agent

## Problem Statement

pi-switch 统计页的"对话（Conversation）"聚合依赖客户端请求携带对话标识（`x-conversation-id` / `x-opencode-session` / body `conversation_id`），但 pi 客户端默认不发送任何此类标识——于是绝大多数请求被归入"未标记（Unlabeled）"单一组，用户无法回答"这一次对话烧了多少 token"。本 feature 在 pi 侧补齐缺失的一环：为每次 provider 请求注入当前对话的标识，让 pi-switch 现有统计（零改动）就能按对话归组。

## Solution

一个独立的 pi 扩展模块：订阅 `before_provider_headers` 生命周期钩子，把当前 Session UUID（`ctx.sessionManager.getSessionId()`）注入 `x-conversation-id` 请求头。pi 保证重试复用同一组 headers，因此重试不会拆分对话；`/resume` 同一 session 文件时 UUID 稳定，`/new` 时更换，与"整个 session 文件"的对话边界一致。注入逻辑封装为纯函数，`node --test` 单测。默认启用、无配置。pi-switch proxy 的对话标识探测已将该头列为最高优先级（ADR-0002），故无需 pi-switch 侧任何改动。

## User Stories

1. As a pi 用户, I want every provider request pi sends to carry my current dialogue's ID, so that pi-switch can group them into one conversation
2. As a pi 用户, I want the ID to stay the same when I resume the same session file, so that a resumed dialogue keeps accumulating in the same conversation
3. As a pi 用户, I want starting a new session to produce a new ID, so that a new dialogue never bleeds into the previous one's stats
4. As a pi 用户, I want the ID carried in the x-conversation-id header, so that pi-switch's existing conversation detection picks it up at highest priority
5. As a pi 用户, I want the stats page to stop lumping my requests into unlabeled, so that I can see per-dialogue token usage
6. As a pi 用户, I want requests without a valid session ID to skip injection, so that edge cases (in-memory sessions) never produce garbage headers
7. As a pi 用户, I want an existing x-conversation-id header to be overwritten by the session ID, so that stale values never mislabel a request
8. As a pi 用户, I want injection enabled by default with zero configuration, so that grouping works immediately after installing the extension
9. As a pi 用户, I want the extension to live in its own module separate from the /piswitch command, so that each concern stays independently testable
10. As a pi 用户, I want the injected header to be harmless to upstream providers that ignore unknown headers, so that direct-to-provider requests keep working unchanged
11. As a pi 用户, I want the inject logic unit-tested as a pure function, so that regressions are caught without a live pi session
12. As a pi 用户, I want the tests to run with node --test, so that the project's existing JS test runner is reused

## Implementation Decisions

- **注入钩子**：订阅 `before_provider_headers`，每次 provider 请求触发一次；pi 保证重试复用同一组 headers，重试请求自动携带同一对话标识，不会把一次请求的多个尝试拆散到不同对话
- **注入值**：当前 Session UUID（`ctx.sessionManager.getSessionId()`），与 session 文件绑定——`/resume` 同一文件同一 UUID，`/new` 新 UUID
- **头名**：`x-conversation-id`；pi-switch proxy 的对话标识探测将该头列为最高优先级（ADR-0002 已落地），pi-switch 侧零改动
- **纯函数边界**：注入逻辑封装为纯函数（入参：headers 与 sessionId；出参：设置后的 headers）。sessionId 非空时赋值覆盖；为空（如内存会话）时不注入且不触碰既有头；其余头一律不动
- **默认行为**：无条件启用，无配置项、无环境变量开关
- **模块组织**：独立扩展模块，并在 pi 扩展登记表中追加登记；与既有 `/piswitch` 命令扩展互不引用
- **上游影响**：未知请求头对主流上游（OpenAI / Anthropic 兼容端点）无害，直连场景不改变既有行为

## Testing Decisions

- 好的测试 = 只测外部行为：给定 headers 与 sessionId，断言注入后的 headers——不依赖 pi 运行时、不 mock 扩展框架
- **被测模块**：扩展模块中的纯函数 `injectConversationId`
- **用例**：非空 sessionId 注入并覆盖既有值；sessionId 为空 / 纯空白时不注入、原 headers 原样保留；其它头不受影响；返回值与入参分离（不污染调用方对象）
- **先例**：项目 JS/TS 层目前无测试文件（Rust 侧 `cargo test` 82 用例、webui vitest）；本模块以 `node --test` 建立 JS/TS 层测试先例（node ≥23.6 原生 type stripping 可直跑 .ts 测试文件）
- **验收（人工端到端）**：以经 pi-switch proxy 的 profile 启动 pi 会话发请求，`/piswitch stats` 的 byConversation 出现 UUID 标识而非 unlabeled；`/new` 后再请求，出现第二个 UUID

## Out of Scope

- 费用（cost）换算：属于 pi-switch 的职责，本插件不涉及
- 对话显示名可读化（注入可读名称、或 pi-switch 读 session 文件映射 UUID→名称）
- pi-switch proxy / stats / WebUI / TUI 的任何改动
- body `conversation_id` 字段注入（header 已覆盖最高优先级路径，body 兜底路径不需要）
- 注入开关配置
- 历史已记录为 unlabeled 的请求回溯归组（无法事后补救）

## Further Notes

- 术语对齐：CONTEXT.md 的"对话（Conversation）"定义为三源识别（`x-conversation-id` 头优先、`x-opencode-session` 次之、body `conversation_id` 兜底），本插件实现其中最高优先级路径；"未标记（Unlabeled）"将因此显著减少
- 决策已记录：`docs/adr/0002-conversation-identity.md` 已确立探测优先级，本插件与之对齐，无新增 ADR
- pi 扩展文档 `before_provider_headers` 的官方示例即注入会话 ID 用于网关归因，本插件采用同机制、换用 pi-switch 识别的头名
