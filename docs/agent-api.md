# Agent 调用指南：鉴权与 API 使用

> 面向智能体（AI Agent）的接口调用文档。指导智能体如何完成鉴权并调用 riben-server 的 REST API，
> 覆盖公开接口与管理接口。实现细节见 [development-plan.md](development-plan.md)，代码入口
> `server/src/routes/mod.rs`、`server/src/middleware/auth.rs`。
>
> **优先使用 MCP**：如果客户端支持 Model Context Protocol，直接连接 `/mcp` 即可，
> 管理员通过邮箱验证码登录后可用 26 个工具完成几乎全部管理操作，
> 无需自己处理 HTTP 细节。见 [mcp.md](mcp.md)。本文档适用于只能调用 REST 的场景。

## 1. 服务入口

| 环境 | 地址 | 说明 |
|---|---|---|
| 本地开发 | `http://127.0.0.1:9015` | `cargo run` 默认监听 `HOST:PORT`（见 `server/.env`） |
| 生产 | `https://riben.icu` | Caddy 反代 `/api/*`、`/mcp` → riben-server，同源无 CORS |
| MCP | `https://riben.icu/mcp` | Streamable HTTP；见 [mcp.md](mcp.md) |

所有 API 挂载在 `/api` 前缀下，例如健康检查为 `GET /api/health`。

## 2. 响应约定

### 2.1 统一响应包

成功与失败都是同一个 JSON 信封，**判断成功与否看 `code`，不是 HTTP 状态码**：

```json
// 成功
{ "code": 0, "data": { ... }, "message": "" }

// 失败（HTTP 状态码与业务 code 同时非零）
{ "code": 40100, "data": null, "message": "未登录或会话已过期" }
```

- `code = 0`：成功，`data` 为业务数据。
- `code != 0`：失败，`message` 为面向用户的中文提示，`data` 恒为 `null`。

### 2.2 业务码速查

| code | HTTP | 含义 | 典型场景 |
|---|---|---|---|
| 40000 | 400 | 请求体解析失败 | JSON 格式错误 |
| 40001 | 422 | 参数校验失败 | 标题超长、邮箱非法、URL 非法等 |
| 40100 | 401 | 未登录 / 会话过期 / 验证码错误 | 缺少或失效的凭证 |
| 40400 | 404 | 资源不存在 | slug 不存在、文章未发布、评论不存在 |
| 40900 | 409 | 冲突 | 标签重名 |
| 42900 | 429 | 触发限流 | 见 §4 |
| 50000 | 500 | 服务端错误 | 数据库 / Redis 故障 |

> 智能体处理建议：收到 `40100` 时先检查凭证是否携带正确，再考虑重新鉴权；收到 `42900` 时按提示退避重试，不要立刻重发。

## 3. 鉴权

系统有两条鉴权路径，**智能体应优先使用方式 A**。

### 方式 A：管理员 API Token（推荐）

服务器配置环境变量 `ADMIN_API_TOKEN` 后启用。携带该 token 的请求**绕过登录流程**，
以管理员身份（users 表首个用户）通过鉴权，且不依赖 Redis 会话——即使 Redis 故障也能管理站点。

请求头二选一（等效）：

```
x-admin-token: <token>
Authorization: Bearer <token>
```

- 适用于：`/api/admin/*` 全部接口、`/api/auth/me`、`/api/auth/logout`；
  以及 MCP 端点 `/mcp` 的请求头鉴权（跳过 MCP 会话内的邮箱登录）。
- 不适用于：不需要鉴权的公开接口（带了也无妨）。
- token 通过 GitHub Actions Secret（`ADMIN_API_TOKEN`）或服务器 `.env` 注入；
  **切勿提交进仓库、切勿写进日志**。

curl 示例：

```bash
curl -s https://riben.icu/api/admin/posts \
  -H "x-admin-token: $ADMIN_API_TOKEN"
```

### 方式 B：邮箱验证码登录（Cookie Session）

仅当没有 API Token 时采用，模拟浏览器登录流程。会话 Cookie 名固定为 `riben_session`。

1. **发送验证码**：`POST /api/auth/email-code`，body `{"email": "<管理员邮箱>"}`。
   验证码为 6 位数字，有效期与限流见 §4。注意：非管理员邮箱也返回成功（防枚举），但不会有邮件。
2. **登录**：`POST /api/auth/email-login`，body `{"email": "...", "code": "123456"}`。
   成功响应带 `Set-Cookie: riben_session=<uuid>; HttpOnly; SameSite=Lax; Max-Age=...`，
   同时 `data` 返回 `{ "id": 1, "username": "admin" }`。
3. **后续请求**：透传该 Cookie。会话有效期默认 7 天且滑动续期（每次访问自动延长）。

```bash
# 1. 发码
curl -s -X POST https://riben.icu/api/auth/email-code \
  -H 'Content-Type: application/json' \
  -d '{"email": "admin@example.com"}'

# 2. 登录（人工从邮箱读取验证码后）
curl -s -c cookies.txt -X POST https://riben.icu/api/auth/email-login \
  -H 'Content-Type: application/json' \
  -d '{"email": "admin@example.com", "code": "483920"}'

# 3. 带会话调用管理接口
curl -s -b cookies.txt https://riben.icu/api/admin/posts
```

4. **登出**：`POST /api/auth/logout`（需鉴权），删除服务端 session 并清除 Cookie。

> 智能体注意：验证码需要人工从邮箱读取，无法全自动完成。因此自动化任务请走方式 A；
> 方式 B 仅适合有邮箱访问权限的半自动场景。

### 凭证检查

`GET /api/auth/me`（需鉴权）返回当前用户 `{ "id": ..., "username": ... }`，
可用于调用前验证凭证是否有效。

## 4. 限流（Redis 计数器）

触发限流返回 `42900 / 429`「操作太频繁，请稍后再试」。各接口限额：

| 接口 | 维度 | 限额 |
|---|---|---|
| `POST /api/comments` | IP | 60 秒 1 次 |
| `POST /api/auth/email-login` | IP | 10 分钟 5 次（成功登录后清零） |
| `POST /api/auth/email-code` | IP | 10 分钟 5 次 |
| `POST /api/auth/email-code` | 邮箱 | 60 秒 1 次 + 1 小时 5 次 |
| 验证码错误 | 邮箱 | 连续错 5 次作废，需重新发送 |

管理接口（`/api/admin/*`）**无限流**，但写操作会失效缓存，请勿高频轮询列表。

## 5. 接口清单

### 5.1 公开接口（无需鉴权）

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/health` | 健康检查：`{ "mysql": bool, "redis": bool, "version": "x.y.z" }` |
| GET | `/api/posts` | 首页数据：`{ "posts": PostSummary[], "total_posts": n, "recorded_days": n }`（最新 6 篇已发布文章 + 统计） |
| GET | `/api/posts/archive` | 按月归档：`ArchiveMonth[]`，`month` 格式 `"2026-11"` |
| GET | `/api/posts/{slug}` | 文章详情（`content_html` 已由后端渲染并消毒）；未发布返回 404 |
| GET | `/api/posts/{slug}/comments` | 该文章已通过审核的评论树 `CommentNode[]`（两层） |
| POST | `/api/comments` | 发表评论（默认待审核 status=0），body 见下 |

`POST /api/comments` 请求体（`slug` 与 `post_id` 二选一）：

```json
{
  "slug": "first-snow",
  "parent_id": null,
  "name": "访客昵称",
  "email": "visitor@example.com",
  "url": "https://example.com",
  "content": "评论内容（Markdown 纯文本）"
}
```

校验规则：`name` 1–64 字符；`email` 合法邮箱 ≤190；`url` 合法 URL ≤190 可空；
`content` 1–2000 字符。成功后 `data` 为空对象 `{}`。

### 5.2 认证接口

| 方法 | 路径 | 鉴权 | 说明 |
|---|---|---|---|
| POST | `/api/auth/email-code` | 否 | 发送登录验证码，body `{"email": "..."}` |
| POST | `/api/auth/email-login` | 否 | 验证码登录，body `{"email": "...", "code": "123456"}`，Set-Cookie 建会话 |
| POST | `/api/auth/logout` | 是 | 销毁当前会话 |
| GET | `/api/auth/me` | 是 | 当前用户 `{ "id": 1, "username": "admin" }` |

### 5.3 管理接口（需鉴权：API Token 或会话 Cookie）

整组挂载在 `/api/admin` 下，未鉴权一律 401。分页参数统一为 `?page=1&per_page=20`
（per_page 最大 100）。

#### 文章

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/admin/posts?page=&per_page=` | 文章列表（含草稿），`{ "list": PostAdmin[], "total": n, ... }` |
| POST | `/api/admin/posts` | 新建文章 |
| PUT | `/api/admin/posts/{id}` | 更新文章 |
| DELETE | `/api/admin/posts/{id}` | 删除文章（含其评论），`data` 为 `{}` |

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

- `title`：1–190 字符（必填）。
- `content_md`：Markdown 源码，1–100000 字符（必填）。
- `sticker`：≤32 字符，可空（手账贴纸 emoji）。
- `status`：`0` 草稿 / `1` 发布（`status=1` 时写入 `published_at`，重复发布不覆盖首次发布时间）。
- `tag_ids`：标签 id 数组；**新建文章前须先 `GET /api/admin/tags` 拿到 id**。
- slug 由服务端根据标题自动生成（英文转 kebab-case，中文回退 `page-<随机>`），创建后不可改。
- 摘要 `excerpt` 由服务端从 Markdown 自动提取，不可手动传。
- 返回 `PostAdmin`：`{ "id", "slug", "title", "content_md", "status", "sticker", "published_at", "updated_at", "tags", "comment_count" }`。

#### 评论审核

评论状态：`0` 待审核（默认）/ `1` 已通过 / `2` 已拒绝。

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/admin/comments?status=&page=&per_page=` | 评论列表，`status` 过滤（0/1/2，缺省全部）；返回 `{ "list": CommentAdmin[], "total": n, "pending": n, ... }`，`pending` 为待审核总数 |
| PUT | `/api/admin/comments/{id}/approve` | 通过审核 |
| PUT | `/api/admin/comments/{id}/reject` | 拒绝 |

#### 标签

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/admin/tags` | 全部标签 `TagOut[]`：`{ "id", "name", "color" }` |
| POST | `/api/admin/tags` | 新建标签，body `{ "name": "日常", "color": "#f7a8b8" }`（name 1–64，color 1–16；重名 409） |
| PUT | `/api/admin/tags/{id}` | 修改标签，body `{ "name"?: "...", "color"?: "..." }`（至少一个字段） |
| DELETE | `/api/admin/tags/{id}` | 删除标签（文章关联一并移除，文章保留） |

#### 工具

| 方法 | 路径 | 说明 |
|---|---|---|
| POST | `/api/admin/preview` | Markdown 渲染预览，body `{ "md": "..." }` → `{ "html": "..." }`（与文章详情同一渲染管线） |

## 6. 智能体典型任务配方

### 发布一篇文章（完整流程）

```bash
BASE=https://riben.icu
AUTH="x-admin-token: $ADMIN_API_TOKEN"

# 1. 取标签 id
TAG_IDS=$(curl -s "$BASE/api/admin/tags" -H "$AUTH" | jq -c '[.data[] | select(.name=="日常") | .id]')

# 2. 创建并发布
curl -s -X POST "$BASE/api/admin/posts" -H "$AUTH" -H 'Content-Type: application/json' -d "{
  \"title\": \"初雪比往年早了一些\",
  \"content_md\": \"# 初雪\n\n今早拉开窗帘……\",
  \"sticker\": \"❄️\",
  \"status\": 1,
  \"tag_ids\": $TAG_IDS
}"
# → { "code": 0, "data": { "id": 42, "slug": "page-xxxxxx", ... } }

# 3. 验证公开可见
curl -s "$BASE/api/posts/page-xxxxxx" | jq '.code, .data.title'
```

### 审核新评论

```bash
# 看待审核队列
curl -s "$BASE/api/admin/comments?status=0" -H "$AUTH" | jq '.data.pending, .data.list[0]'

# 通过 / 拒绝
curl -s -X PUT "$BASE/api/admin/comments/123/approve" -H "$AUTH"
curl -s -X PUT "$BASE/api/admin/comments/124/reject"  -H "$AUTH"
```

### 巡检服务健康

```bash
curl -s "$BASE/api/health"   # → { "code": 0, "data": { "mysql": true, "redis": true, "version": "0.1.0" } }
```

## 7. 注意事项

1. **永远先看 `code`**：HTTP 200 也可能是业务错误（非零 code），HTTP 非 200 时 code = HTTP×100 + 序号。
2. **token 保密**：`ADMIN_API_TOKEN` 等价于管理员身份，不要写进会被提交的代码、文档或日志。
3. **评论默认待审核**：游客评论不会立刻公开显示，审核后才出现在 `GET /api/posts/{slug}/comments`。
4. **写后缓存自动失效**：文章/评论/标签的写操作会失效对应缓存，无需手动处理。
5. **本地联调**：本地后端默认 `http://127.0.0.1:9015`，Vite dev（`:5173`）已将 `/api/*` 代理到该端口。
6. **slug 只读**：创建文章后 slug 由服务端生成且不可修改；引用文章请保存返回的 `slug`。
