# 智能体配置指南：让 AI Agent 管理本站

> 面向「如何给 riben 项目配置一个智能体（AI Agent）」的文档：介绍在各类客户端
> （ZCode、Claude Code、Cursor 及任意能调用 HTTP 的 Agent）中接入本站的步骤、
> 变量约定与验证方法。接口明细见 [agent-api.md](agent-api.md)，MCP 工具清单见 [mcp.md](mcp.md)。

## 1. 配置总览

给本项目配置智能体只需三步：

| 步骤 | 内容 | 适用 |
|---|---|---|
| ① 获取凭证 | 从服务端管理员处取得管理员 API Token（服务端环境变量 `ADMIN_API_TOKEN` 的值） | 全部方式 |
| ② 接入通道 | 配置 MCP 连接（推荐）或 REST 直连 | 二选一 |
| ③ 验证 | 用健康检查与凭证检查确认配置生效 | 全部方式 |

两个接入通道：

- **MCP（推荐）**：连接 `https://riben.icu/mcp`（Streamable HTTP），带 token 请求头即可
  免登录使用全部 26 个管理工具，无需关心 HTTP 细节。
- **REST 直连**：智能体直接调用 `/api/*` 接口，适合不支持 MCP 的运行环境。

## 2. 核心约定：`RIBENICU_ADMIN_TOKEN`

本项目约定：**智能体侧存放管理员 token 的字段 / 变量名统一为 `RIBENICU_ADMIN_TOKEN`**。

- `RIBENICU_ADMIN_TOKEN` 是智能体侧（配置文件、环境变量、shell 会话）的字段名，
  其**值**就是服务端配置的 `ADMIN_API_TOKEN`（部署方在服务器 `.env` 或
  GitHub Actions Secret `ADMIN_API_TOKEN` 中设置的那个字符串）。
- 发送到服务器时，该值放进请求头 `x-admin-token` 或 `Authorization: Bearer <token>`；
  服务器不认识名为 `RIBENICU_ADMIN_TOKEN` 的请求头或环境变量，只认识头里的值。
- 之所以单独命名：智能体常同时管理多个站点，统一带项目前缀的字段名可避免不同站点的
  管理凭证互相覆盖、误发。
- 该 token 等价于管理员身份（users 表首个用户），**切勿写进会提交的代码、文档或日志**。

```
智能体配置                        服务端
RIBENICU_ADMIN_TOKEN = "xxxx"  ──x-admin-token: xxxx──▶  ADMIN_API_TOKEN 校验通过 → 管理员身份
（字段名只在智能体侧使用）          （或 Authorization: Bearer xxxx）
```

## 3. 方式 A：配置 MCP 客户端（推荐）

### 3.1 ZCode / Claude Code（命令行接入）

```bash
claude mcp add --transport http riben https://riben.icu/mcp \
  --header "x-admin-token: ${RIBENICU_ADMIN_TOKEN}"
```

其中 `${RIBENICU_ADMIN_TOKEN}` 替换为 token 实际值（或先 `export RIBENICU_ADMIN_TOKEN=...`
再执行）。验证与移除：

```bash
claude mcp list      # 应看到 riben 处于 connected
claude mcp remove riben
```

### 3.2 Cursor / VS Code 等图形客户端

在客户端的 MCP 配置（如 `.cursor/mcp.json`）中加入。支持变量插值的客户端直接引用
`RIBENICU_ADMIN_TOKEN` 字段；不支持的客户端把值粘贴到 `headers` 中：

```json
{
  "mcpServers": {
    "riben": {
      "type": "http",
      "url": "https://riben.icu/mcp",
      "headers": {
        "x-admin-token": "${RIBENICU_ADMIN_TOKEN}"
      }
    }
  }
}
```

> 字段名以客户端文档为准（有的用 `serverUrl`、`"transport": "http"` 等），
> 服务端只要求 Streamable HTTP 类型。带 token 头即可跳过邮箱验证码登录；
> 不带 token 头则需在 MCP 会话内走邮箱验证码登录（见 [mcp.md](mcp.md) §2）。

### 3.3 本地开发环境

本地后端默认 `http://127.0.0.1:9015/mcp`，把配置里的 `url` 换成该地址即可，其余不变。

## 4. 方式 B：配置 REST 直连

适用于只能发 HTTP 请求的智能体。约定智能体的运行环境里定义环境变量 / 配置字段
`RIBENICU_ADMIN_TOKEN`，请求时把它放入鉴权头：

```bash
# 智能体侧配置（如 ~/.zshrc、任务环境的 secrets、CI 变量等）
export RIBENICU_ADMIN_TOKEN="请替换为服务端 ADMIN_API_TOKEN 的值"

# 调用时引用该字段名，而不是硬编码 token
curl -s https://riben.icu/api/auth/me \
  -H "x-admin-token: $RIBENICU_ADMIN_TOKEN"
```

- 凭证检查通过后，即可调用全部 `/api/admin/*` 接口（发文章、审评论、管标签等），
  完整接口清单与任务配方见 [agent-api.md](agent-api.md)。
- 也可以改用 `Authorization: Bearer $RIBENICU_ADMIN_TOKEN`，两者等效。
- 响应统一为 `{ "code": ..., "data": ..., "message": ... }` 信封，
  **判断成功看 `code` 是否为 0**，详见 [agent-api.md](agent-api.md) §2。

### 本地 / 生产端点

| 环境 | REST 基址 | MCP |
|---|---|---|
| 生产 | `https://riben.icu/api` | `https://riben.icu/mcp` |
| 本地 | `http://127.0.0.1:9015/api` | `http://127.0.0.1:9015/mcp` |

## 5. 配置验证清单

配置完成后依次确认：

```bash
# 1. 服务健康（无需鉴权）
curl -s https://riben.icu/api/health
# → { "code": 0, "data": { "mysql": true, "redis": true, "version": "x.y.z" } }

# 2. token 有效（对应 RIBENICU_ADMIN_TOKEN 的值）
curl -s https://riben.icu/api/auth/me \
  -H "x-admin-token: $RIBENICU_ADMIN_TOKEN"
# → { "code": 0, "data": { "id": 1, "username": "admin" } }
```

- 第 2 步返回 `40100`：token 缺失或错误，检查 `RIBENICU_ADMIN_TOKEN` 的值是否与服务端
  `ADMIN_API_TOKEN` 一致（注意有无多余空白、换行、引号）。
- MCP 客户端接入验证：让智能体调用 `health_check` 与 `auth_whoami` 工具，
  带 token 头时 `auth_whoami` 应直接报告已登录。

## 6. 智能体行为约定

给智能体的系统提示 / 任务说明中建议写入以下规则（均为项目既有约定，细节见各文档）：

1. 写操作前先读：新建文章前先 `GET /api/admin/tags`（或 MCP `admin_list_tags`）拿标签 id。
2. 收到 `42900` 按提示退避重试，不要立刻重发；管理接口无限流但勿高频轮询。
3. `slug` 由服务端生成且不可修改，创建文章后保存返回的 `slug` 用于后续引用。
4. 游客评论默认待审核（status=0），审核后才公开；回复用 `admin_reply_comment`（MCP）。
5. `RIBENICU_ADMIN_TOKEN` 的值不写入日志、不回显到对话、不提交进任何文件。

## 7. 常见问题

| 现象 | 排查 |
|---|---|
| REST 返回 `40100` | `RIBENICU_ADMIN_TOKEN` 未定义、值为空或与 `ADMIN_API_TOKEN` 不一致 |
| MCP 报「未登录」 | 请求头没带 token 或值不对；带对后无需会话内邮箱登录 |
| MCP `403 Forbidden` / Invalid Host | 服务端 `MCP_ALLOWED_HOSTS` 未包含站点域名，见 [mcp.md](mcp.md) §5 |
| 本地能连、生产 401 | 生产服务器未配置 `ADMIN_API_TOKEN`（GitHub Secret 或服务器 `.env`） |
| token 想作废 | 更换服务端 `ADMIN_API_TOKEN` 并重新部署，所有智能体同步换新值 |
