/* 首页日记卡片 + 骨架屏：旋转 class / 贴纸样式与原静态版一一对应 */
import type { PostCard as PostCardData } from '../lib/api';
import { fmtDate } from '../lib/format';
import Tag from './Tag';

const ROTATE = ['c1', 'c2', 'c3'];
const STICKER_STYLE = ['s1', 's2', 's3'];

export function PostCard({ post, index }: { post: PostCardData; index: number }) {
  const rot = ROTATE[index % ROTATE.length];
  return (
    <article className={`card ${rot}`}>
      {post.sticker && (
        <span className={`sticker ${STICKER_STYLE[index % STICKER_STYLE.length]}`}>{post.sticker}</span>
      )}
      <time className="date" dateTime={post.published_at}>{fmtDate(post.published_at)}</time>
      <h3 className="card-title">{post.title}</h3>
      <p className="card-text">{post.excerpt || ''}</p>
      <div className="card-foot">
        <span className="card-tags">{(post.tags || []).map((t) => <Tag key={t.id} tag={t} />)}</span>
        <a className="read" href={`post.html?p=${encodeURIComponent(post.slug)}`}>翻开这一页 →</a>
      </div>
    </article>
  );
}

export function SkeletonCards({ n }: { n: number }) {
  return (
    <>
      {Array.from({ length: n }, (_, i) => (
        <article key={i} className={`card ${ROTATE[i % 3]} card-skeleton`} aria-hidden="true">
          <span className={`sticker ${STICKER_STYLE[i % 3]}`}>贴上来中</span>
          <div className="sk-line sk-date"></div>
          <div className="sk-line sk-title"></div>
          <div className="sk-line"></div>
          <div className="sk-line sk-short"></div>
        </article>
      ))}
    </>
  );
}
