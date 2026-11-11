# 一本日记 · 全栈开发计划

> 项目现状：纯静态站点（`index.html` / `post.html` / `archive.html` + `css/style.css` + `js/main.js`），手账风格个人博客，域名 riben.icu。
> 目标：引入 **Rust 后端 + MySQL + Redis**，实现博客文章的动态管理、评论系统（含管理/审核）、缓存加速，并保留现有前端视觉风格。
>
> **更新（M6 之后）**：前端已由原生 JS 重写为 **Vite + React 18 + TypeScript**（MPA，`frontend/`，URL 不变），§1.2 的"前端框架化"已落地。

---

## 1. 目标与范围

### 1.1 核心目标
- 博客文章（Posts）从静态 HTML 迁移到数据库，支持增删改查、标签分类、按月归档。
- 新增评论系统（Comments）：游客可发表评论，作者可审核/删除/回复。
- 引入 Redis 缓存，加速首页/归档/文章页等读多写少场景。
- 提供作者后台（Admin）：登录后发布、编辑文章，管理评论。

### 1.2 非目标（本期不做）
- 多用户注册 / 游客账号体系（评论以昵称 + 邮箱 + 可选网址的形式发布，不需要注册）。
- 富媒体上传（图片先保持外链；图片上传可列入二期）。
- SSR / 前端框架化（保持纯 HTML + 原生 JS，见 §8）。

---

## 2. 技术选型

| 领域 | 推荐方案 | 说明 |
|---|---|---|
| Web 框架 | **axum** | 与 Tokio 生态契合，路由/中间件/提取器模型清晰，社区活跃 |
| 数据库访问 | **sqlx**（编译期检查 SQL） | 手写 SQL 可控性强，适合简单模型；若偏好 ORM 可用 **sea-orm**（备选） |
| 连接池 | sqlx 自带 `PgPool` 的 MySQL 对应物 `MySqlPool` | — |
| 缓存 | **redis-rs**（`aio::MultiplexedConnection`） | 配合 connection pool 使用 |
| Markdown 渲染 | **comrak**（CommonMark + GFM） | 文章正文以 Markdown 存储，渲染为 HTML，配合 **ammonia** 做 HTML 消毒 |
| 密码哈希 | **argon2**（或 bcrypt） | 仅作者账号使用 |
| 会话管理 | Redis 存储 Session Token（`SESSION:{token} -> user_id`） | 无需 JWT，简单、可吊销；管理端 Cookie（`HttpOnly; SameSite=Lax`） |
| 配置管理 | **config-rs** + `.env` / `config.toml` | 环境变量优先，便于部署 |
| 日志/追踪 | **tracing** + **tracing-subscriber** | 结构化日志 |
| 部署 | Docker Compose（mysql + redis + app + caddy/nginx） | 单机部署，见 §9 |
| 开发辅助 | **sqlx-cli**（migration）、**cargo-watch**、**cargo-nextest** | — |

> 备选说明：`actix-web`、`poem` 均可替代 axum；如果后续模型复杂化（多表关联、关联查询多），sea-orm 会比 sqlx 更省力。本计划按 **axum + sqlx** 编排。

---

## 3. 总体架构

```
浏览器（现有静态前端 + fetch 动态加载）
        │  HTTP / JSON（/api/*）
        ▼
┌─────────────────────────────────────┐
│  Rust 后端 (axum, 单二进制)          │
│  ├─ 路由层 routes/                   │
│  ├─ 业务/服务层 services/            │
│  ├─ 中间件：鉴权、限流、日志、CORS    │
│  ├─ 静态资源：托管 frontend/（或由 Caddy 托管）│
│  └─ 错误处理：统一 JSON 错误体        │
└──────┬──────────────────┬───────────┘
       │ MySQL (TCP)      │ Redis (RESP)
       ▼                  ▼
  持久化存储            缓存 / Session / 限流计数器
```

- **部署拓扑**：单机上 Docker Compose 跑 3 个容器（app、mysql、redis），Caddy/Nginx 做反向代理 + TLS。
- **同构演进**：`index.html` / `archive.html` / `post.html` 保留为入口壳页面，内容由 JS `fetch('/api/...')` 渲染（§8）。

---

## 4. 数据库设计（MySQL 8，utf8mb4）

### 4.1 表结构

**`posts` — 文章**

| 字段 | 类型 | 说明 |
|---|---|---|
| id | BIGINT UNSIGNED AUTO_INCREMENT PK | |
| slug | VARCHAR(190) UNIQUE NOT NULL | URL 友好标识，如 `first-snow-early` |
| title | VARCHAR(190) NOT NULL | |
| content_md | MEDIUMTEXT NOT NULL | Markdown 原文 |
| excerpt | VARCHAR(500) | 卡片摘要（可为空，由正文截断生成） |
| status | TINYINT NOT NULL DEFAULT 0 | 0=草稿，1=已发布 |
| published_at | DATETIME NULL | 发布时间 |
| created_at / updated_at | DATETIME NOT NULL | DEFAULT CURRENT_TIMESTAMP / ON UPDATE |
| INDEX (status, published_at DESC) | | 列表查询主索引 |

**`tags` / `post_tags` — 标签（多对多）**

- `tags`: id, name UNIQUE, color（保留现有 tag 色系：t-pink / t-mint / t-blue / t-yellow）
- `post_tags`: post_id, tag_id，联合主键 (post_id, tag_id)，索引 (tag_id, post_id)

**`comments` — 评论**

| 字段 | 类型 | 说明 |
|---|---|---|
| id | BIGINT UNSIGNED PK | |
| post_id | BIGINT UNSIGNED NOT NULL, FK→posts ON DELETE CASCADE, 索引 | |
| parent_id | BIGINT UNSIGNED NULL, FK→comments ON DELETE SET NULL | 回复楼中楼，顶层为 NULL |
| author_name | VARCHAR(64) NOT NULL | |
| author_email | VARCHAR(190) NOT NULL | 仅展示与通知用，不公开显示 |
| author_url | VARCHAR(190) NULL | 可选，nofollow |
| content | TEXT NOT NULL | 纯文本 + 换行，前端渲染时转义 |
| status | TINYINT NOT NULL DEFAULT 0 | 0=待审核，1=已通过，2=已拒绝/删除（软删） |
| created_at | DATETIME NOT NULL | |
| INDEX (post_id, status, created_at) | | 按文章拉取已通过的评论 |

**`users` — 作者账号（单一/少量）**

| 字段 | 类型 | 说明 |
|---|---|---|
| id | BIGINT UNSIGNED PK | |
| username | VARCHAR(64) UNIQUE NOT NULL | |
| password_hash | VARCHAR(190) NOT NULL | argon2 |
| created_at | DATETIME NOT NULL | |

### 4.2 Migration 管理

- 使用 `sqlx-cli`：`sqlx migrate add <name>` 生成 `migrations/`，应用顺序纳入版本控制。
- 所有 DDL 可重复执行（幂等），禁止直接改已发布的 migration 文件。

---

## 5. Redis 缓存设计

策略：**Cache-Aside（旁路缓存）+ 写后失效**。

| Key 模式 | 类型 | TTL | 内容 | 失效时机 |
|---|---|---|---|---|
| `post:detail:{slug}` | String (JSON) | 10 min + 主动失效 | 已发布文章详情（含标签） | 文章更新/删除/状态变更时 DEL |
| `post:list:home` | String (JSON) | 5 min | 首页最新 N 篇（默认 6 篇） | 发布新文章 / 更新 / 删除时 DEL |
| `post:list:archive` | String (JSON) | 10 min | 按月分组的归档数据 | 同上 |
| `archive:months` | String (JSON) | 10 min | 归档月份列表 | 同上 |
| `tag:all` | String (JSON) | 30 min | 全部标签 | 标签变更时 DEL |
| `comment:list:{post_id}` | String (JSON) | 2 min | 已通过评论列表（嵌套结构） | 该文章评论审核通过/新增时 DEL |
| `session:{token}` | String | 7 天（滑动续期） | user_id | 登出/吊销时 DEL |
| `rate:comment:{ip}` | String (INCR + EXPIRE) | 1 min | 评论提交频率限制（如 60s 内 1 次） | 到期自然失效 |
| `rate:login:{ip}` | String | 10 min | 登录尝试限制（如 5 次失败锁定） | 同上 |

约定：
- 所有写操作（POST/PUT/DELETE）成功提交后，同步失效相关 key；TTL 作为兜底。
- 缓存未命中时回源 MySQL 并回填；Redis 不可用时降级直连 MySQL（日志告警，不阻断服务）。
- 后期可给 Key 统一加版本前缀（如 `v1:`），便于整体换缓存结构。

---

## 6. API 设计（REST，统一前缀 `/api`）

统一响应包：

```json
{ "code": 0, "data": ..., "message": "" }        // 成功
{ "code": 40001, "data": null, "message": "..." } // 失败（业务码）
```

| 方法 | 路径 | 鉴权 | 说明 |
|---|---|---|---|
| GET | `/api/posts?limit=6` | 公开 | 首页最新已发布列表 |
| GET | `/api/posts/archive` | 公开 | 按月归档分组 |
| GET | `/api/posts/{slug}` | 公开 | 文章详情（含标签、评论计数） |
| GET | `/api/posts/{slug}/comments` | 公开 | 已通过的评论（嵌套） |
| POST | `/api/comments` | 公开（限流+校验） | 发表评论（默认待审核）；body: post_id/slug, name, email, url?, content, parent_id? |
| POST | `/api/auth/login` | 公开（限流） | 登录，返回 Set-Cookie 会话 |
| POST | `/api/auth/logout` | 登录 | 登出 |
| GET | `/api/auth/me` | 登录 | 当前用户 |
| POST | `/api/admin/posts` | 登录 | 新建文章（含标签 id 列表） |
| PUT | `/api/admin/posts/{id}` | 登录 | 编辑文章 |
| DELETE | `/api/admin/posts/{id}` | 登录 | 删除文章（软删可选） |
| GET | `/api/admin/posts` | 登录 | 后台文章列表（含草稿，分页） |
| GET | `/api/admin/comments?status=pending` | 登录 | 评论管理列表（分页、按状态筛） |
| PUT | `/api/admin/comments/{id}/approve` | 登录 | 审核通过 |
| PUT | `/api/admin/comments/{id}/reject` | 登录 | 拒绝/删除 |
| POST | `/api/admin/tags` | 登录 | 新建标签（编辑可二期） |

校验与约定：
- 入参统一用 `serde` DTO + validator（`validator` crate）；错误返回 422 + 字段明细。
- 游客接口：评论字段长度限制（昵称 ≤64、内容 ≤2000、内容去首尾空白后非空），邮箱格式校验。
- 服务端渲染评论时纯文本转义；URL 仅允许 http/https 且输出加 `rel="nofollow ugc"`。
- 管理接口走 `RequireAuth` 中间件（校验 session cookie → Redis）；失败一律 401，由前端跳登录。
- 限流：评论 60 秒 1 次/IP（Redis 计数器），登录 10 分钟 5 次/IP。

---

## 7. 仓库结构与项目管理方式

### 7.0 总原则：一个 Git 仓库，两个独立项目

仓库根目录按 **前后端分离** 组织为两个项目目录，各自有独立的依赖管理与构建流程，共享同一个 Git 仓库、docs 与 deploy 配置：

```
riben/                        # Git 仓库根
├── frontend/                 # 前端项目（纯静态站点，独立可发布）
│   ├── index.html
│   ├── post.html / archive.html
│   ├── admin.html            # 新增：管理后台
│   ├── css/ js/
│   └── README.md             # 前端说明：本地预览方式、构建=无构建
├── server/                   # 后端项目（独立 Cargo 项目）
│   ├── Cargo.toml
│   ├── .env.example
│   ├── migrations/
│   └── src/…
├── docs/
│   └── development-plan.md   # 本文档
├── deploy/                   # 部署配置（前后端共同使用）
│   ├── docker-compose.yml
│   ├── Dockerfile
│   └── Caddyfile
└── README.md                 # 仓库总览：架构图、两个项目的关系、快速开始
```

- 部署时 Rust 服务托管 `frontend/` 下的静态资源（或由 Caddy 直接托管 `frontend/`，只反代 `/api/*` 到 Rust，二选一，推荐后者便于前后端独立迭代）。
- 前端无构建步骤，"版本"即 Git 提交；后端以 `server/Cargo.toml` 的 version 字段为准。

### 7.1 后端项目结构（`server/`）

```
server/
├── Cargo.toml
├── .env.example           # DATABASE_URL / REDIS_URL / SESSION_SECRET 等
├── migrations/            # sqlx-cli 生成
└── src/
    ├── main.rs            # 启动：加载配置、连接池、路由挂载
    ├── config.rs          # 配置结构体
    ├── state.rs           # AppState（MySqlPool + Redis conn + config）
    ├── error.rs           # AppError → 统一 JSON 错误体
    ├── middleware/
    │   ├── auth.rs        # session 鉴权
    │   └── rate_limit.rs  # Redis 限流
    ├── routes/
    │   ├── mod.rs         # 路由聚合
    │   ├── post.rs        # 公开文章接口
    │   ├── comment.rs     # 公开评论接口
    │   ├── auth.rs        # 登录/登出
    │   └── admin.rs       # 管理接口
    ├── services/          # 业务逻辑（缓存读写、失效）
    │   ├── post.rs
    │   ├── comment.rs
    │   └── cache.rs
    ├── models/            # DB 行结构 + serde DTO
    └── db.rs              # 连接池初始化 + migration 检查
```

### 7.2 前端项目结构（`frontend/`）

```
frontend/
├── index.html             # 首页（壳页面 + JS 渲染）
├── post.html              # 文章详情页 ?p={slug}
├── archive.html           # 按月归档
├── admin.html             # 管理后台（单页）
├── css/style.css          # 现有样式，全站复用
└── js/
    ├── main.js            # 入口（现有主题切换逻辑保留）
    ├── theme.js           # 主题切换（从 main.js 拆出）
    ├── api.js             # fetch 封装
    ├── render.js          # 卡片/归档/评论 DOM 渲染
    └── admin.js           # 管理后台页面逻辑
```

### 7.3 项目管理方式

**分支模型（GitHub Flow 简化版）**

- `main` 为唯一长期分支，始终保持可部署状态；功能在 `feat/*` 分支开发，完成后 PR 合入。
- 分支命名：`feat/<scope>-<简述>`、`fix/<scope>-<简述>`，如 `feat/server-comment-api`、`fix/frontend-archive-render`。
- 分支提交与合入粒度对应里程碑 M0–M6：每个里程碑至少一个 PR，禁止直接 push `main`。

**提交信息规范（Conventional Commits）**

- 格式：`<type>(<scope>): <subject>`，`scope` 用 `frontend` / `server` / `deploy` / `docs`。
- 示例：`feat(server): add comment submit endpoint with rate limiting`、`feat(frontend): render archive page from api`、`fix(frontend): dark mode flash on post page`。
- 一个提交只做一件事；提交信息用英文，正文可补中文说明动机。

**协作与评审**

- 所有 PR 需通过 CI 检查（`cargo fmt --check`、`cargo clippy -D warnings`、`cargo nextest`、前端无构建则跑简单的链接/语法检查）后才能合入。
- 里程碑文档（M0–M6）拆解为 GitHub Issues，打上 `M1`~`M6`、`frontend`/`server` 标签，按里程碑顺序推进；Issue 标题格式 `[M3] 评论审核接口`。
- 较大的设计变更（API 结构、表结构变更）先更新本文档再动手。

**版本与发布**

- 后端版本：`server/Cargo.toml` version，里程碑完成时打 tag，如 `server-v0.3.0`。
- 前端无版本号，跟随仓库 tag 记录整站发布节点，如 `site-v1.0`。
- 数据库 migration 与代码同 PR 提交，migration 文件合入后不可修改，只能新增。

**文档与目录纪律**

- `docs/` 放设计文档；`deploy/` 只放部署配置；前端代码一律在 `frontend/`，后端在 `server/`，禁止跨目录相对引用。
- 每个项目目录自带 `README.md` 说明启动方式；根 `README.md` 维护整体架构图和"改前端/改后端分别怎么跑"的快速指引。

---

## 8. 前端改造计划（保持手账风格）

原则：**视觉零改动，数据动态化**。现有卡片/贴纸/胶带样式全部复用，只把写死的内容换成 JS 渲染。全部前端文件位于 `frontend/` 目录。

1. **`frontend/js/` 模块拆分**
   - `api.js`：fetch 封装（统一解析 `{code,data,message}`，错误提示）。
   - `render.js`：卡片 / 归档列表 / 标签的 DOM 渲染模板（HTML 结构与现有静态版一一对应）。
   - `theme.js`：保留现有主题切换逻辑（从 `main.js` 拆出，`main.js` 仅作入口）。
2. **首页 `index.html`**：手账集卡片区改为渲染 `/api/posts?limit=6`；「已记录 N 天」徽标由接口返回的统计数据驱动。
3. **归档页 `archive.html`**：按月分组渲染 `/api/posts/archive`，月份标题保持胶带样式。
4. **文章页 `post.html?id={slug}`**：URL 改为 `post.html?p={slug}`；正文由 `/api/posts/{slug}` 返回，Markdown 由后端渲染为消毒后的 HTML 后返回（前端不引渲染库）；`title`/`description` 由 JS 动态写入 `<head>`。
5. **评论区（新增模块，post.html 底部）**：
   - 展示已通过评论（嵌套缩进），样式沿用贴纸/卡片语言（`📮` 区标题）。
   - 发表评论表单：昵称、邮箱（不展示）、网址（可选）、内容；提交后提示「待审核」。
6. **管理后台 `frontend/admin.html`（新增，单页原生 JS）**：
   - 登录页 → 文章列表（编辑/发布/删除）→ Markdown 编辑器（textarea + 预览，二期可换 simplemde）。
   - 评论管理：待审核队列（通过/拒绝），已通过的评论可删除。
   - 样式复用现有 `style.css` 变量与组件，保持同一套视觉。
7. **SEO/体验**：首屏仍可由后端注入 SSR？——本期不做，二期考虑在 axum 侧做模板渲染（`askama`）替代 JS 渲染。

---

## 9. 部署方案

- **Docker Compose** 三服务：
  - `mysql:8`（数据卷持久化；初始化脚本自动跑 migration 或容器启动时由 app 执行 `sqlx migrate run`）
  - `redis:7`（AOF 持久化按需开启；缓存 + session）
  - `app`（多阶段构建：chef/缓存依赖层 → release 二进制 → 精简运行时镜像）
- 反代：**Caddy**（自动 TLS，域名 riben.icu；站点根指向 `frontend/`，`/api/*` 反代到 Rust 服务，同源无 CORS；若选择由 Rust 托管静态资源则去掉 Caddy 的站点根配置）。
- 环境变量：数据库/Redis 连接串、session 密钥、管理员初始用户名密码（首次启动播种）。
- CI/CD（二期）：GitHub Actions 跑 `cargo fmt/clippy/test`，构建镜像并推送，服务器 pull 后滚动重启。

---

## 10. 开发阶段与里程碑

| 阶段 | 内容 | 产出 / 验收 |
|---|---|---|
| **M0 项目骨架** | 初始化 `server/`，axum 起服务，托管现有静态文件，连接 MySQL/Redis 健康检查，tracing 日志，统一错误体 | 访问 riben.icu 静态页面由 Rust 服务返回，/api/health 返回 200 |
| **M1 数据库 & 文章 API** | sqlx-cli migration 建 4 张表；文章 CRUD（公开读 + 管理写）；slug 生成与唯一性处理；comrak+ammonia 渲染 | 可发布/编辑文章，GET 列表/详情/归档返回正确 JSON |
| **M2 缓存层** | Redis 封装；文章列表/详情/归档/标签 Cache-Aside；写后失效；Redis 降级容错 | 二次请求命中缓存（日志可见），更新文章后缓存立即失效 |
| **M3 评论系统** | 评论发表（校验+限流+嵌套回复）、待审核机制、公开查询（仅已通过）；后台审核接口 | 游客可发评论（进入待审核），管理端可通过/拒绝，前台只显示已通过 |
| **M4 鉴权与管理端 API** | 登录/登出/session 中间件/登录限流；admin 接口全部接通；种子管理员 | 未登录访问 admin API 返回 401；登录成功后可管理文章与评论 |
| **M5 前端动态化** | api.js/render.js；首页/归档/文章页改 JS 渲染；评论区；动态 SEO 字段 | 三个页面数据来自后端，样式与现状一致，评论区可用 |
| **M6 管理后台页面 + 部署** | `frontend/admin.html` 登录/文章/评论管理；Dockerfile + compose + Caddy；本地全流程联调 | docker compose up 一键起全站；域名 HTTPS 可访问完整闭环 |
| **M7（二期，可选）** | 图片上传（对象存储）、邮件通知新评论、RSS、SSR/SEO 增强、评论垃圾过滤（关键字库）、CI/CD | — |

---

## 11. 测试与质量

- **单元测试**：slug 生成、DTO 校验、评论树组装、缓存 key 生成等纯逻辑（`cargo nextest`）。
- **集成测试**：`sqlx` 基于测试库跑仓库层 SQL；axum `oneshot` 请求测试覆盖关键路由（含鉴权失败场景）；测试用临时 MySQL/Redis（compose 的 `test` profile）。
- **手动验收清单**：以「发布一篇文章 → 首页/归档/详情出现 → 游客评论 → 后台审核通过 → 前台可见」为端到端验收主线。
- 代码门槛：`clippy -D warnings` + `cargo fmt` 纳入提交前检查。

---

## 12. 风险与注意事项

| 风险 | 应对 |
|---|---|
| Markdown 渲染 XSS | 后端 comrak 渲染 + ammonia 白名单消毒，前端不再二次插入原始 HTML（使用 `textContent` 或可信模板） |
| 评论 spam | 先靠昵称/邮箱必填 + IP 限流；二期接 Turnstile/hCaptcha；可再加关键词过滤 |
| session 泄露 | Cookie `HttpOnly + SameSite=Lax`（HTTPS 下 `Secure`）；token 仅存 Redis，可吊销 |
| 缓存与数据库不一致 | 写后主动失效为主 + 短 TTL 兜底；管理操作（删除/下线）必须触发失效 |
| 静态页直接访问时的空数据闪动 | JS 渲染前显示与现风格一致的骨架占位 |
| 数据备份 | MySQL 卷定期快照；迁移文件版本化保证可重建 |

---

## 13. 待确认的决策点

以下按推荐值写入计划，如需调整请指出：

1. **ORM 选择**：推荐 sqlx（简单模型、SQL 可控）；若希望模型驱动开发可换 sea-orm。
2. **评论默认状态**：推荐「默认待审核」（个人站，量小）；若嫌麻烦可改「信任邮箱白名单免审」。
3. **管理员入口**：推荐独立 `frontend/admin.html`（同源部署为 `/admin.html`）；亦可二期做子域名 `admin.riben.icu`。
4. **图片资源**：本期沿用外链；图片上传列入二期（本地目录 + Nginx 托管或对象存储）。
