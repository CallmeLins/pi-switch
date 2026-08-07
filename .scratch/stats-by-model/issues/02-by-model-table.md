# 02 — 按模型统计表格

**What to build:** Stats 面板新增「按模型」统计表格，与「按供应商」并列，显示 名称 / 合计 / 确定 / 成功率 / 输入 / 输出 / 缓存 / 总 / 缓存率 / 消费价格。字段语义与「按供应商」相同（成功非重试有 usage 的行；消费价格 = 已知 cost 行之和，全部未知 `-`）。

**Blocked by:** 01 — 两票改动同一统计区域（数据聚合与面板渲染），串行避免冲突；无逻辑依赖

**Status:** resolved

- [x] 「按模型」表格显示 合计/确定/成功率 + 输入/输出/缓存/总/缓存率/消费价格
- [x] 数值与请求日志口径一致，缓存率与消费价格语义与「按供应商」相同
- [x] 旧数据（无 cost 字段）正常渲染
- [x] 全量测试绿（cargo test；webui vitest 以 NODE_ENV=test 运行）


## 实施总结
- 提交：`97ec078` — feat(stats): 按模型统计表格与按供应商明细列；`f9ac0d7` — docs: align README with per-model stats columns
- 实现的 seams：S1 by_model 聚合（ModelStats 扩展）、S2 by_provider cost/cacheRate 聚合、S3 By provider 表格六列、S4 By model 表格
- 验收标准：全部 `- [x]`（见上）
- 测试结果：cargo test 206 全绿；webui vitest 96 全绿（NODE_ENV=test）
- typecheck：cargo check + tsc 通过
- 文档对齐：README.md / README_ZH.md 统计接口描述补充 per-model 明细列
- 遗留 / 后续建议：by_provider 与 by_model 表格 JSX 结构相同，可后续抽公共组件（code-review minor）
