# 01 — sync 时对 reasoning 模型注入 supportsDeveloperRole（代码实现票）

**What to build:** 实现 `docs/developer-role兼容问题诊断报告.md` 六 节推荐的代码修复：在 `sync_gateway_to_pi` 中对 `reasoning: true` 且未显式声明 `compat` 的模型自动附加 `compat.supportsDeveloperRole`（值取全局可选项 `settings.supportsDeveloperRole`，默认 `false`），使 pi 改用 `system` role，兼容 schema 不含 `developer` role 的上游（如 opencode zen）。

**Status:** ready-for-agent

**Blocked by:** 无（文档先行部分见 02 票，已合入）

## 验收标准

- [ ] config.rs `Settings` 增字段 `#[serde(default, rename = "supportsDeveloperRole")] pub supports_developer_role: bool`，缺省 false；旧 config.json（无此键）加载正常
- [ ] ops.rs 抽出 `build_gateway_models(&PiSwitchConfig) -> Vec<serde_json::Value>` 纯函数，`sync_gateway_to_pi` 改用它；函数内对 `reasoning: true` 且 `compat.is_none()` 的条目注入 `{"supportsDeveloperRole": <settings 值>}`
- [ ] 模型级显式 compat 原样透传、优先（`compat.is_some()` 时不注入）
- [ ] `settings.supportsDeveloperRole: true` 时注入 true（支持 developer role 的上游）
- [ ] webui/types.ts `Settings` 接口补 `supportsDeveloperRole?: boolean` 类型镜像
- [ ] ops.rs 增单测（≥7 例：注入/显式 compat 优先/空 compat 边界/非 reasoning 跳过/未列出模型默认条目/proxy profile 排除/设置 true 注入 true），`cargo check` + `cargo test` 全绿
- [ ] README 不再声称"手动修改 models.json"为唯一方案后，与本文档说明一致（可选，联动 02）
- [ ] 按 tdd-implement ⑤⑥⑦ 完成 code review、commit、issue 置 `resolved`

## 关联

- 诊断报告：`docs/developer-role兼容问题诊断报告.md`（五 验证与复现 / 六 解决方案与复发风险）
- 文档先行票：02-docs-faq
