# 05 — WebUI 展示

**What to build:** 统计面板展示 Token 使用量统计：新增「Tokens」与「Cache 率」指标卡片（总量可读格式化，如 12.3M）；by-provider 表格追加 token 列；新增 "By conversation" 卡片——按最近活跃倒序的对话列表（短标识、请求数、累计 tokens），未标记请求合并为一行显示。

**Blocked by:** 04 — 聚合扩展

**Status:** ready-for-agent

- [ ] 四个统计展示元素（两卡片 + by-provider 列 + 对话列表）在无数据/有数据/仅有旧数据三种状态下渲染正确
- [ ] 缓存率无数据时显示 `-`，不显示误导的 `0%`
- [ ] 前端类型与统计接口新字段一致
- [ ] 现有统计功能（请求数/成功率/导出）不回归
