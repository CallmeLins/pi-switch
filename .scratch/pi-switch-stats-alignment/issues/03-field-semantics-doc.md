# requests.log 字段语义文档（ticket 03）

**Type**: task
**Status**: ready-for-agent（纯文档，无代码冲突）

## 问题

`requests.log` 的 `promptTokens` 字段名有误导性：实际为「含缓存命中的总输入」（≡ pi 会话 usage 的 `input + cacheRead`）。对账时若按 `prompt+cached+completion` 三和相加，缓存被重复计入（实测虚增约 1.05B）。字段语义无权威文档，第三方工具易误消费。

## 方案

在网关文档（README 或 docs/ 下统计/日志章节）新增 requests.log 字段说明：

- 字段映射表：`promptTokens` ≡ pi `input+cacheRead`；`completionTokens` ≡ pi `output`；`cachedTokens` ≡ pi `cacheRead`；`reasoningTokens` ≡ pi `reasoning`；
- 网关总量公式：`promptTokens + completionTokens`（勿三和相加）；
- 标注 `ts` 为 UTC RFC3339、`retryOf` 为可选字段（上线后）；
- 不改字段名，保持向后兼容。

## 完成标准

- [ ] 文档发布到 pi-switch 仓库（README 或 docs/）
- [ ] 包含字段映射表与总量公式
- [ ] 标注时间戳时区语义

## Comments
