# 01 — 代理侧消费计算与日志写入

**What to build:** 每次代理请求完成时，按当时的 profile 模型单价（含分级档位）把消费折算出来，写入请求日志行。端到端行为：请求日志（Request Log）中带 token 使用量的行同时携带消费值；消费 = (输入 − 缓存命中) × 输入单价 + 缓存命中 × 缓存读取单价 + 输出 × 输出单价；分级档位按本次请求输入 token 量选择；模型未配置单价的行消费记为 unknown（不写消费值）。旧日志行不受影响。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 成功且解析到 token 使用量的请求，日志行写入消费值，且 (输入−缓存)×输入单价 + 缓存×缓存读取单价 + 输出×输出单价的折算正确
- [x] 分级档位：输入 token 量达到阈值时按对应档位单价计算
- [x] 模型未配置单价（cost 缺失）的行消费记为 unknown，不写消费值
- [x] 消费计算与 token 同口径：仅成功且非 retry 且解析到 usage 的行计入
- [x] 请求日志旧行（无消费字段）仍可正常解析，格式向后兼容
- [x] 相关单测全绿（消费折算、分级档位、缺单价 unknown、旧行兼容）

## 实施总结
- 提交：`f07fe1f` — feat: add cost tracking to request logs, stats aggregation and dashboards
- 实现的 seams：S1 compute_cost 基本折算（cached 子集按 cacheRead 单价）、S2 分级档位（按输入 token 选最高达标阈值）、S3 build_log_entry costTotal 写入（缺单价/缺 usage → null）＋ lookup_model_cost 按 provider 模型查单价
- 测试结果：Rust 129 全绿（含 compute_cost 折算/档位、lookup 缺单价 unknown、costTotal 写入与旧字段 null）
- typecheck：通过（cargo check --lib）
- 遗留 / 后续建议：`log_request` 位置参数偏多（既有模式，本次新增 cost 参数）；亚美分级金额（< $0.0001）格式化后显示 `$0`，双端规则一致，可后续改用有效数字策略
