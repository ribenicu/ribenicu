/* 一本日记 · 主题切换：移植原 js/theme.js。
 * 初始主题由入口 HTML 的内联脚本写入 <html data-theme>（防闪烁），这里接管后续切换。
 * 选择存 localStorage('riben-theme')；未手动选择过时跟随系统。 */

import { useCallback, useEffect, useState } from 'react';

export type Theme = 'light' | 'dark';

const META_COLORS: Record<Theme, string> = {
  light: '#fff9f2',
  dark: '#26202b',
};

function readStored(): Theme | null {
  try {
    const t = localStorage.getItem('riben-theme');
    return t === 'light' || t === 'dark' ? t : null;
  } catch {
    return null;
  }
}

function currentTheme(): Theme {
  return document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light';
}

function applyMeta(theme: Theme) {
  const meta = document.getElementById('meta-theme-color');
  if (meta) meta.setAttribute('content', META_COLORS[theme]);
}

export function useTheme() {
  const [theme, setTheme] = useState<Theme>(currentTheme());

  const set = useCallback((t: Theme) => {
    document.documentElement.dataset.theme = t;
    try {
      localStorage.setItem('riben-theme', t);
    } catch {
      /* 隐私模式等场景下忽略 */
    }
    applyMeta(t);
    setTheme(t);
  }, []);

  const toggle = useCallback(() => {
    set(currentTheme() === 'dark' ? 'light' : 'dark');
  }, [set]);

  // 跟随系统主题变化（仅当用户未手动选择过）
  useEffect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = (e: MediaQueryListEvent) => {
      if (!readStored()) set(e.matches ? 'dark' : 'light');
    };
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, [set]);

  return { theme, toggle };
}
