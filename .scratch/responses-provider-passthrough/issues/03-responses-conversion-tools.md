# 03 — Responses 非流式转换与工具调用

**What to build:** 当请求路由到 Chat Completions provider 时，用户可以使用 Responses 非 streaming 请求，包括文本输出、function tools、并行 tool calls、`tool_choice`、reasoning 和 usage。

**Blocked by:** 01 — Provider Responses 模式配置与 WebUI 控制

**Status:** resolved

- [x] `auto` / `convert` 的 Chat Completions provider 走转换
- [x] `input`、`instructions` 和常用请求参数正确转换
- [x] 多个 function tools、并行 tool calls 和 `tool_choice` 正确往返
- [x] `call_id`、函数名和 arguments 保持可关联
- [x] 文本、tool call、cached usage 和 reasoning usage 转换为 Responses 语义
- [x] 不支持的 tool 类型返回结构化 `not_supported`
- [x] conversion error、no route 和 failover exhausted 使用 Responses 错误结构
- [x] 非流式转换路径有端到端测试

## 实施总结

- 提交：`90b80d5` — `feat(proxy): Responses non-streaming conversion with function tools`
- 实现的 seams：S1 `responses_to_chat`（input item 完整转换 + Result 错误通道）；S2 `chat_response_to_responses`（tool_calls→function_call + Invalid 错误）；S3 convert 分支端到端（4 个真实 HTTP 测试）
- 验收标准：以上 8 项均已验证并标记为 `- [x]`
- 测试结果：Rust `cargo test` 186 passed（新增 4 个端到端 + 7 个单元测试）；WebUI Vitest 94 passed
- typecheck：`cargo check` 通过；新增代码 rustfmt 通过；clippy 无新增 warning
- 文档对齐：更新 `README.md` / `README_ZH.md` 网关特性行（Responses ↔ Chat Completions 转换含 function tools）
- 遗留 / 后续建议：① upstream 返回 200 但 body 非 JSON 时仍原样透传（pre-existing 行为，未改；如需严格 conversion_error 可后续处理）；② not_supported 用 501（与 streaming 分支一致，OpenAI 惯例 400，spec 仅要求结构化 type）；③ 混合模式 failover（native+convert 候选并存）属 issue 06 范围
