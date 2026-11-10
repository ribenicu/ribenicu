# 一本日记 · 前端

纯静态站点，**无构建步骤**："版本"即 Git 提交。页面内容由 JS `fetch('/api/...')` 动态渲染，视觉与早期静态版完全一致（手账风格，见 `css/style.css`）。

## 文件

| 文件 | 说明 |
|---|---|
| `index.html` | 首页：最新 6 篇卡片 + 站点统计徽标 |
| `archive.html` | 按月归档 |
| `post.html` | 文章详情（`?p={slug}`）+ 留言角 |
| `admin.html` | 管理后台（登录 / 文章 / 评论审核 / 标签） |
| `js/api.js` | fetch 封装：统一解析 `{code,data,message}`，错误抛异常 |
| `js/render.js` | DOM 渲染模板：卡片 / 归档 / 评论（用户输入一律转义） |
| `js/theme.js` | 主题切换（浅色/深色） |
| `js/main.js` | 入口：按 `body[data-page]` 初始化各页面 |
| `js/admin.js` | 管理后台单页逻辑 |

## 本地预览

需要一个可访问的后端（默认同源的 `/api`）。最简单的方式：

```bash
# 终端 1：启动后端（见 ../server/README.md，默认 :3000）
cd ../server && cargo run

# 终端 2：起个静态服务器（或让后端 FRONTEND_DIR=../frontend 直接托管）
cd frontend && python3 -m http.server 8080
```

若静态服务器与后端不同源，可用反向代理把 `/api/*` 指到 `:3000`（开发示例）：

```bash
caddy file-server --root . --listen :8080  # 仅静态；API 代理见 deploy/Caddyfile
```

## 约定

- 文章正文 HTML 由后端渲染并消毒（comrak + ammonia），前端不引 Markdown 库。
- 所有用户输入（标题、昵称、评论等）经 `Riben.render.escapeHtml` 转义后插入 DOM。
- 主题选择存 `localStorage('riben-theme')`，首屏内联脚本防暗色闪烁。
