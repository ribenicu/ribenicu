/* 文章详情页：?p={slug}（兼容旧 ?id=）。
 * 正文 content_html 由后端 comrak 渲染 + ammonia 消毒，经 dangerouslySetInnerHTML 插入；
 * title / description 由 JS 动态写入 <head>。 */
import { useEffect, useState } from 'react';
import { api, type PostDetail } from '../lib/api';
import { fmtDate, fmtDateWithWeek } from '../lib/format';
import Tag from '../components/Tag';
import ErrorBox from '../components/ErrorBox';
import CommentSection from '../components/CommentSection';
import SiteHeader from '../components/SiteHeader';
import SiteFooter from '../components/SiteFooter';

function currentSlug(): string {
  const params = new URLSearchParams(location.search);
  return params.get('p') || params.get('id') || '';
}

type LoadState =
  | { kind: 'loading' }
  | { kind: 'ok'; post: PostDetail }
  | { kind: 'error'; message: string; is404: boolean };

export default function PostPage() {
  const [state, setState] = useState<LoadState>({ kind: 'loading' });
  const slug = currentSlug();

  useEffect(() => {
    if (!slug) {
      setState({ kind: 'error', message: '📭 缺少文章地址参数（?p=slug）', is404: false });
      return;
    }
    let cancelled = false;
    api
      .get<PostDetail>(`/api/posts/${encodeURIComponent(slug)}`)
      .then((post) => {
        if (cancelled) return;
        document.title = `${post.title} · 一本日记`;
        const desc = document.querySelector('meta[name="description"]');
        if (desc && post.excerpt) {
          desc.setAttribute('content', `${post.excerpt.slice(0, 120)} 一本日记 ${fmtDate(post.published_at)}。`);
        }
        setState({ kind: 'ok', post });
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        const status = e instanceof Error && 'status' in e ? (e as { status?: number }).status : undefined;
        setState({
          kind: 'error',
          message: status === 404 ? '这一页还没有贴上来' : e instanceof Error ? e.message : '加载失败',
          is404: status === 404,
        });
      });
    return () => { cancelled = true; };
  }, [slug]);

  return (
    <div className="page">
      <SiteHeader page="post" />

      <main>
        {state.kind === 'loading' && <p className="load-hint">📖 正在翻开这一页…</p>}
        {state.kind === 'error' && <ErrorBox message={state.message} />}

        <article className="post" id="post-article">
          {state.kind === 'ok' && (
            <div id="post-content">
              {state.post.sticker && <span className="sticker s1 post-sticker">{state.post.sticker}</span>}
              <header className="post-header">
                <h1 className="post-title">{state.post.title}</h1>
                <div className="post-meta">
                  <time dateTime={state.post.published_at}>{fmtDateWithWeek(state.post.published_at)}</time>
                  {(state.post.tags || []).map((t) => <Tag key={t.id} tag={t} />)}
                </div>
              </header>

              {/* 可信 HTML：后端已消毒 */}
              <div className="post-body" dangerouslySetInnerHTML={{ __html: state.post.content_html }} />

              <footer className="post-foot">
                <a className="read" href="archive.html">← 翻回手账集</a>
                <a className="read" href="index.html">回到首页 →</a>
              </footer>
            </div>
          )}
        </article>

        {state.kind === 'ok' && <CommentSection slug={state.post.slug} />}
      </main>

      <SiteFooter />
    </div>
  );
}
