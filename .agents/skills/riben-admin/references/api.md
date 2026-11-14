# riben-server REST API 速查（智能体版）

> 服务端 `ADMIN_API_TOKEN` 旁路鉴权：请求头 `x-admin-token: <token>` 或
> `Authorization: Bearer <token>`，智能体侧约定从环境变量 `RIBENICU_ADMIN_TOKEN` 取值。
> 权威文档：仓库 `docs/agent-api.md`；MCP 工具见 `docs/mcp.md`。

## 1. 基址与信封

| 环境 | REST 基址 |
|---|---|
| 生产 | `https://riben.icu/api` |
| 本地 | `http://127.0.0.1:9015/api` |

所有响应统一信封，**判断成败看 `code`**：

```json
{ "code": 0, "data": { }, "message": "" }
```

### 错误码

| code | HTTP | 含义 | 排查 |
|---|---|---|---|
| 40000 | 400 | JSON 解析失败 | 检查请求体格式、`Content-Type: application/json` |
| 40001 | 422 | 参数校验失败 | 对照下方各字段校验规则，`message` 会说明字段 |
| 40100 | 401 | 未鉴权 / 会话过期 / 验证码错误 | `RIBENICU_ADMIN_TOKEN` 未设置或值与服务端 `ADMIN_API_TOKEN` 不一致（注意首尾空白、引号）；服务端未配置该变量时旁路不生效 |
| 40400 | 404 | 资源不存在 | slug 不存在、文章未发布、评论/标签 id 错误 |
| 40900 | 409 | 冲突 | 标签重名 |
| 42900 | 429 | 限流 | 按提示等待后重试一次 |
| 50000 | 500 | 服务端错误 | MySQL/Redis 故障，报告用户稍后再试 |

### 限流表

| 接口 | 维度 | 限额 |
|---|---|---|
| `POST /comments` | IP | 60 秒 1 次 |
| `POST /auth/email-login` | IP | 10 分钟 5 次 |
| `POST /auth/email-code` | IP / 邮箱 | 10 分钟 5 次 / 60 秒 1 次且 1 小时 5 次 |
| 管理接口 | — | 无限流，但勿高频轮询（写操作会失效缓存） |

## 2. 公开接口（无需鉴权）

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/health` | `{ "mysql": bool, "redis": bool, "version": "x.y.z" }` |
| GET | `/posts` | 首页：最新 6 篇 `posts` + `total_posts` + `recorded_days` |
| GET | `/posts/archive` | 按月归档，`month` 格式 `"2026-11"` |
| GET | `/posts/{slug}` | 文章详情（`content_html` 已渲染消毒）；未发布返回 404 |
| GET | `/posts/{slug}/comments` | 已通过审核的评论树（两层 `CommentNode`） |
| POST | `/comments` | 发表评论，默认待审核 |

`POST /comments` 请求体（`slug` 与 `post_id` 二选一）：

```json
{ "slug": "page-xxxxxx", "parent_id": null, "name": "昵称", "email": "a@b.c", "url": "", "content": "Markdown 纯文本" }
```

校验：`name` 1–64；`email` 合法邮箱 ≤190；`url` 合法 URL ≤190 可空；`content` 1–2000。

## 3. 认证接口

| 方法 | 路径 | 鉴权 | 说明 |
|---|---|---|---|
| GET | `/auth/me` | 是 | 凭证检查，`data`: `{ "id": 1, "username": "admin" }` |
| POST | `/auth/logout` | 是 | 销毁当前会话（对 token 旁路无意义） |
| POST | `/auth/email-code` | 否 | 发登录验证码（仅管理员邮箱真发信） |
| POST | `/auth/email-login` | 否 | 验证码登录，`Set-Cookie: riben_session`（会话 7 天滑动续期） |

> 智能体自动化任务一律用 token 旁路；邮箱验证码流程需要人工读信，仅半自动场景使用。

## 4. 管理接口（token 旁路或会话 Cookie）

分页统一 `?page=1&per_page=20`，`per_page` 最大 100。

### 文章

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/admin/posts?page=&per_page=` | 列表（含草稿）：`{ "list": PostAdmin[], "total": n, ... }` |
| POST | `/admin/posts` | 新建文章 |
| PUT | `/admin/posts/{id}` | 更新文章（部分字段，标签整体替换） |
| DELETE | `/admin/posts/{id}` | 删除文章及其评论，`data` 为 `{}` |

新建/更新请求体：

```json
{
  "title": "初雪",
  "content_md": "# 正文\nMarkdown 源码",
  "sticker": "❄️",
  "status": 1,
  "tag_ids": [1, 3]
}
```

- `title` 必填 1–190；`content_md` 必填 1–100000；`sticker` ≤32 可空；`status` `0` 草稿 / `1` 发布。
- `status=1` 写入 `published_at`，重复发布不覆盖首次发布时间。
- REST 的文章接口只接受 `tag_ids`（id 数组）；按名称指定标签（`tag_names`）仅 MCP 工具支持，REST 场景请先查 §标签 拿 id。
- `slug` 服务端生成（英文 kebab-case，中文回退 `page-<随机>`），创建后不可改；`excerpt` 自动提取。
- 返回 `PostAdmin`：`{ "id", "slug", "title", "content_md", "status", "sticker", "published_at", "updated_at", "tags", "comment_count" }`。

### 评论审核

状态：`0` 待审核 / `1` 已通过 / `2` 已拒绝。

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/admin/comments?status=&page=&per_page=` | 列表；`pending` 为待审核总数 |
| PUT | `/admin/comments/{id}/approve` | 通过 |
| PUT | `/admin/comments/{id}/reject` | 拒绝（软删除） |

### 标签

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/admin/tags` | 全部标签 `{ "id", "name", "color" }` |
| POST | `/admin/tags` | 新建 `{ "name": "日常", "color": "#f7a8b8" }`（name 1–64，color 1–16，重名 409） |
| PUT | `/admin/tags/{id}` | 修改 `{ "name"?, "color"? }`（至少一项） |
| DELETE | `/admin/tags/{id}` | 删除标签，文章关联一并移除，文章保留 |

### 工具

| 方法 | 路径 | 说明 |
|---|---|---|
| POST | `/admin/preview` | Markdown 预览 `{ "md": "..." }` → `{ "html": "..." }`（与文章详情同一渲染管线） |

## 5. 发布文章标准流程

```bash
BASE=https://riben.icu/api
AUTH="x-admin-token: $RIBENICU_ADMIN_TOKEN"

# 1. 标签 id（按需）
curl -s "$BASE/admin/tags" -H "$AUTH"

# 2. 创建并发布
curl -s -X POST "$BASE/admin/posts" -H "$AUTH" -H 'Content-Type: application/json' -d '{
  "title": "…", "content_md": "…", "sticker": "❄️", "status": 1, "tag_ids": [1]
}'
# → data.slug

# 3. 公开验证
curl -s "$BASE/posts/<slug>"
```
