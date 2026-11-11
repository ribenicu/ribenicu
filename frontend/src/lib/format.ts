/* 一本日记 · 日期格式化：移植原 js/render.js 的日期工具。
 * 后端返回 RFC3339（UTC），按本地时区展示。 */

const WEEK = ['星期日', '星期一', '星期二', '星期三', '星期四', '星期五', '星期六'];

function toDate(iso: string): Date | null {
  const d = new Date(iso);
  return isNaN(d.getTime()) ? null : d;
}

export function fmtDate(iso: string): string {
  const d = toDate(iso);
  if (!d) return '';
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${d.getFullYear()}.${m}.${day}`;
}

export function fmtDateWithWeek(iso: string): string {
  const d = toDate(iso);
  if (!d) return fmtDate(iso);
  return `${fmtDate(iso)} · ${WEEK[d.getDay()]}`;
}

export function fmtMonth(month: string): string {
  // "2026-11" → "2026 年 11 月"
  const parts = String(month).split('-');
  if (parts.length !== 2) return String(month);
  return `${parts[0]} 年 ${parseInt(parts[1], 10)} 月`;
}

export function fmtDay(iso: string): string {
  const d = toDate(iso);
  if (!d) return '';
  return `${String(d.getMonth() + 1).padStart(2, '0')}.${String(d.getDate()).padStart(2, '0')}`;
}
