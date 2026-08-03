# 07 — 收尾：核对完成情况 + 更新文档

**What to build:** 功能实现的收尾票：逐个核对 01–06 的验收标准是否全部满足、无遗留；同步更新项目文档（README.md 与 README_ZH.md），记录 Token 使用量统计功能（累计总量、单次对话统计、缓存命中率）的用法与统计面板展示变化，确保用户侧文档与实现一致。

**Blocked by:** 01, 02, 03, 04, 05, 06 — 全部实现票

**Status:** done

- [x] 逐票核对 01–06 的验收标准：全部满足或显式记录未满足项（不允许静默跳过）
- [x] 验证端到端链路可用：真实请求 → 请求日志含 token 数据 → 统计接口返回 → WebUI/TUI 正确展示（WebUI 环节未满足，见下方记录）
- [x] README.md 与 README_ZH.md 同步更新：新统计能力、缓存率口径、对话统计口径（含 unlabeled 语义）与现有文档语言一致
- [x] 更新后的文档无过期信息（命令示例、界面描述与实际一致）

## 核对结果（2026-08-02）

### 01 Prefactor — 满足
`aggregate(entries, circuit, cooldown_ms, now_ms)` 为纯函数（stats.rs:210，无 I/O）；`get_stats()` 只读文件后调用。单测覆盖空输入/单行/多行/异常行跳过（`aggregate_empty_entries_yields_zero_stats`、`aggregate_single_success_entry_counts_everywhere`、`aggregate_multiple_entries_accumulates_groups`、`parse_entries_skips_empty_and_malformed_lines`）。

### 02 Usage 解析模块 — 满足
`usage.rs` 全纯函数。`extract_usage` 三风格探测顺序（`extract_usage_reads_all_three_field_styles`、`extract_usage_probes_cache_fields_in_order_and_takes_first`）；`SseUsageParser` 覆盖 OpenAI/Anthropic、任意 chunk 切分、CRLF、垃圾输入、缺失事件不 panic（12 个测试）。缓存率分母口径由 aggregate 侧测试覆盖（`aggregate_cache_hit_rate_is_cached_over_total_input`）。

### 03 代理采集 — 满足
`stream_response` tee 直通 + `StreamTee` 单次回调；`conversation_id_of` 头优先 body 兜底（`conversation_id_prefers_header_over_body`、`conversation_id_falls_back_to_body_when_header_absent_or_empty`）；流掐断/无 usage 仍写行（`stream_tee_drop_mid_stream_still_reports`、`stream_tee_reports_none_when_stream_has_no_usage`）；旧行共存（`log_entry_roundtrips_through_request_log_entry`、`parse_entries_defaults_missing_token_fields_to_none`）。

### 04 聚合扩展 — 满足
`UsageStats` 含 `totalTokens`/`cacheHitRate`/`byConversation` 与 by-provider token 列；`/api/stats` 返回（实测见下）；CSV/JSON 导出含 token 与对话列（`csv_export_includes_token_and_conversation_columns`、`json_export_serializes_token_and_conversation_fields`）；无 token 数据 → 总量 0、缓存率 `-`（`aggregate_no_token_data_serializes_empty_by_conversation`、`aggregate_cache_rate_is_dash_without_any_token_data`）。

### 05 WebUI 展示 — 未满足（显式记录）
`webui/src` 无任何 `totalTokens`/`cacheHitRate`/token 展示代码，git 历史无对应提交。05 的实现不在本 issue 范围内执行（07 只做核对与文档，实现票需另行处理）。因此验收标准 2 的「WebUI 正确展示」环节标记未满足——端到端链路验证到「统计接口 + TUI 数据源」为止。README 未声称 WebUI 存在 token 展示，避免过期信息。

### 06 TUI 展示 — 满足
`render_stats` Overview 追加 Tokens 与 Cache hit rate 两行（commit 0b9de83）；无 token 数据显示 `-`（`token_summary_without_data_is_dash`）；与 WebUI 同源（`UiData::load` 与 `/api/stats` 均来自 `get_stats()`）。

### 端到端链路实测（隔离 HOME + 本地 mock SSE 上游）
1. 真实请求（`x-conversation-id: e2e-conv-1`，model `mock/mock-model`）→ 流式逐 chunk 透传，无缓冲
2. `requests.log` 行含 `promptTokens: 1234`、`completionTokens: 567`、`cachedTokens: 900`、`conversationId: "e2e-conv-1"`；第二个无标识请求行 `conversationId: null`
3. `GET /api/stats`（隔离 WebUI，port 43121）实测：`totalTokens {input: 2468, output: 1134, total: 3602}`（2×1234 / 2×567，与日志一致）；`cacheHitRate: "72.9%"`（1800÷2468 手算一致）；`byConversation` 两组：`e2e-conv-1` 与 `unlabeled`，各 1 请求
4. TUI 数据源：`UiData::load` → 同一 `get_stats()`；渲染值由 06 单测覆盖（headless 无法真实渲染 TUI，以此作为等价证据）
