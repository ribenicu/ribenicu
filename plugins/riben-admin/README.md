# riben-admin

管理一本日记博客（riben.icu）的 ZCode 插件。触发关键词：riben、手账、日记、riben.icu、一本日记、发到博客、审评论。

功能（通过 riben-server REST API）：

- 写手账 / 发文章：创建、修改、发布与转草稿，Markdown 正文，标签与贴纸
- 审评论：查看待审核队列，通过 / 拒绝，以站长身份回复
- 站点状态：health、文章列表、待审评论数

鉴权：环境变量 `RIBENICU_ADMIN_TOKEN`（值为服务端 `ADMIN_API_TOKEN`），
请求头 `x-admin-token`。详见 `skills/riben-admin/references/api.md`。
