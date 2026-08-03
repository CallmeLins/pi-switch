# Developer Role 兼容问题诊断报告

- 日期：2026-08-04
- 状态：已定位根因，已通过手动配置解决（未改代码）
- 相关文件：`src-rust/ops.rs`（sync_gateway_to_pi）、`webui/src/components/ProfilesPanel.tsx`（defaultModel）

## 一、现象

pi 客户端使用 pi-switch 暴露的模型时报错：

```
Error: 400: {"param":null,"type":"invalid_request_error","code":"invalid_request_error",
"message":"Error from provider (Console Go): Upstream request failed:
[invalid_request_error] Failed to deserialize the JSON body into the target type:
messages[0].role: unknown variant `developer`, expected one of `system`, `user`,
`assistant`, `tool`, `latest_reminder` at line 1 column 28100"}
```

触发条件：

- 使用**自定义配置文件**（模型条目带 `reasoning: true`）时必现
- 清除配置、用网页 fetch-models 重新导入模型后正常
- 两种配置在模型 ID / 名称 / 上下文窗口上看似一致，实际差异在 `reasoning` 字段

## 二、因果链

```
① config.json 模型条目 reasoning: true        ← 自定义配置
② sync_gateway_to_pi 原样透传条目到 pi models.json  ← ops.rs:542-555
③ pi 解析模型：reasoning: definition.reasoning ?? false → true
                                              ← pi provider-composer.js:65
④ pi 构造请求：useDeveloperRole =
     model.reasoning && compat.supportsDeveloperRole
                                              ← pi-ai openai-completions.js:793
⑤ compat 检测：对非标准 URL 默认 supportsDeveloperRole = true
                                              ← pi-ai openai-completions.js:1160
⑥ pi-switch 代理原样透传 body（不修改 role）
⑦ 上游 opencode zen 网关（Rust/serde）schema 枚举无 developer → 400
```

**故障因子只有一个：`reasoning: true`。**

方式 B（fetch-models 重导）能用的原因：fetch 只返回模型 ID，webui `defaultModel` 生成的条目无 `reasoning` 字段 → ③ 为 false → 发 `system` role → 上游契约内 → 正常。

## 三、深层原因

1. **developer role 是可选新特性，不是兼容契约的必选项**。OpenAI 2025 年为 reasoning 模型引入 developer role（推荐项）；`system` 从未被废弃，仍是官方契约内合法 role。"兼容 OpenAI 接口"的实际含义是端点形状 + 主流 schema **子集**兼容，兼容层是否跟进新特性是各家的范围决定。opencode zen 网关的 schema 枚举（错误消息即为契约声明）不含 developer。
2. **pi 的默认值建立在一串隐含假设上**：`reasoning: true → developer role` 对 OpenAI 官方端点成立（官方推荐），但 pi 把这个默认应用到所有 OpenAI-compatible 端点。pi 提供文档化校正机制：`compat.supportsDeveloperRole: false`（models.md 明确列出，用于 LiteLLM、自定义代理等非官方端点）。
3. **pi-switch 缺桥接层**：它同步了 `reasoning`（激活 pi 的 developer 假设），却没有同步"模型是否支持 developer role"这一约束。pi 的 compat 开关未被 pi-switch 使用，pi 对官方 OpenAI 的假设被静默传递到不满足该假设的上游。
4. **"配置没区别"的错觉**：两种配置差异仅在 `reasoning`（及派生的 cost/compat/thinkingLevelMap），被 ID 级相似掩盖。

## 四、归因结论

| 方 | 行为 | 判定 |
|---|---|---|
| pi | reasoning 模型用 developer role，有文档有开关 | 设计内，非 bug |
| 上游 opencode zen 网关 | schema 枚举不含 developer，合法拒绝 | 契约声明，非 bug |
| pi-switch | 忠实透传 reasoning，未做 compat 适配 | 缺口所在 |

## 五、验证与复现

最小复现脚本 `/tmp/opencode/repro-role.mjs`（直接驱动 pi-ai 的 `stream()`，假 fetch 截获实际请求体）：

```
reasoning=true  compat={}                           → role = developer  ← 报错场景
reasoning=true  compat={supportsDeveloperRole:true} → role = developer
reasoning=true  compat={supportsDeveloperRole:false} → role = system    ← 修复场景
reasoning=false / 缺省                              → role = system    ← 正常场景
```

磁盘证据（诊断时点）：

- `~/.pi-switch/config.json`：profile `opencode-go` 3 个模型全部 `reasoning: true`
- `~/.pi/agent/models.json`：`pi-switch` provider 条目 `opencode-go/deepseek-v4-flash` 带 `reasoning: true`（sync 后仍如此）

## 六、解决方案与复发风险

**已采用（手动配置）**：在 pi 侧模型条目的 `compat` 中显式加 `"supportsDeveloperRole": false`，pi 改用 `system` role，思考功能保留，问题解决。

**复发风险**：若该字段只加在 pi 的 models.json 上，下次网页 sync（勾选模型、改 profile、fetch-models）会重建 `pi-switch` provider 条目，手动修改被抹掉，问题复发。建议持久化到 `~/.pi-switch/config.json` 的 profile.models 条目 compat 中（sync 原样透传），或后续在 `sync_gateway_to_pi` 中对 `reasoning: true` 且未显式声明 compat 的模型自动附加 `supportsDeveloperRole: false`（本报告日期未实施，用户选择保持手动方案）。
