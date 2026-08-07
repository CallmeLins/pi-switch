# 01 — 按供应商显示明细列

**What to build:** Stats 面板「按供应商」表格从单一 Token 列升级为明细列：输入 / 输出 / 缓存 / 总 / 缓存率 / 消费价格（合计 / 确定 / 成功率保留）。消费价格 = 该供应商成功、非重试、有 usage 的行中 `costTotal` 之和；全部未知显示 `-`。旧日志行（无消费价格）正常渲染。

**Blocked by:** None — can start immediately

**Status:** ready-for-agent

- [ ] 「按供应商」表格显示 输入/输出/缓存/总/缓存率/消费价格 六列，数值与请求日志口径一致（成功非重试有 usage 的行）
- [ ] 缓存率 = 缓存 / 输入：无输入 `-`、0 缓存 `0.0%`、否则一位小数百分比
- [ ] 消费价格 = 已知 cost 行之和，全部未知显示 `-`
- [ ] 旧数据（无 cost 字段的日志行）正常渲染，不影响其他统计
- [ ] 全量测试绿（cargo test；webui vitest 以 NODE_ENV=test 运行）
