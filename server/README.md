# 一本日记 · 后端（riben-server）

Rust 单二进制服务：axum + sqlx(MySQL) + redis-rs。负责 REST API（`/api/*`）、MCP 服务（`/mcp`）、Markdown 渲染与 HTML 消毒、Session 鉴权、Redis 缓存与限流，可选托管前端静态资源。

## 快速开始

```bash
cp .env.example .env   # 配置 DATABASE_URL / REDIS_URL / ADMIN_EMAIL / SMTP_* 等
cargo run              # 默认 0.0.0.0:9015；启动时自动执行 migrations/
```

> **注意：migration 只建表，不建库。** 本地直连 MySQL 开发时需先手动创建数据库，否则服务会以 lazy 池静默启动（不报错退出），随后 migration 失败、接口不可用：
>
> ```sql
> CREATE DATABASE riben CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
> ```
>
> 使用根目录 `deploy/docker-compose.yml` 部署时无需此步骤——compose 的 `MYSQL_DATABASE` 变量会在 MySQL 容器首次启动时自动创建数据库。

首次启动且 `users` 表为空时按 `ADMIN_USERNAME` / `ADMIN_EMAIL` 播种管理员（登录方式为邮箱验证码）；`SEED_DEMO=true`（默认）且文章表为空时写入 6 篇演示文章（原静态站内容）。

## 质量门槛

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test
```

## 结构

```
src/
├── main.rs       # 启动：配置 → MySQL 池 → migration → Redis（可降级）→ 路由
├── config.rs     # 环境变量配置（dotenvy 加载 .env）
├── db.rs         # 连接池（lazy 兜底）+ migration + 种子数据
├── error.rs      # AppError → 统一 JSON 错误体
├── state.rs      # AppState（MySqlPool + Redis + Config）
├── util.rs       # slug 生成、摘要提取、comrak+ammonia 渲染
├── middleware/   # auth.rs（Session 鉴权）、rate_limit.rs（Redis 计数器限流）
├── routes/       # post / comment / auth / admin 路由
├── services/     # 业务层：cache / post / comment / auth / email / system（REST 与 MCP 共用）
├── mcp/          # MCP 服务：Streamable HTTP、会话级邮箱登录、26 个工具
└── models/       # DB 行结构 + serde DTO
migrations/       # sqlx migration（内嵌进二进制，启动自动执行）
```

## 设计要点

- **缓存**：Cache-Aside + 写后失效；Redis 不可用时全部回源 MySQL 并告警，不中断服务（降级策略见 `docs/development-plan.md` §5）。
- **安全**：argon2 密码哈希；Session Token 仅存 Redis（`session:{token}`，7 天滑动续期）；Cookie `HttpOnly; SameSite=Lax`（HTTPS 下加 `Secure`）；Markdown 渲染后 ammonia 白名单消毒；评论游客输入长度/格式校验 + 60s/IP 限流；登录 10min 5 次/IP 限流。可选 `ADMIN_API_TOKEN`：配置后请求头 `x-admin-token` / `Authorization: Bearer` 携带该 token 可绕过登录鉴权执行全部管理操作（取库中第一位用户作为管理员身份），不配置则关闭；生产环境务必通过流水线 Secret 注入。
- **MCP**：官方 `rmcp` SDK + Streamable HTTP，挂载 `/mcp`。MCP 会话内先 `auth_send_email_code` 再 `auth_login`，之后可调用文章/评论/标签/统计等 26 个工具；也支持 `x-admin-token` 请求头直通。协议上限固定在 `2025-11-25`（后续版本移除会话，无法承载登录态）。详见 [docs/mcp.md](../docs/mcp.md)。
- **migration**：文件合入后不可修改，只新增；由 `sqlx::migrate!` 在启动时应用。

## API 一览

见 [docs/development-plan.md](../docs/development-plan.md) §6。统一响应包：`{ "code": 0, "data": ..., "message": "" }`。

## 部署

见 [deploy/](../deploy/)（Dockerfile + docker-compose + Caddyfile）。生产建议 Caddy 托管 `frontend/` 并反代 `/api/*` 到本服务（同源无 CORS），此时 `FRONTEND_DIR` 留空。

另有 GitHub Actions 打包部署流水线（scp 二进制 + 原子替换 + 健康检查），配置方法见根目录 [README.md](../README.md) 的「CI/CD 流水线」一节。
