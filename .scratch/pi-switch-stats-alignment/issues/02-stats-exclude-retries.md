# stats 排除重试选项 --exclude-retries（ticket 02）

**Type**: task
**Status**: ready-for-agent（stats.rs 与并行项目无冲突，可先行）

## 问题

用户需要对账时得到与 pi 会话口径一致的网关统计——即排除 pi 重试请求（`retryOf` 非空的条目）。默认统计口径保持「真实消耗」（含重试）不变。

## 方案

- stats 增加 `--exclude-retries` 选项（及等价 webui 参数，可选）；
- 生效时跳过 `retryOf` 非空的日志条目；未生效时行为与现状完全一致；
- 在 `retryOf` 字段上线前，选项对旧日志无效果（无字段即无排除）；
- CLI 帮助文本注明语义：排除后口径 = pi 会话口径（总输入+输出）。

## 完成标准

- [ ] `--exclude-retries` 生效时排除 `retryOf` 非空条目，其余统计（请求数/token/成本）同步变化
- [ ] 默认行为不变（无参数时输出与现状一致）
- [ ] 单测覆盖排除与不排除两种路径
- [ ] cargo test 全绿

## Comments
