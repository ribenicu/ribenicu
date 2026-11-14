---
name: riben-admin
description: 管理一本日记博客（riben.icu）：写手账/发文章、草稿与发布、审核与回复评论、管理标签、查看站点数据与健康状态。只要用户想发布、修改或删除博客文章，提到「发手账」「写日记」「发到博客」「审评论」「riben」「一本日记」，或想检查 riben.icu 服务状态，就使用本技能——即使没有明说「管理博客」。
---

# riben-admin：管理一本日记（riben.icu）

通过 riben-server 的 REST API 管理站点。鉴权凭证从**环境变量 `RIBENICU_ADMIN_TOKEN`** 读取，
其值即服务端 `ADMIN_API_TOKEN`；调用时把该值放进请求头 `x-admin-token`（或
`Authorization: Bearer`）。完整接口细节、参数校验与错误码见
[references/api.md](references/api.md)，本文件只保留工作流。

## 第 0 步：拿到凭证与基址

```bash
# 读取约定变量；不要把它的值写进任何文件、日志或对话回复
echo "${RIBENICU_ADMIN_TOKEN:+set}"   # 只确认是否已设置，不回显值
```

- 变量已设置 → 直接用 `BASE=https://riben.icu/api`（生产）。
- 本地联调（用户说本地/开发环境）→ `BASE=http://127.0.0.1:9015/api`。
- 变量为空 → 告诉用户：请先 `export RIBENICU_ADMIN_TOKEN=<服务端 ADMIN_API_TOKEN 的值>`，
  或直接粘贴 token 让你临时使用（临时的不得写入任何文件）。**停止等待，不要猜测。**

## 第 1 步：验证凭证（每次会话一次）

```bash
curl -s "$BASE/auth/me" -H "x-admin-token: $RIBENICU_ADMIN_TOKEN"
```

返回 `{"code":0,"data":{"id":1,"username":"admin"}}` 即可继续；`code` 非 0 时
见 references/api.md §错误码排查，常见原因是值不一致（空白/换行）或服务端未配置。

## 通用约定

1. **看 `code` 不看 HTTP 状态码**：所有响应都是 `{ "code": 0, "data": ..., "message": "" }`；
   `code != 0` 即失败，把 `message`（中文提示）转述给用户，`data` 恒为 null。
2. 收到 `42900`（限流）按提示等待后重试一次，仍失败则报告用户。
3. token 只通过 `-H "x-admin-token: $RIBENICU_ADMIN_TOKEN"` 引用环境变量，
   不在命令行中硬编码 token 值（避免进入 shell history）。为省事可在会话内设
   `AUTH="x-admin-token: $RIBENICU_ADMIN_TOKEN"` 后复用。
4. 文章 `slug` 由服务端生成且不可改；发布成功后**必须把返回的 slug 和
   `https://riben.icu/post.html?p=<slug>` 链接告诉用户**。

## 任务 1：写手账 / 发文章

新文章（用户给标题和内容，Markdown）：

```bash
# 1. 取标签列表，按用户提到的标签名找 id；没有的标签先创建（见 references/api.md §标签）
curl -s "$BASE/admin/tags" -H "$AUTH"   # AUTH="x-admin-token: $RIBENICU_ADMIN_TOKEN"

# 2. 创建文章。status=1 直接发布；用户说「存草稿/先不发布」则 status=0
curl -s -X POST "$BASE/admin/posts" -H "$AUTH" -H 'Content-Type: application/json' -d '{
  "title": "标题（1–190 字符，必填）",
  "content_md": "# Markdown 正文（1–100000 字符，必填）",
  "sticker": "❄️",
  "status": 1,
  "tag_ids": [1]
}'
```

- 用户没提标签就不传 `tag_ids`；没提贴纸就不传 `sticker`，不要自作主张编造标签名。
- `excerpt`（摘要）由服务端自动提取，不要传。
- 改文章：`PUT /api/admin/posts/{id}`（只传要改的字段，标签整体替换）。
- 发布 / 转草稿：`PUT .../{id}` 改 `status`，或 MCP 场景下的 publish/unpublish。
- 发布前用户若想确认排版，可 `POST /api/admin/preview` 渲染 HTML 给用户看。

## 任务 2：审评论

```bash
# 待审核队列（先看 pending 总数）
curl -s "$BASE/admin/comments?status=0" -H "$AUTH"
# 通过 / 拒绝
curl -s -X PUT "$BASE/admin/comments/<id>/approve" -H "$AUTH"
curl -s -X PUT "$BASE/admin/comments/<id>/reject"  -H "$AUTH"
```

- 呈现评论给用户时引用 `name`、`content` 与所属文章；批量处理前先列清单让用户确认。
- 以站长身份回复评论用 MCP 工具 `admin_reply_comment`（REST 无此接口）。

## 任务 3：查站点状态

```bash
curl -s "$BASE/health"        # mysql / redis / version
curl -s "$BASE/admin/posts?page=1&per_page=10" -H "$AUTH"   # 文章列表（含草稿）
curl -s "$BASE/admin/comments?status=0" -H "$AUTH"          # 待审核评论
```

用户问「最近写了什么」「有几篇文章」「有没有新评论待审」时走这一节，不要公开接口一个个拼。

## 什么时候读 references/api.md

遇到以下情况先读 [references/api.md](references/api.md) 再操作：

- 需要字段级校验规则（标题/邮箱/评论长度限制、tag 颜色格式等）
- 创建/修改标签、删除文章、按月归档、公开评论树等本文件未展开的接口
- `code` 非 0 需要排查（错误码表在 §响应与错误码）
