# 一本日记（riben.icu）

一本手账风格的个人博客：纯 HTML + 原生 JS 前端，Rust（axum）后端，MySQL 持久化，Redis 缓存 / Session / 限流。

## 仓库结构

```
riben/
├── frontend/          # 前端：纯静态站点（无构建步骤），由 Caddy 托管
│   ├── index.html / post.html / archive.html / admin.html
│   ├── css/style.css
│   └── js/            # api.js / render.js / theme.js / main.js / admin.js
├── server/            # 后端：Rust + axum + sqlx + redis（独立 Cargo 项目）
│   ├── migrations/    # sqlx migration（启动时自动执行，内嵌于二进制）
│   └── src/           # routes / services / middleware / models
├── deploy/            # 部署配置：Dockerfile + docker-compose + Caddyfile
└── docs/
    └── development-plan.md   # 设计与开发计划（M0–M6 里程碑）
```

## 架构

```
浏览器（frontend/ 静态页面 + fetch 渲染）
        │ /api/*
        ▼
   Caddy ── /api/* → app: riben-server (axum)
                        ├── MySQL 8（文章/评论/标签/用户）
                        └── Redis 7（Cache-Aside 缓存 + Session + 限流计数器）
```

- 前端无构建："版本"即 Git 提交；直接静态托管即可预览。
- 后端单二进制：migration 内嵌，启动自动建表；Redis 不可用时降级直连 MySQL，不中断服务。

## 快速开始

### 本地开发（需要 MySQL 8 + Redis 7，或直接用 Docker Compose）

```bash
# 后端（自动跑 migration；首次启动按 .env 播种管理员/演示文章）
cd server
cp .env.example .env       # 修改 DATABASE_URL / ADMIN_PASSWORD 等
cargo run

# 前端（另开终端，静态预览；页面数据来自 :3000 的 API）
cd frontend
python3 -m http.server 8080   # 或任意静态服务器
```

生产形态建议直接用 Compose（Caddy 托管前端 + 反代 API，同源无 CORS）：

```bash
cp deploy/.env.example deploy/.env   # 改密码
docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d --build
```

### 测试与质量门槛

```bash
cd server
cargo fmt --check
cargo clippy --all-targets
cargo test          # slug / 摘要 / 评论树 / Markdown 消毒等单元测试
```

## 常用入口

| 入口 | 说明 |
|---|---|
| `/`（index.html） | 首页：最新 6 篇 + 「已记录 N 天」徽标 |
| `/archive.html` | 按月归档 |
| `/post.html?p={slug}` | 文章详情 + 留言角 |
| `/admin.html` | 管理后台（登录后管理文章/评论/标签） |
| `/api/health` | 健康检查（MySQL / Redis 状态） |

详细 API、缓存键设计、里程碑拆解见 [docs/development-plan.md](docs/development-plan.md)。

## 环境变量

见 [server/.env.example](server/.env.example) 与 [deploy/.env.example](deploy/.env.example)。

## 许可

MIT
