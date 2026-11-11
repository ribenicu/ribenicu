# 一本日记 · 前端

Vite + React 18 + TypeScript 的 MPA 工程。**URL 与原静态站完全一致**（`/index.html`、`/archive.html`、`/post.html?p={slug}`、`/admin.html`），视觉复用 `css/style.css` 零改动；页面数据来自 JS `fetch('/api/...')`。

## 目录

| 路径 | 说明 |
|---|---|
| `index.html` / `archive.html` / `post.html` / `admin.html` | Vite MPA 入口（含首屏防暗色闪烁的内联主题脚本） |
| `src/main-*.tsx` | 4 个入口，各挂一个页面组件 |
| `src/lib/api.ts` | fetch 封装：统一解析 `{code,data,message}`，类型化 DTO，错误抛 `ApiError` |
| `src/lib/format.ts` | 日期格式化（本地时区） |
| `src/theme.ts` | `useTheme()`：localStorage + 跟随系统 |
| `src/components/` | SiteHeader / SiteFooter / Tag / PostCard / ArchiveMonth / CommentSection / ErrorBox |
| `src/pages/` | HomePage / ArchivePage / PostPage / admin/（AdminApp + PostEditor） |
| `css/style.css` | 全站样式（原样复用，由各入口 import） |

## 本地开发

需要 pnpm（Node 18+）和可访问的后端（默认 `:3000`，见 `../server/README.md`）：

```bash
pnpm install
pnpm dev          # http://localhost:5173，/api/* 已代理到 :3000
```

## 构建与部署

```bash
pnpm build        # tsc --noEmit && vite build → dist/
```

产物在 `dist/`，4 个 `.html` 入口 + 哈希化静态资源。生产由 Caddy 托管 `dist/`（见 `../deploy/docker-compose.yml`），**部署前必须先 `pnpm build`**。

## 约定

- 文章正文 HTML 由后端渲染并消毒（comrak + ammonia），仅此处使用 `dangerouslySetInnerHTML`；其余用户输入一律经 JSX 转义。
- 主题选择存 `localStorage('riben-theme')`，入口 HTML 内联脚本防暗色闪烁。
- 类型检查纳入构建：`pnpm build` = `tsc --noEmit && vite build`，类型错误会阻断构建。
