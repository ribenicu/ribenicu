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
cargo run                  # 默认 :3000

# 前端（另开终端；Vite dev 会把 /api/* 代理到 :3000）
cd frontend
pnpm install
pnpm dev                   # http://localhost:5173
```

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
