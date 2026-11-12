# 一本日记（riben.icu）

一本手账风格的个人博客：React（Vite + TypeScript）前端，Rust（axum）后端，MySQL 持久化，Redis 缓存 / Session / 限流。

## 仓库结构

```
riben/
├── frontend/          # 前端：Vite + React 18 + TS（MPA，构建产物 dist/ 由 Caddy 托管）
│   ├── index.html / post.html / archive.html / admin.html   # Vite MPA 入口（URL 与旧静态站一致）
│   ├── css/style.css  # 全站样式（手账风格，前后端共用视觉）
│   └── src/           # lib/api.ts、theme.ts、components/、pages/（含 admin/）
├── server/            # 后端：Rust + axum + sqlx + redis（独立 Cargo 项目）
│   ├── migrations/    # sqlx migration（启动时自动执行，内嵌于二进制）
│   └── src/           # routes / services / middleware / models
├── deploy/            # 部署配置：Dockerfile + docker-compose + Caddyfile
├── .github/workflows/ # CI/CD：backend-deploy.yml（后端打包部署）、frontend-deploy.yml（前端构建部署）
└── docs/
    └── development-plan.md   # 设计与开发计划（M0–M6 里程碑）
```

## 架构

```
浏览器（frontend/dist 构建产物：React MPA，fetch 渲染）
        │ /api/*
        ▼
   Caddy ── /api/* → app: riben-server (axum)
                        ├── MySQL 8（文章/评论/标签/用户）
                        └── Redis 7（Cache-Aside 缓存 + Session + 限流计数器）
```

- 前端为 Vite MPA：`pnpm build` 产物在 `frontend/dist/`，URL 保持 `/post.html?p=slug` 等旧链接不变。
- 后端单二进制：migration 内嵌，启动自动建表；Redis 不可用时降级直连 MySQL，不中断服务。

## 快速开始

### 本地开发（需要 MySQL 8 + Redis 7，或直接用 Docker Compose）

```bash
# 后端（自动跑 migration；首次启动按 .env 播种管理员/演示文章）
cd server
cp .env.example .env       # 修改 DATABASE_URL / ADMIN_PASSWORD 等
cargo run                  # 默认 :9015

# 前端（另开终端；Vite dev 会把 /api/* 代理到 :9015）
cd frontend
pnpm install
pnpm dev                   # http://localhost:5173
```

> **注意：数据库是否需要手动创建？**
> - **Docker Compose 部署**：无需手动建库。compose 中的 `MYSQL_DATABASE` 变量会让 MySQL 容器首次启动时自动创建数据库（见 `deploy/docker-compose.yml`）。
> - **本地开发直连 MySQL**：需要手动建库。migration 只会 `CREATE TABLE`，不会创建数据库本身：
>   ```sql
>   CREATE DATABASE riben CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
>   ```
>   若数据库不存在，服务不会报错退出（连接失败会退化为 lazy 池静默启动），但随后 migration 失败、接口不可用——排查时请先确认数据库已创建。

生产形态建议直接用 Compose（Caddy 托管前端 dist + 反代 API，同源无 CORS）。**部署前先构建前端**：

```bash
cd frontend && pnpm build  # 产物输出到 frontend/dist/
cp deploy/.env.example deploy/.env   # 改密码
docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d --build
```

### 测试与质量门槛

```bash
cd server
cargo fmt --check
cargo clippy --all-targets
cargo test          # slug / 摘要 / 评论树 / Markdown 消毒等单元测试

cd ../frontend
pnpm build          # tsc --noEmit 类型检查 + vite build
```

## CI/CD 流水线（GitHub Actions）

两条流水线各自使用独立的服务器与密钥配置，互不干扰、按需触发：

- 后端：[.github/workflows/backend-deploy.yml](.github/workflows/backend-deploy.yml)
- 前端：[.github/workflows/frontend-deploy.yml](.github/workflows/frontend-deploy.yml)

### 后端流水线（backend-deploy）

- **触发**：`main` 分支上 `server/**` 或该 workflow 文件变更时自动触发，也可在 Actions 页面手动触发（workflow_dispatch）。
- **流程**：`check-ssh`（逐台验证 SSH 登录）→ `build`（Ubuntu 上 cargo Release 编译并上传产物）→ `deploy`（按 `SERVERS_CONFIG` 逐机部署，一台失败不影响其他机器）。
- **部署方式**：scp 上传二进制到 `deploy_path` 并原子替换（先传 `.new` 再 `mv`），写入 `deploy_path/.env`，优先 `systemctl restart riben-server`，无 systemd 单元时 `pkill + nohup` 重启，最后探测 `http://127.0.0.1:<app_port>/api/health` 确认启动成功。
- **并发控制**：同一时刻只跑一个部署（concurrency），避免并发互踩。

### 前端流水线（frontend-deploy）

- **触发**：`main` 分支上 `frontend/**` 或该 workflow 文件变更时自动触发，也可手动触发。与后端流水线独立，改前端不会触发后端重编译。
- **流程**：`parse`（解析 `FRONTEND_SERVERS_CONFIG`）→ `build`（pnpm 安装依赖、`tsc --noEmit` 类型检查 + `vite build`，打包 `frontend/dist` 为 tar 上传产物）→ `deploy`（按 `FRONTEND_SERVERS_CONFIG` 逐机发布，一台失败不影响其他机器）。
- **部署方式**：scp tar 包到服务器后解压进 `<web_root>.new`，再原子切换目录（旧版本保留为 `<web_root>.old`，便于回滚），静态文件无需重启服务；最后校验 `index.html` 就位。
- **web_root**：前端发布目录取 `FRONTEND_SERVERS_CONFIG` 每台服务器的 `web_root` 字段（必填）。若服务器上用 Caddy/Nginx 托管，请把站点根目录指向该 `web_root`。

### 配置方法

在 GitHub 仓库 **Settings → Secrets and variables → Actions** 中配置。后端、前端两套变量相互独立，互不影响。

**Variables**（明文变量）：

| 变量 | 必填 | 说明 |
|---|---|---|
| `SERVERS_CONFIG` | 是（后端） | 后端服务器列表，JSON 数组（示例见下） |
| `SESSION_TTL_DAYS` | 否 | 后端 Session 有效期天数，默认 7 |
| `COOKIE_SECURE` | 否 | 后端默认 true（HTTPS 部署）；纯 HTTP 内网部署时设 false |
| `FRONTEND_SERVERS_CONFIG` | 是（前端） | 前端发布服务器列表，JSON 数组（示例见下） |

`SERVERS_CONFIG`（后端）示例：

```json
[
  {
    "name": "prod-1",
    "addr": "1.2.3.4",
    "port": "22",
    "user": "deploy",
    "deploy_path": "/opt/smileyan-backend",
    "app_port": "9015"
  }
]
```

`FRONTEND_SERVERS_CONFIG`（前端）示例——每台服务器必须配置 `web_root`（前端静态目录）：

```json
[
  {
    "name": "prod-1",
    "addr": "1.2.3.4",
    "port": "22",
    "user": "deploy",
    "web_root": "/opt/smileyan-web"
  }
]
```

**Secrets**（机密变量）：

| 密钥 | 说明 |
|---|---|
| `SSH_PRIVATE_KEY` | 后端各服务器 deploy 用户的 SSH 私钥（公钥需预先放到对应用户的 `~/.ssh/authorized_keys`） |
| `DATABASE_URL` | MySQL 连接串，如 `mysql://user:pass@host:3306/riben` |
| `REDIS_URL` | Redis 连接串，如 `redis://:pass@host:6379` |
| `ADMIN_USERNAME` / `ADMIN_PASSWORD` | 首次播种的管理员账号（仅首次启动写入） |
| `ADMIN_API_TOKEN` | 可选，管理员 API Token。**务必用 Secret 配置**：配置后请求头 `x-admin-token: <token>`（或 `Authorization: Bearer <token>`）可绕过登录鉴权，执行全部管理操作；不配置则关闭该能力 |
| `FRONTEND_SSH_PRIVATE_KEY` | 前端各服务器 deploy 用户的 SSH 私钥（独立于后端的 `SSH_PRIVATE_KEY`） |

### 服务器前提

- 各流水线的 deploy 用户能通过对应的私钥免密登录（流水线使用 `BatchMode` 非交互登录）。
- 后端 `user` 对 `deploy_path` 有读写权限；前端 `user` 对 `web_root` 有读写权限（流水线会 `mkdir -p`；后端写入二进制与 `.env`，前端写入静态文件）。
- 若希望用 systemd 管理（推荐）：预先在服务器上创建 `riben-server` systemd 单元，并给该用户 `sudo systemctl restart riben-server` 的免密权限；否则流水线自动回退为 `pkill + nohup`（日志写入 `deploy_path/app.log`）。
- 服务器需装有 `curl`（健康检查用）。

## 常用入口

| 入口 | 说明 |
|---|---|
| `/`（index.html） | 首页：最新 6 篇 + 「已记录 N 天」徽标 |
| `/archive.html` | 按月归档 |
| `/post.html?p={slug}` | 文章详情 + 留言角 |
| `/admin.html` | 管理后台（登录后管理文章/评论/标签） |
| `/api/health` | 健康检查（MySQL / Redis 状态） |

- 详细 API、缓存键设计、里程碑拆解见 [docs/development-plan.md](docs/development-plan.md)；
  智能体（Agent）鉴权与接口调用指南见 [docs/agent-api.md](docs/agent-api.md)。

## 环境变量

见 [server/.env.example](server/.env.example) 与 [deploy/.env.example](deploy/.env.example)。

## 许可

MIT
