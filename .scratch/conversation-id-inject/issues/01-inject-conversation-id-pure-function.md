# 01 — 对话标识注入纯函数与单元测试

**What to build:** 把"为请求头写入当前对话标识"的核心逻辑做成不依赖 pi 运行时的纯函数，并用项目现有 JS 测试运行器建立单测。端到端行为：给定一组请求头与一个对话标识，函数正确设置 `x-conversation-id`（或按规则跳过），返回值不污染调用方对象。本片为 02 的接线提供经过验证的注入逻辑，同时建立 JS/TS 层测试先例。

**Blocked by:** None — can start immediately

**Status:** resolved

- [ ] 对话标识非空时，`x-conversation-id` 被设置为该值，且覆盖请求头中已有的同名值
- [ ] 对话标识为空或纯空白时，不注入，原请求头原样保留
- [ ] 除 `x-conversation-id` 外的其它请求头不受影响
- [ ] 函数返回新对象，不修改调用方传入的请求头对象
- [ ] 测试用项目现有 JS 测试运行器执行并全绿

## 实施总结
- 提交：`b1f6748` — feat(extensions): inject session conversation id into provider requests
- 实现的 seams：S1 非空 sessionId 注入并覆盖既有 `x-conversation-id`；S2 空/纯空白/undefined 跳过且原头保留；S3 其它头不受影响、返回新对象不污染入参
- 测试结果：`npm test` 5/5 全绿（node --test，node 26 type stripping 直跑 .ts）
- typecheck：node 加载验证通过（扩展模块可被 node --test 正常 import）；webui tsc 0 错误
- 遗留 / 后续建议：package.json engines 已从 >=20 提升至 >=23.6（node --test 直跑 .ts 需要）；`npm test` 脚本限定 `extensions/**/*.test.ts`，避免误扫 webui 的 vitest 测试
