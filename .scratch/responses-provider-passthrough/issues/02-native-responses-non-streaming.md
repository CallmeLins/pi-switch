# 02 — 原生 Responses 非流式透传

**What to build:** 当请求路由到原生 Responses provider 时，非 streaming 请求和响应均保持 Responses 协议原样传递，同时正确处理认证 headers、业务 headers、usage 旁路解析和 upstream 错误。

**Blocked by:** 01 — Provider Responses 模式配置与 WebUI 控制

**Status:** resolved

- [x] `auto` / `passthrough` 的原生 Responses provider 走原样透传
- [x] request body 不被转换或丢字段
- [x] response body、status、错误 body 和非 hop-by-hop headers 保持 upstream 语义
- [x] provider 认证 header 覆盖客户端认证 header
- [x] 客户端业务 headers 和 provider 自定义 headers 按既定规则合并
- [x] usage、reasoning、conversation 和 request log 仍可记录
- [x] upstream 错误不被代理重写
- [x] passthrough 非流式路径有端到端测试

## 实施总结

- 提交：`34f3410` — `feat(proxy): native Responses non-streaming passthrough`
- 实现的 seams：S1 原生 Responses 非流式 endpoint；S2 请求 headers 合并；S3 response/error 与旁路记录
- 验收标准：以上 8 项均已验证并标记为 `- [x]`
- 测试结果：Rust `cargo test` 175 passed（含 3 个新端到端测试）；WebUI Vitest 94 passed
- typecheck：`cargo check` 通过；新增代码 rustfmt 通过；clippy 无新增 warning
- 文档对齐：更新 `README.md` / `README_ZH.md` 网关特性行（原生 OpenAI Responses 透传）
- 遗留 / 后续建议：① 修复了 `append_log_line` 并发写入竞态（全局锁串行化，防止日志行黏连）——真实并发代理请求同样受益；② streaming 透传与 Chat→Responses SSE 转换由 tickets 04/05 继续；③ 熔断跳过的候选暂未写日志行（与既有 chat 路径的 circuit_open 日志不一致，可后续补）
- 后续修复（`8f5a9b0` — `fix(test): isolate proxy tests from real log`）：原 `PI_SWITCH_CONFIG_DIR` 仅隔离了单个日志测试，另两个透传测试仍写真实 `~/.pi-switch/requests.log` 造成 webui 统计污染（`native/model-a` 条目）。改为 proxy 测试状态目录统一重定向：`state_dir()` 在 `#[cfg(test)]` 下指向 per-process temp（`init_test_state_dir`），`append_log_line` 与 `circuit_path` 均经它解析；移除 env 覆盖机制。修复后全量测试不再写真实日志（行数不变），并清理了已污染数据（备份 `~/.pi-switch/requests.log.bak-test-pollution-*`）
