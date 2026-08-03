# 会话名称首条消息回退（Conversation Title Fallback）

Status: ready-for-agent

## Problem Statement

Stats 页「By conversation」聚合行的显示名只来自 pi 会话的 `/name` 显式显示名（`sessionManager.getSessionName()`）；绝大多数会话未设置显式名，`getSessionName()` 返回 undefined → 扩展不注入 `x-conversation-name` → 对话行显示截断 UUID，用户无法辨认会话。

## Solution

扩展在**无显式名时回退注入会话第一条 user message 的文本**（sanitize 控制字符 + 截断 60 字符）作为 `x-conversation-name`，与 pi 会话选择器"显示名或首条消息"的心智模型一致。显式名优先；回退标题仅为显示属性，不参与对话边界识别（ADR-0002）。仅影响对话聚合行（By conversation），Request details 表不加会话列。

## User Stories

1. As a pi 用户, I want an unnamed session to show its first user message as a readable title in the stats conversation list, so that I can tell which dialogue burned tokens without decoding a UUID
2. As a pi 用户, I want an explicitly named session (`/name`) to keep showing that name, so that explicit names always win
3. As a pi 用户, I want the fallback title to be single-line and control-character-free, so that the injected header and the log stay clean
4. As a pi 用户, I want the fallback title truncated, so that a long prompt never bloats the UI row
5. As a pi 用户, I want sessions without any user message to keep showing the short id, so that nothing breaks
6. As a pi 用户, I want the display name to never affect conversation grouping, so that ADR-0002's identity rules stay untouched

## Implementation Decisions

- **注入契约**：`x-conversation-name` 取值为 `getSessionName() ?? firstUserMessageText(getEntries())`（经 sanitize + 截断）；空/空白不注入且保留既有同名头
- **首条消息提取**：取 entries 中**第一条非空 user message** 的文本——content 为 string 直接用；为 blocks 数组时拼接全部 `type === "text"` 的 text（忽略 image/thinking/toolCall 块）；无 user message 或全空 → `undefined`
- **sanitize**：`replace(/[\x00-\x1f\x7f]+/g, " ")` 后 `trim()`（HTTP header 禁控制字符，防脏数据入 header/日志/导出）
- **截断**：`slice(0, 60)`（`TITLE_MAX_LEN = 60`）
- **接口**：扩展 `SessionIdProvider` 新增 `getEntries(): SessionEntry[]`；`SessionEntry` 为宽松结构 `{ role?: string; content?: string | Array<{ type?: string; text?: string }> }`，不硬依赖 pi 内部 AgentMessage 类型
- **代理防御**：`conversation_name_of`（src-rust/proxy.rs）对 header 值追加 `replace(['\r','\n','\t'], " ")` + `trim()`（纵深防御，HTTP 层已拦 CRLF，防 tab 等脏字符入日志）
- **聚合/UI 零改动**：stats.rs 已取最新行名称、StatsPanel 已 `name || shortConversationId`，名称注入后自然显示
- **compaction 语义**：首条 user message 被 compaction summary 替代后该会话无回退标题（显示 UUID），显式决策

## Testing Decisions

- 只测外部行为：纯函数给 entries/名称输入断言输出；handler 给 mock provider 断言注入后的 headers；代理给 header map 断言解析值
- **扩展（node --test 既有先例）**：显式名优先；无显式名回退首条消息；首条为空取下一条；string 与 blocks 两种 content 形态；控制字符清理；60 字符截断；无 user message → 不注入
- **代理（cargo test 既有先例）**：`conversation_name_of` 清理 `\t`/换行、空值忽略、无 header → None
- 覆盖工具：`node --test "extensions/**/*.test.ts"`、`cargo test --lib`；webui vitest 仅回归确认

## Out of Scope

- Request details 表增加会话列（保持 stats-conversation-name spec 边界）
- CSV/JSON 导出变化
- 折叠状态、排序规则、条数上限调整
- 名称参与对话边界识别（ADR-0002 三源不变）
- compaction summary 文本作为回退标题
- 回退标题的悬停 tooltip 扩展（仍显示完整 conversationId）

## Further Notes

- 承接 stats-conversation-name feature：该 feature 交付"名称显示链路"，本 feature 补齐"名称来源"的未命名场景
- 回退标题只是显示属性；会话选择器与 Stats 页的标题截断长度各自独立（60 字符为本 feature 决策）
