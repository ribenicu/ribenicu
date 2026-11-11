/* 管理后台：登录 → 文章 / 评论审核 / 标签 三 tab 单页。
 * 逻辑对照原 js/admin.js 逐段移植。 */
import { useCallback, useEffect, useState, type FormEvent } from 'react';
import { api, ApiError, type AdminComment, type AdminPost, type Paged, type Tag, type User } from '../../lib/api';
import { fmtDate } from '../../lib/format';
import TagChip from '../../components/Tag';
import ErrorBox from '../../components/ErrorBox';
import PostEditor from './PostEditor';
import SiteHeader from '../../components/SiteHeader';
import SiteFooter from '../../components/SiteFooter';

type TabName = 'posts' | 'comments' | 'tags';
type Session = { kind: 'checking' } | { kind: 'in'; user: User } | { kind: 'out' };

const COMMENT_STATUS: [number, string, string][] = [
  [0, 'st-pending', '待审核'],
  [1, 'st-approved', '已通过'],
  [2, 'st-rejected', '已拒绝'],
];

export default function AdminApp() {
  const [session, setSession] = useState<Session>({ kind: 'checking' });
  const [loginTip, setLoginTip] = useState('');
  const [tab, setTab] = useState<TabName>('posts');
  const [tags, setTags] = useState<Tag[]>([]);
  const [posts, setPosts] = useState<AdminPost[] | null>(null);
  const [postError, setPostError] = useState('');
  const [editing, setEditing] = useState<AdminPost | 'new' | null>(null);
  const [commentStatus, setCommentStatus] = useState('0');
  const [comments, setComments] = useState<AdminComment[] | null>(null);
  const [commentError, setCommentError] = useState('');
  const [pending, setPending] = useState(0);
  const [tagTip, setTagTip] = useState('');

  /* ---------- 会话探测：已登录直接进入后台 ---------- */
  useEffect(() => {
    api
      .get<User>('/api/auth/me')
      .then((user) => setSession({ kind: 'in', user }))
      .catch(() => setSession({ kind: 'out' }));
  }, []);

  const loadTags = useCallback(() => {
    api
      .get<Tag[]>('/api/admin/tags')
      .then((list) => setTags(list || []))
      .catch(() => {});
  }, []);

  const loadPosts = useCallback(() => {
    setPostError('');
    api
      .get<Paged<AdminPost>>('/api/admin/posts?page=1&per_page=100')
      .then((data) => setPosts(data.list || []))
      .catch((e: unknown) => setPostError(e instanceof Error ? e.message : '加载失败'));
  }, []);

  const loadComments = useCallback((status: string) => {
    setCommentError('');
    const qs = status === '' ? '?page=1&per_page=100' : `?status=${status}&page=1&per_page=100`;
    api
      .get<Paged<AdminComment>>(`/api/admin/comments${qs}`)
      .then((data) => {
        setPending(data.pending ?? 0);
        setComments(data.list || []);
      })
      .catch((e: unknown) => setCommentError(e instanceof Error ? e.message : '加载失败'));
  }, []);

  // 登录成功后加载初始数据
  useEffect(() => {
    if (session.kind === 'in') {
      loadTags();
      loadPosts();
      loadComments(commentStatus);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session.kind]);

  /* ---------- 登录 / 登出 ---------- */
  function doLogin(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = e.currentTarget;
    setLoginTip('');
    api
      .post<User>('/api/auth/login', {
        username: (form.elements.namedItem('username') as HTMLInputElement).value,
        password: (form.elements.namedItem('password') as HTMLInputElement).value,
      })
      .then((user) => setSession({ kind: 'in', user }))
      .catch((err: unknown) => {
        if (err instanceof ApiError && err.status === 429) {
          setLoginTip('⏳ 尝试太多次了，歇 10 分钟再来');
        } else {
          setLoginTip(`📭 ${err instanceof Error ? err.message : '登录失败'}`);
        }
      });
  }

  function doLogout() {
    api.post('/api/auth/logout').catch(() => {}).finally(() => location.reload());
  }

  /* ---------- 文章 ---------- */
  function deletePost(id: number) {
    if (!confirm('确定删除这篇文章？评论会一起消失。')) return;
    api.del(`/api/admin/posts/${id}`).then(loadPosts);
  }

  /* ---------- 评论审核 ---------- */
  function moderate(id: number, act: 'approve' | 'reject') {
    api.put(`/api/admin/comments/${id}/${act}`).then(() => loadComments(commentStatus));
  }

  function switchCommentStatus(status: string) {
    setCommentStatus(status);
    loadComments(status);
  }

  /* ---------- 标签 ---------- */
  function createTag(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = e.currentTarget;
    setTagTip('');
    api
      .post('/api/admin/tags', {
        name: (form.elements.namedItem('name') as HTMLInputElement).value.trim(),
        color: (form.elements.namedItem('color') as HTMLSelectElement).value,
      })
      .then(() => {
        (form.elements.namedItem('name') as HTMLInputElement).value = '';
        loadTags();
      })
      .catch((err: unknown) => setTagTip(`📭 ${err instanceof Error ? err.message : '创建失败'}`));
  }

  return (
    <div className="page">
      <SiteHeader page="admin" />

      <main>
        {/* ---------- 登录 ---------- */}
        {session.kind !== 'in' && (
          <section className="admin-login" id="admin-login" hidden={session.kind === 'checking'}>
            <span className="sticker s1">管理员入口</span>
            <h1>🔐 翻开管理页</h1>
            <form onSubmit={doLogin}>
              <label className="c-field">
                <span>用户名</span>
                <input name="username" autoComplete="username" required />
              </label>
              <label className="c-field">
                <span>密码</span>
                <input name="password" type="password" autoComplete="current-password" required />
              </label>
              {loginTip && <p className="c-tip">{loginTip}</p>}
              <button className="btn-more c-submit" type="submit">📖 打开手账</button>
            </form>
          </section>
        )}

        {/* ---------- 后台主体 ---------- */}
        {session.kind === 'in' && (
          <section id="admin-main">
            <div className="admin-bar">
              <span className="who">✍️ {session.user.username} 的手账管理台</span>
              <nav className="admin-tabs" aria-label="后台导航">
                <button className={`admin-tab${tab === 'posts' ? ' is-active' : ''}`} type="button" onClick={() => setTab('posts')}>文章</button>
                <button className={`admin-tab${tab === 'comments' ? ' is-active' : ''}`} type="button" onClick={() => setTab('comments')}>
                  评论{pending > 0 && <span className="admin-badge">{pending}</span>}
                </button>
                <button className={`admin-tab${tab === 'tags' ? ' is-active' : ''}`} type="button" onClick={() => setTab('tags')}>标签</button>
              </nav>
              <button className="admin-logout" type="button" onClick={doLogout}>退出登录 →</button>
            </div>

            {/* ----- 文章面板 ----- */}
            {tab === 'posts' && (
              <div className="admin-panel" id="panel-posts">
                <div className="admin-actions">
                  <button className="ai-btn primary" type="button" onClick={() => setEditing('new')}>＋ 写新的一篇</button>
                </div>
                {editing !== null && (
                  <PostEditor
                    post={editing === 'new' ? null : editing}
                    tags={tags}
                    onDone={() => { setEditing(null); loadPosts(); }}
                    onCancel={() => setEditing(null)}
                  />
                )}
                {postError && <ErrorBox message={postError} />}
                <ul className="admin-list" id="admin-posts">
                  {posts === null && !postError && <li className="admin-empty">加载中…</li>}
                  {posts !== null &&
                    (posts.length ? (
                      posts.map((p) => (
                        <li className="admin-item" key={p.id}>
                          <span className="ai-title">{p.title}</span>
                          {p.status === 1
                            ? <span className="ai-status st-pub">已发布</span>
                            : <span className="ai-status st-draft">草稿</span>}
                          {(p.tags || []).map((t) => <TagChip key={t.id} tag={t} />)}
                          <span className="ai-meta">{p.published_at ? fmtDate(p.published_at) : '未发布'} · 💬 {p.comment_count}</span>
                          <a className="ai-btn" href={`post.html?p=${encodeURIComponent(p.slug)}`} target="_blank">查看</a>
                          <button className="ai-btn" type="button" onClick={() => setEditing(p)}>编辑</button>
                          <button className="ai-btn danger" type="button" onClick={() => deletePost(p.id)}>删除</button>
                        </li>
                      ))
                    ) : (
                      <li className="admin-empty">还没有文章，点「写新的一篇」开始吧</li>
                    ))}
                </ul>
              </div>
            )}

            {/* ----- 评论面板 ----- */}
            {tab === 'comments' && (
              <div className="admin-panel" id="panel-comments">
                <div className="c-filter">
                  {([['0', '待审核'], ['1', '已通过'], ['2', '已拒绝'], ['', '全部']] as [string, string][]).map(([v, label]) => (
                    <button key={v} className={`ai-btn${commentStatus === v ? ' primary' : ''}`} type="button" onClick={() => switchCommentStatus(v)}>
                      {label}
                    </button>
                  ))}
                </div>
                {commentError && <ErrorBox message={commentError} />}
                <ul className="admin-list" id="admin-comments">
                  {comments === null && !commentError && <li className="admin-empty">加载中…</li>}
                  {comments !== null &&
                    (comments.length ? (
                      comments.map((c) => {
                        const [_, cls, txt] = COMMENT_STATUS[c.status] || COMMENT_STATUS[0];
                        return (
                          <li className="admin-item" key={c.id}>
                            <span className="ai-title">
                              {c.author_name} <span className="ai-meta">说给《{c.post_title}》</span>
                            </span>
                            <span className={`ai-status ${cls}`}>{txt}</span>
                            <span className="ai-meta">{fmtDate(c.created_at)}</span>
                            {c.status !== 1 && (
                              <button className="ai-btn primary" type="button" onClick={() => moderate(c.id, 'approve')}>通过</button>
                            )}
                            {c.status !== 2 && (
                              <button className="ai-btn danger" type="button" onClick={() => moderate(c.id, 'reject')}>拒绝</button>
                            )}
                            <span className="ai-excerpt">
                              {c.content.slice(0, 120)}{c.content.length > 120 ? '…' : ''}
                            </span>
                          </li>
                        );
                      })
                    ) : (
                      <li className="admin-empty">这个分类箱是空的</li>
                    ))}
                </ul>
              </div>
            )}

            {/* ----- 标签面板 ----- */}
            {tab === 'tags' && (
              <div className="admin-panel" id="panel-tags">
                <form className="tag-inline" onSubmit={createTag}>
                  <label className="c-field">
                    <span>标签名</span>
                    <input name="name" maxLength={64} required />
                  </label>
                  <label className="c-field">
                    <span>颜色</span>
                    <select name="color" defaultValue="t-pink">
                      <option value="t-pink">粉（日常）</option>
                      <option value="t-mint">绿（改造）</option>
                      <option value="t-blue">蓝（食物/风景）</option>
                      <option value="t-yellow">黄（淘货）</option>
                    </select>
                  </label>
                  <button className="ai-btn primary" type="submit">＋ 新标签</button>
                </form>
                {tagTip && <p className="c-tip">{tagTip}</p>}
                <ul className="admin-list" id="admin-tags" style={{ marginTop: 16 }}>
                  {tags.length ? (
                    tags.map((t) => (
                      <li className="admin-item" key={t.id}>
                        <span className="ai-title"><TagChip tag={t} /></span>
                        <span className="ai-meta">#{t.id}</span>
                      </li>
                    ))
                  ) : (
                    <li className="admin-empty">还没有标签</li>
                  )}
                </ul>
              </div>
            )}
          </section>
        )}
      </main>

      <SiteFooter />
    </div>
  );
}
