/* 归档页：按月分组渲染 /api/posts/archive */
import { useEffect, useState } from 'react';
import { api, type ArchiveMonth as ArchiveMonthData } from '../lib/api';
import ArchiveMonth from '../components/ArchiveMonth';
import ErrorBox from '../components/ErrorBox';
import SiteHeader from '../components/SiteHeader';
import SiteFooter from '../components/SiteFooter';

type LoadState =
  | { kind: 'loading' }
  | { kind: 'ok'; months: ArchiveMonthData[] }
  | { kind: 'error'; message: string };

export default function ArchivePage() {
  const [state, setState] = useState<LoadState>({ kind: 'loading' });

  useEffect(() => {
    let cancelled = false;
    api
      .get<ArchiveMonthData[]>('/api/posts/archive')
      .then((months) => { if (!cancelled) setState({ kind: 'ok', months }); })
      .catch((e: unknown) => {
        if (!cancelled) setState({ kind: 'error', message: e instanceof Error ? e.message : '加载失败' });
      });
    return () => { cancelled = true; };
  }, []);

  return (
    <div className="page">
      <SiteHeader page="archive" />

      <main>
        <section className="archive" aria-label="手账归档">
          <h1 className="page-title">📚 手账集</h1>
          <p className="page-lede">所有的日子都贴在这里了，按月翻旧账～</p>

          <div id="archive-months">
            {state.kind === 'loading' && <div className="load-hint">📖 正在翻旧账…</div>}
            {state.kind === 'error' && <ErrorBox message={state.message} />}
            {state.kind === 'ok' &&
              (state.months.length ? (
                state.months.map((m) => <ArchiveMonth key={m.month} month={m} />)
              ) : (
                <ErrorBox message="📔 手账还是空白页～" />
              ))}
          </div>
        </section>
      </main>

      <SiteFooter />
    </div>
  );
}
