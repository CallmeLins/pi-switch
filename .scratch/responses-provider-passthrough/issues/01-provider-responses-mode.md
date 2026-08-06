# 01 — Provider Responses 模式配置与 WebUI 控制

**What to build:** 用户可以在 WebUI 为 provider 查看和选择 `auto`、`passthrough`、`convert` 模式；配置可保存、复制、导入并兼容旧 profile；provider 列表展示实际生效模式。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 新增 `responsesMode` 配置，旧配置缺失时按 `auto` 处理
- [x] `auto` 根据 provider API 类型计算实际模式
- [x] WebUI provider 编辑表单支持三种模式
- [x] 不兼容的模式/API 组合无法保存并显示明确错误
- [x] provider 列表显示实际生效模式 Badge
- [x] 复制、导入和配置往返持久化保留该字段
- [x] TUI/CLI 能读取并保留该字段
- [x] 配置层、API 层和 WebUI 测试覆盖上述行为

## 实施总结

- 提交：`ccef15a` — `feat(profiles): add Responses mode configuration`；`43f6ace` — `test(profiles): cover Responses mode persistence`
- 实现的 seams：S1 Provider 配置 REST；S2 WebUI ProviderForm；S3 WebUI Profiles 列表
- 验收标准：以上 8 项均已验证并标记为 `- [x]`
- 测试结果：Rust `cargo test` 172 passed；WebUI Vitest 4 files / 94 tests passed
- typecheck：Rust `cargo check`、目标 Rust 文件 rustfmt check、WebUI `npm run typecheck` 均通过
- 文档对齐：更新 `README.md` 与 `README_ZH.md` 的 Provider 管理说明
- 遗留 / 后续建议：Responses 请求透传、转换和 streaming 行为由 tickets 02–06 继续实现；仓库已有的 `src-rust/proxy.rs` 格式差异及其他未跟踪文件未触碰
