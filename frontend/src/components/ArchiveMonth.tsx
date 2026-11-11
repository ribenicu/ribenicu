/* 归档：一个月份块（胶带样式月份标题 + 文章列表） */
import type { ArchiveMonth as ArchiveMonthData } from '../lib/api';
import { fmtDay, fmtMonth } from '../lib/format';
import Tag from './Tag';

export default function ArchiveMonth({ month }: { month: ArchiveMonthData }) {
  return (
    <div className="month">
      <h2 className="month-title"><span className="tape-label">{fmtMonth(month.month)}</span></h2>
      <ul className="archive-list">
        {month.posts.map((p) => (
          <li key={p.slug}>
            <time dateTime={p.published_at}>{fmtDay(p.published_at)}</time>
            <a href={`post.html?p=${encodeURIComponent(p.slug)}`}>{p.title}</a>
            {p.tags && p.tags.length > 0 && <Tag tag={p.tags[0]} />}
          </li>
        ))}
      </ul>
    </div>
  );
}
