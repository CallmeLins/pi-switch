# 推理 token 是输出 token 的子集，总数不重复累加

统计要展示四个 token 维度（输入/输出/缓存/推理），但推理 token 的来源决定了它不能与输入输出并列相加：OpenAI 系（含 DeepSeek）在 `completion_tokens_details.reasoning_tokens` 报告推理 token，而该数值已包含在 `completion_tokens` 内；Anthropic 则完全不提供细分。因此决定：推理 token 取上游细分子段，缺失记 0，语义上是输出 token 的子集；`total = input + output`，缓存与推理都不参与合计。展示上四个维度平铺，缓存/推理以角标注明子集关系。备选方案（四数并列相加）会造成 Anthropic 数据与 OpenAI 数据口径不一致，且 total 会随上游报告能力不同而漂移。
