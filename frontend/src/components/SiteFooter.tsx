export default function SiteFooter() {
  return (
    <footer className="site-footer">
      <span>♡ 2026 一本日记</span>
      <span className="footer-dots" aria-hidden="true"><i></i><i></i><i></i></span>
      <span>riben.icu · 每天贴一页</span>
      <a
        className="footer-icp"
        href="https://beian.miit.gov.cn/"
        target="_blank"
        rel="noopener noreferrer"
      >
        <svg
          className="footer-icp-icon"
          viewBox="0 0 24 24"
          width="14"
          height="14"
          aria-hidden="true"
        >
          {/* 盾牌 + 五角星（公安备案徽标风格） */}
          <path
            d="M12 1.5 3.5 4.5v7c0 5 3.6 9.6 8.5 11 4.9-1.4 8.5-6 8.5-11v-7L12 1.5z"
            fill="currentColor"
          />
          <path
            d="M12 6.2l1.35 2.73 3.01.44-2.18 2.12.52 3-2.7-1.42-2.7 1.42.52-3-2.18-2.12 3.01-.44L12 6.2z"
            fill="var(--bg, #fff)"
          />
        </svg>
        湘ICP备17012851号-6
      </a>
    </footer>
  );
}
