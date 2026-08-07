# 01 — 按供应商显示明细列

**What to build:** Stats 面板「按供应商」表格从单一 Token 列升级为明细列：输入 / 输出 / 缓存 / 总 / 缓存率 / 消费价格（合计 / 确定 / 成功率保留）。消费价格 = 该供应商成功、非重试、有 usage 的行中 `costTotal` 之和；全部未知显示 `-`。旧日志行（无消费价格）正常渲染。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 「按供应商」表格显示 输入/输出/缓存/总/缓存率/消费价格 六列，数值与请求日志口径一致（成功非重试有 usage 的行）
- [x] 缓存率 = 缓存 / 输入：无输入 `-`、0 缓存 `0.0%`、否则一位小数百分比
- [x] 消费价格 = 已知 cost 行之和，全部未知显示 `-`
- [x] 旧数据（无 cost 字段的日志行）正常渲染，不影响其他统计
- [x] 全量测试绿（cargo test；webui vitest 以 NODE_ENV=test 运行）


## 实施总结
- 提交：`97ec078` — feat(stats): 按模型统计表格与按供应商明细列；`f9ac0d7` — docs: align README with per-model stats columns
- 实现的 seams：S1 by_model 聚合（ModelStats 扩展）、S2 by_provider cost/cacheRate 聚合、S3 By provider 表格六列、S4 By model 表格
- 验收标准：全部 `- [x]`（见上）
- 测试结果：cargo test 206 全绿；webui vitest 96 全绿（NODE_ENV=test）
- typecheck：cargo check + tsc 通过
- 文档对齐：README.md / README_ZH.md 统计接口描述补充 per-model 明细列
- 遗留 / 后续建议：by_provider 与 by_model 表格 JSX 结构相同，可后续抽公共组件（code-review minor）
