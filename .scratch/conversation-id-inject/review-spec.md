# review-spec — conversation-id-inject WIP 对照 SPEC 评审

评审对象：uncommitted vs HEAD bfeb6ec（extensions/conversation-id-inject.ts、conversation-id-inject.test.ts、package.json、src-rust/proxy.rs）。实测：`npm test` 5/5 通过；`cargo test conversation_id` 5/5 通过（含新增 `conversation_id_falls_back_to_opencode_session_header`）。

## 验证结论（对照 5 项核查）

1. **纯函数** ✓ — 非空注入并覆盖既有值；空/纯空白/undefined 跳过且原头保留；其它头不动；返回新对象（`{...headers}`，测试断言 `notEqual`）。
2. **接线** ✓ — `pi.on("before_provider_headers", ...)` 用 `ctx.sessionManager.getSessionId()` 写入 `x-conversation-id`，与 pi 官方 docs/extensions.md:667-669 示例逐字一致；handler 原地 mutation 符合钩子契约。
3. **登记/独立性** ✓ — package.json `pi.extensions` 改为两个文件条目（packages.md "path is a file → single extension"），同时避免目录加载误把测试文件当扩展；扩展仅 import `ExtensionAPI` 类型，无 `/piswitch` 引用。
4. **node --test** ✓ — 5/5 绿。
5. **proxy.rs 伴生改动** ✓ — `conversation_id_of` 顺序 `x-conversation-id` → `x-opencode-session` → body，与 CONTEXT.md 三源定义一致；新测试同时覆盖兜底与优先级。

## (a) 缺失/部分实现

- **Spec 前提不成立，ADR 未同步**：Spec Solution 称"pi-switch proxy 的对话标识探测已将该头列为最高优先级（ADR-0002），故无需 pi-switch 侧任何改动"。但 `docs/adr/0002-conversation-boundary-from-client-id.md` 只记录两源（"`x-conversation-id` 请求头优先，body `conversation_id` 兜底"），无 `x-opencode-session`；三源定义仅存在于 CONTEXT.md。proxy.rs 落地后 ADR-0002/README 与代码、CONTEXT.md 三方漂移，WIP 未含任何文档同步。另 Spec Further Notes 引用 `docs/adr/0002-conversation-identity.md`——该路径不存在。
- **人工端到端未验收**：Issue 02 勾选项"pi 启动后扩展被加载（扩展登记生效）"与"`/piswitch stats` 可见 UUID 对话"仍为未验证状态（纯静态/单测层面无法覆盖）。

## (b) 范围蔓延

- **Spec Out of Scope 明确写**："pi-switch proxy / stats / WebUI / TUI 的任何改动"——而 `src-rust/proxy.rs` 改动 +37/-8 正属 proxy 改动。任务方将其定位为"CONTEXT.md 三源伴生改动"，Spec Problem Statement 也列出 `x-opencode-session`，但按字面仍越界；落地后必须同步 ADR-0002 与 README（此前多份 audit 已两次标记该遗留）。
- **无关工作区脏改**：`webui/package-lock.json`（删 458 行）、`webui/dist/.gitkeep`（删除）不在 4 个变更文件之列，系先前遗留，与本 feature 无关。

## (c) 实现了但有问题

- **`engines.node >= 20` 与新测试要求矛盾**：Spec 明言"node ≥23.6 原生 type stripping 可直跑 .ts"，但 package.json 仍声明 `"node": ">=20"`；`node --test "extensions/**/*.test.ts"` 在 Node 20/22 及 23.0–23.5 无法直跑 .ts 测试，声明的最低支持版本与实际要求不符。
- **注入值未 trim（次要）**：纯函数用 `trim()` 判空但原样注入值——`" abc "` 通过判空后被逐字写入头。Spec 只要求"为空 / 纯空白时不注入"，未要求清洗值，属边界瑕疵而非违约。
