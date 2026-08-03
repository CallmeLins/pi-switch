# 04 — 聚合扩展：总量 / 缓存率 / 对话分组 + 导出

**What to build:** 在 01 抽出的聚合函数上扩展 Token 使用量统计，统计接口响应体向后兼容追加（不改 API 路由）：

- 累计总量：总输入、总输出、总 token（仅成功且有使用量的行计入；重试/失败的中间行不计，避免同一请求重复累计）
- 缓存命中率：命中缓存输入 ÷ 总输入，字符串百分比；无缓存数据时显示 `-` 而非 `0%`
- 按对话分组：按 `conversationId` 聚合的列表（请求数、输入/输出累计、最近活跃时间），最近活跃倒序截取 Top 20，无标识请求合并为 `unlabeled` 一组
- by-provider 明细追加 token 累计列
- 请求日志行类型补上四个可选字段（旧行反序列化为空，向后兼容）
- 日志导出（JSON/CSV）顺带输出 token 使用量与对话标识列

**Blocked by:** 01 — Prefactor：抽出聚合纯函数

**Status:** ready-for-agent

- [ ] 统计接口返回 `totalTokens` / `cacheHitRate` / `byConversation` 与 by-provider token 列，旧字段不变
- [ ] 聚合单测全绿：累计、缓存率口径、Top 20 截断、unlabeled 合并、失败行不计、旧行兼容
- [ ] 无任何 token 数据的日志 → 总量为 0、缓存率显示 `-`、无空数组/空对象异常
- [ ] CSV 导出新增列可读且与 JSON 导出一致
