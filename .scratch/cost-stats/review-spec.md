# Cost-Stats Diff Review vs spec.md + issues 01–05

**Verdict:** Feature complete. Verified by running `cargo test --lib` (129 passed) and webui `vitest run StatsPanel.test.tsx format.test.ts` (54 passed). Cost formula, aggregation scope, webui/TUI display, auto-refresh, and exports all match the spec.

## (a) Missing / partial requirements

- 无硬性缺失项。唯一措辞级偏差：spec「金额格式化：`$0.00 / $1 以下 4 位有效小数 / $1 以上两位小数 / 大额 K/M 缩写 / -`」——webui `formatCost`（`toFixed(4)` 去尾零）与 TUI `format_cost`（`{:.4}` 去尾零）实现的是「4 位小数、去尾零」，非「4 位有效小数」：如 `0.000042 → "$0"`。spec 全部示例（$0.0042 等）均通过，webui/TUI 规则一致，影响可忽略。
- 提示口径：spec「unknown 行单独计数」（与 token 同口径）——`cost_unknown` 仅在 in-scope 行（成功、非 retry、有 usage）内计数；成功但无 usage 解析的行不计入 unknown，请求明细中仍显示 `-`。与「聚合口径」一致，非缺陷。

## (b) Scope creep

- `src-rust/proxy.rs` `conversation_id_of` 新增 `x-opencode-session` header 回退（含新测试 `conversation_id_falls_back_to_opencode_session_header`）——属 conversation-id-inject feature（spec Out of Scope 之外），cost-stats spec 未要求。
- `webui/package-lock.json` 删除 458 行（清理 extraneous 平台包）、`webui/dist/.gitkeep` 删除——与 cost-stats 无关的噪音变更。

## (c) Implemented-but-wrong

- 大额缩写边界：spec「大额 K/M 缩写」——`formatCost`/`format_cost` 均以 `value < 1000` 走两位小数分支，`999.999 → "$1000.00"`（未触发 K 后缀），四舍五入跨过 1000 时输出与缩写规则不符。纯观感，双端行为一致。
- 亚美分金额坍缩为 `"$0"`：`formatCost(0.000042) → "$0"`（`toFixed(4)` 后去尾零），TUI 同。可与 `"$0.00"`（显式零）和 `"-"`（unknown）区分，但极小金额易被误读为零。
- TUI `format_cost` 对 ≥1000 用 `cost as u64` 截断后再缩放（`1234.9 → 1234 → "$1.2K"`）——因缩放保留 1 位小数，输出与 webui 一致，无用户可见差异。

## 其余核对（全部通过）

- 公式 `(prompt−cached)×input + cached×cacheRead + completion×output`，cacheWrite 不参与，档位按 prompt 选最高达标阈值；无单价 → `costTotal: null`（issue 01 ✓）
- 聚合与 token 同口径（`usage_of` 过滤 ok/retry/usage）；`totalCost`/`costUnknown`/`ConversationStats.cost`/`RecentRequest.cost`/types.ts 镜像（issue 02 ✓）
- CSV 表头+行、JSON 导出含 `costTotal`；旧行 `#[serde(default)]` → None（issue 02 ✓）
- 消费卡片 + unknown 提示、对话/请求消费列、全 unknown `-`、`$0.00` 显式零（issue 03 ✓）
- 四档 Off/5s/30s/5min 默认 Off；切换重置计时；Off 停止；卸载清理；失败保留数据；沿用当前窗口参数（issue 04 ✓，测试覆盖）
- TUI Overview 总消费行 + i18n EN/ZH（issue 05 ✓）
