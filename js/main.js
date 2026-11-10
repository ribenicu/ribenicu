/* 一本日记 · 主题切换 */
(function () {
  var root = document.documentElement;
  var toggle = document.getElementById('theme-toggle');
  var meta = document.getElementById('meta-theme-color');

  function applyMeta(theme) {
    if (meta) meta.setAttribute('content', theme === 'dark' ? '#26202b' : '#fff9f2');
  }

  function syncToggle(theme) {
    if (!toggle) return;
    toggle.setAttribute('aria-pressed', String(theme === 'dark'));
    toggle.setAttribute('aria-label', theme === 'dark' ? '切换到浅色模式' : '切换到暗色模式');
  }

  function setTheme(theme) {
    root.dataset.theme = theme;
    try { localStorage.setItem('riben-theme', theme); } catch (e) {}
    applyMeta(theme);
    syncToggle(theme);
  }

  applyMeta(root.dataset.theme || 'light');
  syncToggle(root.dataset.theme || 'light');

  if (toggle) {
    toggle.addEventListener('click', function () {
      setTheme(root.dataset.theme === 'dark' ? 'light' : 'dark');
    });
  }

  // 跟随系统主题变化（仅当用户未手动选择过）
  var mq = window.matchMedia('(prefers-color-scheme: dark)');
  mq.addEventListener('change', function (e) {
    var stored = null;
    try { stored = localStorage.getItem('riben-theme'); } catch (err) {}
    if (!stored) setTheme(e.matches ? 'dark' : 'light');
  });
})();
