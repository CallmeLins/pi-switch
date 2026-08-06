# 06 — 混合 Provider Failover 与 Responses 全链路验证

**What to build:** 同一个 Responses 请求可以在透传 provider 与转换 provider 之间按候选顺序 failover，并且非 streaming、streaming、tools、usage、错误和日志行为保持一致。

**Blocked by:** 02 — 原生 Responses 非流式透传；03 — Responses 非流式转换与工具调用；04 — 原生 Responses Streaming 透传；05 — Chat Completions Streaming 转 Responses

**Status:** resolved

- [x] 非 streaming 请求可在两种 provider 模式之间 failover
- [x] streaming 请求开始前可在两种模式之间 failover
- [x] streaming 开始后不重放请求
- [x] 每个候选 provider 按自身 `responsesMode` 工作
- [x] 最终错误符合统一 Responses 错误契约
- [x] passthrough 与 conversion 的请求/响应不互相污染
- [x] 混合候选、重试状态、日志和旧配置有集成测试
- [x] 全 Rust 测试和 WebUI 测试通过

## 实施总结

- 提交：`4a72846` — `feat(proxy): mixed provider failover for Responses`
- 实现的 seams：S1 `forward_responses_mixed`（非流式按候选模式分派 + 跨模式切换）；S2 `forward_responses_mixed_stream`（流式透传/转换分派，headers 前切换）；S3 模式隔离集成测试 + 全链路验证
- 验收标准：以上 8 项均已验证并标记为 `- [x]`
- 测试结果：Rust `cargo test` 204 passed（新增 5 个端到端：非流式双向 failover、流式双向 failover、模式隔离、混合链日志断言）；WebUI Vitest 94 passed
- typecheck：`cargo check` 通过；新增代码 rustfmt 通过；clippy 无新增 warning
- 文档对齐：无需更新 README（网关特性行已覆盖 failover 与 Responses 转换）
- 遗留 / 后续建议：① 双轴 review 修复项：转换失败不再截断 failover 链（记错误、链耗尽才返回）、`record_success` 移至 SSE content-type 守卫后、混合链日志断言测试、失败臂抽 `log_failed_attempt` 消除 12 参重复、恢复 circuit-open/half-open 日志；② `forward_responses_mixed` 与 `forward_responses_mixed_stream` 约 150 行结构相似（成功臂异构无法共享），完整合并为参数化单循环可作后续重构候选；③ 本 issue 完成后，responses-provider-passthrough 全部 6 个 tickets（01–06）均已 resolved，特性完整交付
