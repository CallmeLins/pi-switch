# 01 — 扩展注入对话名称（纯函数 + 接线）

**What to build:** pi 扩展在注入 `x-conversation-id` 的同时注入当前会话的显示名称。端到端行为：给定一组请求头与会话信息（id + 名称），核心逻辑正确设置 `x-conversation-name`（名称非空时覆盖既有同名值；空/空白时不注入且保留原值），返回值不污染调用方对象；接线层把该逻辑接入 `before_provider_headers` 钩子，与对话标识注入保持同一纯函数边界。本片为 02 提供经过验证的名称数据源，并建立名称注入的测试先例。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 名称非空时，`x-conversation-name` 被设置为该值，且覆盖请求头中已有的同名值
- [x] 名称为空或纯空白时，不注入，原请求头原样保留（含既有同名头）
- [x] id 与 name 注入互不干扰、独立生效（空名称不影响 id 注入，反之亦然）
- [x] 函数返回新对象，不修改调用方传入的请求头对象
- [x] 会话信息提供方接口同时暴露 id 与名称，接线层两处注入均经该接口取值
- [x] 测试用项目现有 JS 测试运行器（node --test）执行并全绿

## 实施总结
- 提交：`3fa5779` — feat: surface conversation display names in stats (inject, proxy, aggregate, webui)
- 实现的 seams：S1 非空名称注入并覆盖既有 `x-conversation-name`；S2 空/纯空白/undefined 跳过且原头保留；S3 id 与 name 独立生效、返回新对象不污染入参；S4 会话信息提供方接口扩展为同时暴露 id 与名称，`before_provider_headers` 两处注入均经接口取值
- 接口演进：`SessionIdProvider` 增加 `getSessionName()`；`makeBeforeProviderHeadersHandler` 工厂参数由返回 id 的函数改为返回 `{ id, name }` 的函数，既有调用（扩展入口 + 既有测试）同步更新
- 测试结果：`npm test` 9/9 全绿（node --test，node 26 type stripping 直跑 .ts；既有 5 个 id 测试 + 新增 4 个名称测试）
- typecheck：node 加载验证通过；webui tsc 0 错误
