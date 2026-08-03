# 02 — 统计聚合与导出扩展

**What to build:** 统计聚合在现有 token 口径上增加消费维度，并透出到统计 API 与导出。端到端行为：统计接口返回总消费、消费 unknown 行数与单次对话消费、单次请求消费；总消费 = 已知消费行之和（与 token 使用量同口径），unknown 行单独计数；CSV 与 JSON 导出包含消费列；旧日志行（无消费字段）按 unknown 处理，统计不报错。

**Blocked by:** 01 — 代理侧消费计算与日志写入

**Status:** resolved

- [x] 统计接口新增总消费与消费 unknown 行数（unknown 展示为 `-` 的数据基础）
- [x] 按对话聚合的统计每行包含消费（对话消费；归组依赖对话标识，标识缺失时按现有未标记规则）
- [x] 请求明细每行包含消费（unknown 行为空）
- [x] 消费求和与 token 使用量同口径：仅成功且非 retry 且解析到 usage 的行计入
- [x] 旧日志行（无消费字段）反序列化兼容，统计不报错
- [x] CSV 导出追加消费列；JSON 导出包含消费字段
- [x] 相关单测全绿（求和、unknown 计数、对话聚合、旧行兼容、导出列）

## 实施总结
- 提交：`f07fe1f` — feat: add cost tracking to request logs, stats aggregation and dashboards
- 实现的 seams：S4 aggregate totalCost 求和 + costUnknown 计数（与 token 同口径：成功且非 retry 且解析到 usage）、S5 byConversation cost（全 unknown → null）、S6 RecentRequest.cost ＋ 旧行（无 costTotal）反序列化兼容、S7 CSV 追加 costTotal 列 ＋ JSON 序列化 costTotal（旧行 null）
- 测试结果：Rust 129 全绿（含求和/unknown 计数/对话聚合/旧行兼容/导出列）
- typecheck：通过（cargo check --lib）
- 遗留 / 后续建议：`total_cost: Option<f64>` 与 `cost_unknown: u64` 在 Rust/TS 双端镜像，如需可封装成单一类型
