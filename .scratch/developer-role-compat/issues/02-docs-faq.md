# 02 — README 问题说明 + pi 配置修改建议（文档先行）

**What to build:** 在 README.md / README_ZH.md 的 FAQ 区各新增一条：描述 pi 对 `reasoning: true` 模型发 `developer` role 导致上游 400 的问题，并给出 pi 配置文件（`~/.pi/agent/models.json`）的修改建议（`compat.supportsDeveloperRole: false`）与 opencode 上游脱敏示例；同时提示持久化路径（`~/.pi-switch/config.json` profile.models compat，sync 原样透传）。本期不改代码。

**Status:** resolved

## 验收标准

- [x] README.md FAQ 新增英文条目（插在 "How does gateway routing work?" 之后），含问题描述、`models.json` 修改建议、sync 抹掉手动修改的注意、opencode 上游完整脱敏示例
- [x] README_ZH.md FAQ 镜像中文条目（对应位置），内容互译一致
- [x] 两个 README 的示例 JSON 块经 `JSON.parse` 校验合法
- [x] 源码零改动；commit 仅含 README.md + README_ZH.md（`bin/pi-switch.js` 版本号改动为工作区既有未提交变更，不在本期提交内）
- [x] 不描述未实现的 `settings.supportsDeveloperRole` 选项与 sync 自动注入行为

## 核对结果（2026-08-04）

- commit `docs: document developer-role 400 fix for pi models.json`（仅两个 README）
- JSON 校验：README.md 2 blocks / README_ZH.md 2 blocks，全部合法
- 示例使用用户提供的脱敏样本原文（apiKey 为占位 `pi-switch-proxy`、baseUrl 为本地代理地址、无真实密钥），仅将 cost 对象压缩为单行
- 代码实现（settings 选项 + sync 注入 + 单测）由 01 票承接，本期保持 `ready-for-agent` 不置 resolved
