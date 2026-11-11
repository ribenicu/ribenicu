/* 站点页头：logo + 主导航 + 主题切换按钮（SVG 图标照搬原静态版） */
import { useTheme } from '../theme';

export type PageName = 'home' | 'archive' | 'post' | 'admin';

function ThemeToggle() {
  const { theme, toggle } = useTheme();
  const dark = theme === 'dark';
  return (
    <button
      className="theme-toggle"
      id="theme-toggle"
      type="button"
      aria-label={dark ? '切换到浅色模式' : '切换到暗色模式'}
      aria-pressed={dark}
      onClick={toggle}
    >
      <svg className="icon-sun" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" aria-hidden="true">
        <circle cx="12" cy="12" r="4.2"/><path d="M12 2.5v2.4M12 19.1v2.4M4.3 4.3l1.7 1.7M18 18l1.7 1.7M2.5 12h2.4M19.1 12h2.4M4.3 19.7 6 18M18 6l1.7-1.7"/>
      </svg>
      <svg className="icon-moon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <path d="M20.5 14.2A8.5 8.5 0 1 1 9.8 3.5a7 7 0 1 0 10.7 10.7Z"/>
      </svg>
    </button>
  );
}

export default function SiteHeader({ page }: { page: PageName }) {
  return (
    <header className="site-header">
      <a className="logo" href="index.html">一本日记<span className="dot"> ●</span></a>
      <nav className="site-nav" aria-label="主导航">
        {page === 'admin' ? (
          <>
            <a className="nav-pill" href="index.html">首页</a>
            <a className="nav-pill" href="archive.html">手账集</a>
          </>
        ) : page === 'home' ? (
          <>
            <a className="nav-pill is-active" href="index.html" aria-current="page">首页</a>
            <a className="nav-pill" href="#entries">手账集</a>
            <a className="nav-pill" href="#about">关于我</a>
          </>
        ) : page === 'archive' ? (
          <>
            <a className="nav-pill" href="index.html">首页</a>
            <a className="nav-pill is-active" href="archive.html" aria-current="page">手账集</a>
            <a className="nav-pill" href="index.html#about">关于我</a>
          </>
        ) : (
          <>
            <a className="nav-pill" href="index.html">首页</a>
            <a className="nav-pill" href="archive.html">手账集</a>
            <a className="nav-pill" href="index.html#about">关于我</a>
          </>
        )}
        <ThemeToggle />
      </nav>
    </header>
  );
}
