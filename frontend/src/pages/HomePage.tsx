/* 首页：hero + 最新 6 篇卡片 + 站点统计徽标 + 关于 */
import { useEffect, useState } from 'react';
import { api, type HomeData, type PostCard as PostCardData } from '../lib/api';
import { PostCard, SkeletonCards } from '../components/PostCard';
import ErrorBox from '../components/ErrorBox';
import SiteHeader from '../components/SiteHeader';
import SiteFooter from '../components/SiteFooter';

type LoadState =
  | { kind: 'loading' }
  | { kind: 'ok'; posts: PostCardData[]; days: number }
  | { kind: 'error'; message: string };

export default function HomePage() {
  const [state, setState] = useState<LoadState>({ kind: 'loading' });

  useEffect(() => {
    let cancelled = false;
    api
      .get<HomeData>('/api/posts?limit=6')
      .then((data) => {
        if (cancelled) return;
        setState({
          kind: 'ok',
          posts: data.posts,
          days: data.recorded_days ?? data.total_posts ?? 0,
        });
      })
      .catch((e: unknown) => {
        if (!cancelled) setState({ kind: 'error', message: e instanceof Error ? e.message : '加载失败' });
      });
    return () => { cancelled = true; };
  }, []);

  return (
    <div className="page">
      <SiteHeader page="home" />

      <section className="hero">
        <h1 className="hero-title"><span className="w1">一本</span><span className="w2">日</span><span className="w3">记</span></h1>
        <svg className="squiggle" width="180" height="14" viewBox="0 0 180 14" aria-hidden="true">
          <path d="M2 8 Q 14 2, 26 8 T 50 8 T 74 8 T 98 8 T 122 8 T 146 8 T 170 8" fill="none" stroke="var(--pink)" strokeWidth="3" strokeLinecap="round"/>
        </svg>
        <p className="hero-lede">把每天的小确幸剪下来、贴上去、画个圈！这里是我的生活手账：吃了一顿好饭、看到一次晚霞、捡到一句好听的话，统统收进来～</p>
        <ul className="hero-badges" aria-label="站点小档案">
          <li className="badge b-pink">📔 <span>已记录 {state.kind === 'ok' ? state.days : '…'} 天</span></li>
          <li className="badge b-mint">🌈 持续更新中</li>
          <li className="badge b-blue">✂️ 手账风格</li>
        </ul>
      </section>

      <main>
        <section className="entries" id="entries" aria-label="最新日记">
          <h2 className="section-title"><span className="marker">✏️</span> 最新一页</h2>

          <div className="card-grid">
            {state.kind === 'loading' && <SkeletonCards n={6} />}
            {state.kind === 'error' && <ErrorBox message={state.message} />}
            {state.kind === 'ok' &&
              (state.posts.length ? (
                state.posts.map((p, i) => <PostCard key={p.slug} post={p} index={i} />)
              ) : (
                <ErrorBox message="📔 还没有贴上来任何一页，敬请期待～" />
              ))}
          </div>

          <div className="more-wrap">
            <a className="btn-more" href="archive.html">📖 翻翻旧账（归档）</a>
          </div>
        </section>

        <section className="about" id="about" aria-label="关于本站">
          <div className="about-card">
            <span className="about-sticker">关于我</span>
            <p className="about-text">你好呀，我是这本日记的主人。白天写代码，晚上写手账，相信「写下来就不会忘记」。这里不追热点、不设清单，只收录生活里那些微小而确定的快乐。</p>
            <p className="about-sign">—— 写于 riben.icu ✍️</p>
          </div>
        </section>
      </main>

      <SiteFooter />
    </div>
  );
}
