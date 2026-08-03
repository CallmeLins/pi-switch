# 01 — Prefactor：抽出聚合纯函数

**What to build:** 把统计聚合逻辑从"读取请求日志"中解耦出来：`get_stats()` 只负责读文件，聚合变成独立的纯函数 `aggregate(entries) -> UsageStats`，接收请求日志行列表、返回统计结果。行为与重构前完全一致——纯重构，不新增任何统计字段。

**Blocked by:** None — can start immediately

**Status:** ready-for-agent

- [ ] 聚合逻辑可独立调用：给定请求日志行列表即可得到统计结果，不触碰文件系统
- [ ] 重构前后统计结果一致（现有用例：总量/成功率/平均延迟/by-provider/by-model/熔断状态）
- [ ] 聚合函数带单测覆盖（空输入、单行、多行、异常行跳过）
- [ ] `cargo test` 全绿，Rust 侧构建通过
