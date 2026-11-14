# MCP 服务：管理员通过邮箱登录后管理站点

`riben-server` 内置一个标准的 **MCP（Model Context Protocol）** 服务，使用官方 Rust SDK
[`rmcp`](https://github.com/modelcontextprotocol/rust-sdk) 的 **Streamable HTTP** 传输，
与 REST API 共用同一个进程、同一套业务层和同一个数据库。

管理员在 MCP 客户端里完成一次**邮箱验证码登录**后，即可通过工具完成文章、评论、标签、
统计等几乎全部后台操作，无需再打开管理页面。

- 端点：`https://riben.icu/mcp`（本地：`http://127.0.0.1:9015/mcp`）
- 传输：Streamable HTTP（`POST /mcp` JSON-RPC、`GET /mcp` SSE 通知流、`DELETE /mcp` 结束会话）
- 协议版本：支持到 `2025-11-25`（原因见 §7）
- 鉴权：MCP 会话内邮箱验证码登录；也支持 `x-admin-token` 请求头

---

## 1. 在 MCP 客户端中接入

### 1.1 Claude Code

```bash
# 远程 HTTP 接入
claude mcp add --transport http riben https://riben.icu/mcp

# 查看 / 移除
claude mcp list
claude mcp remove riben
```

### 1.2 Cursor / VS Code / 其他支持 HTTP 的客户端

在客户端的 MCP 配置（如 Cursor 的 `.cursor/mcp.json`）中加入：

```json
{
  "mcpServers": {
    "riben": {
      "type": "http",
      "url": "https://riben.icu/mcp"
    }
  }
}
```

> 不同客户端的字段名略有差异：有的用 `url`，有的用 `serverUrl` 或 `"transport": "http"`。
> 以客户端文档为准；本服务只要求它是 **Streamable HTTP** 类型，而不是 stdio 或旧版 SSE。

### 1.3 使用管理员 API Token 免登录（可选）

如果不想每次走邮箱验证码，可以在客户端配置固定请求头（`ADMIN_API_TOKEN` 需在服务端配置）：

```json
{
  "mcpServers": {
    "riben": {
      "type": "http",
      "url": "https://riben.icu/mcp",
      "headers": {
        "x-admin-token": "请替换为服务端 ADMIN_API_TOKEN"
      }
    }
  }
}
```

也支持 `Authorization: Bearer <token>`。该 token 等价于管理员身份，请只放在本机配置或密钥管理中。
使用 API Token 时，`auth_logout` 只清除当前 MCP 会话的缓存身份，下次请求仍会按 token 重新鉴权；
如需停用，请更换或移除服务端的 `ADMIN_API_TOKEN`。

---

## 2. 登录流程（邮箱验证码）

MCP 的登录状态保存在**当前 MCP 会话**内，不同会话互不影响。

1. 对助手说：「调用 `auth_send_email_code`，邮箱是 admin@example.com」
   服务端向该邮箱发送 6 位验证码（5 分钟内有效）。
2. 从邮箱读取验证码，对助手说：「验证码是 123456，调用 `auth_login` 登录」。
3. 登录成功后，同一会话即可调用全部 `admin_*` 工具。
4. 需要退出时调用 `auth_logout`；随时可用 `auth_whoami` 查看登录状态。

发送验证码的限流（与网页登录一致）：

| 维度 | 限额 |
|---|---|
| 同一邮箱 | 60 秒 1 次 |
| 同一邮箱 | 1 小时 5 次 |
| 验证码错误 | 连续错 5 次作废，需重新发送 |

> 只有 `users` 表中已登记的管理员邮箱会真正收到邮件；其他邮箱不会发信，
> 接口也仍返回成功提示，避免邮箱枚举。

登录成功后服务端返回 `authenticated: true`、用户名与剩余有效期。
MCP 会话与网页 Session 一样默认 7 天滑动续期；服务重启会丢失内存中的 MCP 会话，
客户端重新 initialize 后需要重新登录（或改用 API Token 请求头）。

---

## 3. 工具清单

共 26 个工具，按前缀分组：

### 3.1 登录

| 工具 | 说明 |
|---|---|
| `auth_send_email_code` | 向管理员邮箱发送 6 位验证码 |
| `auth_login` | 邮箱 + 验证码登录（建立当前 MCP 会话身份） |
| `auth_logout` | 退出当前 MCP 会话 |
| `auth_whoami` | 查看当前会话是否已登录 |

### 3.2 公开读取（无需登录）

| 工具 | 说明 |
|---|---|
| `health_check` | MySQL / Redis / 版本健康检查 |
| `get_home` | 首页数据：最新 6 篇 + 文章总数 + 记录天数 |
| `list_archive` | 按月归档的已发布文章 |
| `get_post` | 按 slug 获取已发布文章详情（渲染后的 HTML） |
| `list_public_comments` | 某篇文章已通过审核的评论树 |

### 3.3 文章管理（需登录）

| 工具 | 说明 |
|---|---|
| `admin_list_posts` | 文章列表（含草稿），可按 `status` 过滤、分页 |
| `admin_get_post` | 按 ID 查看文章详情（含 Markdown 原文） |
| `admin_create_post` | 新建文章（默认草稿，`status=1` 立即发布） |
| `admin_update_post` | 部分更新文章字段；标签整体替换 |
| `admin_delete_post` | 删除文章及其评论 |
| `admin_publish_post` | 发布文章（保留其余字段） |
| `admin_unpublish_post` | 转为草稿（保留其余字段） |

文章参数约定：

- `title`：1–190 字符；`content_md`：1–100000 字符；`sticker`：≤32 字符。
- `status`：`0` 草稿 / `1` 已发布；新建默认 `0`，发布时服务端补 `published_at`。
- 标签用 `tag_ids`（ID 数组）或 `tag_names`（名称数组）指定；名称必须已存在，
  也可先用 `admin_list_tags` 查询。
- 更新时只传需要改的字段；`tag_ids: []` 或 `tag_names: []` 表示清空标签；
  `sticker: ""` 表示清除贴纸。
- slug 由服务端生成且不可修改，创建后请保存返回值中的 `slug`。

### 3.4 评论管理（需登录）

| 工具 | 说明 |
|---|---|
| `admin_list_comments` | 评论列表，按状态过滤（0 待审核 / 1 已通过 / 2 已拒绝） |
| `admin_approve_comment` | 通过审核，公开显示 |
| `admin_reject_comment` | 拒绝（软删除） |
| `admin_reply_comment` | 以站长身份回复评论，直接公开 |

### 3.5 标签管理（需登录）

| 工具 | 说明 |
|---|---|
| `admin_list_tags` | 全部标签（id / 名称 / 颜色） |
| `admin_create_tag` | 新建标签（名称不可重复） |
| `admin_update_tag` | 改名或改颜色；改名后文章上的标签同步更新 |
| `admin_delete_tag` | 删除标签；文章与标签的关联一并移除，文章本身保留 |

### 3.6 系统（需登录）

| 工具 | 说明 |
|---|---|
| `admin_site_stats` | 文章/评论/标签数量、记录天数、最近发布时间 |
| `preview_markdown` | 用与文章详情相同的消毒管线渲染 Markdown |

---

## 4. 典型对话

```text
你：调用 auth_send_email_code，邮箱 admin@example.com
助手：验证码已发送，请把邮件里的 6 位数字告诉我。

你：123456，调用 auth_login
助手：登录成功（用户名 admin）。当前站点：7 篇文章、2 条评论、8 个标签。

你：把《初雪的早晨》发成文章，贴纸 ❄️，标签「日常」和「城市」
助手：（先 admin_list_tags 找标签，再 admin_create_post）已创建并发布，
      slug 为 page-1a2b3c，链接 https://riben.icu/post.html?p=page-1a2b3c

你：看看有没有待审核评论
助手：（admin_list_comments status=0）有 1 条待审核，来自「小明」……

你：通过它，并以站长身份回复「谢谢喜欢」
助手：（admin_approve_comment + admin_reply_comment）已完成。
```

---

## 5. 配置

服务端环境变量：

| 变量 | 默认 | 说明 |
|---|---|---|
| `MCP_ENABLED` | `true` | 是否启用 `/mcp` |
| `MCP_ALLOWED_HOSTS` | `localhost,127.0.0.1,::1` | 允许的 `Host` 头（逗号分隔），防 DNS rebinding；生产必须追加站点域名，如 `riben.icu`；`*` 表示关闭校验（仅内网调试） |
| `MCP_ALLOWED_ORIGINS` | 空 | 允许的浏览器 `Origin`（逗号分隔）；空表示不校验 |
| `ADMIN_EMAIL` | 空 | 管理员邮箱；邮箱验证码登录必需 |
| `SMTP_*` | — | 发送验证码的 SMTP 配置 |
| `ADMIN_API_TOKEN` | 空 | 可选，MCP 请求头鉴权与 REST 管理接口共用 |

反向代理要求：

- 必须把 `/mcp` 与 `/mcp/*` 反代到 `riben-server`（本项目 `deploy/Caddyfile` 已配置）。
- SSE 响应不能被缓冲：Caddy 使用 `flush_interval -1`；Nginx 需关闭 `proxy_buffering`。
- 生产 HTTPS 下建议只暴露 `https://<域名>/mcp`，不要直接暴露后端端口。

---

## 6. 安全说明

1. **管理工具必须登录**：所有 `admin_*` 工具在未登录时返回登录提示，不会执行操作。
2. **邮箱验证码即登录凭证**：验证码 5 分钟有效、错误 5 次作废；请勿把验证码转发给他人或写入日志。
3. **API Token 等价于管理员密码**：只通过 Secret 注入，不要提交到仓库或出现在对话记录中。
4. **Host 白名单**：默认只允许本机回环地址；公网部署务必配置 `MCP_ALLOWED_HOSTS`，
   否则请求会因 `Host` 校验失败返回 403。
5. **多实例部署**：MCP 会话保存在单实例内存中，当前实现适合单机部署；
   若要多副本负载均衡，需使用粘性会话，或改用 `x-admin-token` 请求头鉴权。
6. **公开读取工具**：`health_check`、`get_home`、`list_archive`、`get_post`、
   `list_public_comments` 与对应的 REST 公开接口一样无需登录。

---

## 7. 协议版本说明

`rmcp` 支持到 `2026-07-28`，但该版本起 MCP 移除了会话（session）。
本服务需要「登录一次、同一会话后续调用都有效」的邮箱登录体验，因此刻意把协商上限
固定在仍带 `initialize` 握手与会话的最后一个版本 **`2025-11-25`**。
客户端会在 `initialize` 时自动协商到该版本，无需手动配置。

---

## 8. 本地调试

启动后端（`cargo run` 默认 `:9015`），确认健康检查：

```bash
curl -s http://127.0.0.1:9015/api/health
```

MCP 握手 + 工具列表（Streamable HTTP）：

```bash
# 1. initialize，记下响应头里的 mcp-session-id
curl -i -s http://127.0.0.1:9015/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"curl","version":"1"}}}'

# 2. 发送 initialized 通知（用上一步的 session id）
curl -i -s http://127.0.0.1:9015/mcp \
  -H 'Content-Type: application/json' \
  -H 'Mcp-Session-Id: <session-id>' \
  -d '{"jsonrpc":"2.0","method":"notifications/initialized"}'

# 3. 列出工具
curl -s http://127.0.0.1:9015/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -H 'Mcp-Session-Id: <session-id>' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
```

使用 API Token 直接调用管理工具：

```bash
curl -s http://127.0.0.1:9015/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -H 'Mcp-Session-Id: <session-id>' \
  -H 'x-admin-token: <ADMIN_API_TOKEN>' \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"admin_site_stats","arguments":{}}}'
```

---

## 9. 常见问题

| 现象 | 排查 |
|---|---|
| `403 Forbidden` / Invalid Host | `MCP_ALLOWED_HOSTS` 未包含实际访问域名（含反代后的 Host） |
| `401` / 会话不存在 | 没带 `Mcp-Session-Id`，或服务重启导致会话丢失；重新 initialize |
| 工具返回「未登录」 | 先 `auth_send_email_code` + `auth_login`，或配置 `x-admin-token` |
| 收不到验证码 | 检查 `ADMIN_EMAIL` 与库中 users.email 是否一致、SMTP 配置是否正确、是否触发限流 |
| 响应卡住 / 工具列表一直不返回 | 反向代理缓冲了 SSE；Caddy 加 `flush_interval -1`，Nginx 关闭 `proxy_buffering` |
| 更新文章后页面仍旧 | 写操作会自动失效 Redis 缓存；确认 Redis 可用，或等待 TTL 过期 |
