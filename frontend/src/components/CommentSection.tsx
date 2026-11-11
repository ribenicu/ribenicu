/* 留言角：评论树展示 + 回复 + 发表（审核后展示）。用户输入经 JSX 转义；
 * URL 输出 rel="nofollow ugc noopener noreferrer"，与原静态版一致。 */
import { useEffect, useState, type FormEvent } from 'react';
import { api, ApiError, type CommentNode } from '../lib/api';
import { fmtDate } from '../lib/format';
import ErrorBox from './ErrorBox';

function CommentNodeView({ node, onReply }: { node: CommentNode; onReply: (n: CommentNode) => void }) {
  return (
    <div className="comment" data-id={node.id}>
      <div className="c-head">
        {node.author_url ? (
          <a className="c-author" href={node.author_url} target="_blank" rel="nofollow ugc noopener noreferrer">
            {node.author_name}
          </a>
        ) : (
          <span className="c-author">{node.author_name}</span>
        )}
        <time dateTime={node.created_at}>{fmtDate(node.created_at)}</time>
        <button className="c-reply-btn" type="button" onClick={() => onReply(node)}>回复</button>
      </div>
      {/* 评论为纯文本：按换行分段展示，等效原版的 \n → <br> */}
      <div className="c-body">
        {node.content.split('\n').map((line, i) => (
          <span key={i}>{line}{i < node.content.split('\n').length - 1 && <br />}</span>
        ))}
      </div>
    </div>
  );
}

export default function CommentSection({ slug }: { slug: string }) {
  const [tree, setTree] = useState<CommentNode[] | null>(null);
  const [loadError, setLoadError] = useState('');
  const [replyTo, setReplyTo] = useState<CommentNode | null>(null);
  const [tip, setTip] = useState(''); // 提交结果提示（空 = 隐藏）
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api
      .get<CommentNode[]>(`/api/posts/${encodeURIComponent(slug)}/comments`)
      .then((data) => { if (!cancelled) setTree(data); })
      .catch((e: unknown) => {
        if (!cancelled) setLoadError(e instanceof Error ? e.message : '加载失败');
      });
    return () => { cancelled = true; };
  }, [slug]);

  function onSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = e.currentTarget;
    setTip('');
    const data = {
      slug,
      parent_id: replyTo ? replyTo.id : null,
      name: (form.elements.namedItem('name') as HTMLInputElement).value,
      email: (form.elements.namedItem('email') as HTMLInputElement).value,
      url: (form.elements.namedItem('url') as HTMLInputElement).value || null,
      content: (form.elements.namedItem('content') as HTMLTextAreaElement).value,
    };
    setSubmitting(true);
    api
      .post('/api/comments', data)
      .then(() => {
        setReplyTo(null);
        form.reset();
        setTip('📮 留言已收到，审核通过后会贴出来～');
      })
      .catch((err: unknown) => {
        if (err instanceof ApiError && err.status === 429) {
          setTip('⏳ 说话慢一点啦，一分钟后再来');
        } else {
          setTip(`📭 ${err instanceof Error ? err.message : '提交失败'}`);
        }
      })
      .finally(() => setSubmitting(false));
  }

  return (
    <section className="comments" id="comments" aria-label="留言角">
      <h2 className="comments-title"><span className="marker">📮</span> 留言角</h2>

      <div className="comments-list">
        {tree === null && !loadError && <div className="load-hint">📮 正在取留言…</div>}
        {loadError && <ErrorBox message={loadError} />}
        {tree !== null &&
          (tree.length ? (
            tree.map((n) => (
              <div key={n.id}>
                <CommentNodeView node={n} onReply={setReplyTo} />
                {(n.children || []).map((c) => (
                  <div className="c-children" key={c.id}>
                    <CommentNodeView node={c} onReply={setReplyTo} />
                  </div>
                ))}
              </div>
            ))
          ) : (
            <div className="comments-empty">还没有留言，来贴第一张～</div>
          ))}
      </div>

      <form className="comment-form" onSubmit={onSubmit}>
        <p className="c-reply-line">
          {replyTo && (
            <>
              <span className="c-reply-tip">回复 @{replyTo.author_name}：</span>
              <button type="button" className="c-cancel" onClick={() => setReplyTo(null)}>取消回复</button>
            </>
          )}
        </p>
        <div className="c-row">
          <label className="c-field">
            <span>昵称 *</span>
            <input name="name" maxLength={64} required autoComplete="nickname" />
          </label>
          <label className="c-field">
            <span>邮箱 *（不公开）</span>
            <input name="email" type="email" maxLength={190} required autoComplete="email" />
          </label>
          <label className="c-field">
            <span>网址（可选）</span>
            <input name="url" type="url" maxLength={190} placeholder="https://" />
          </label>
        </div>
        <label className="c-field c-field-full">
          <span>想说的话 *</span>
          <textarea name="content" rows={5} maxLength={2000} required></textarea>
        </label>
        {tip && <p className="c-tip">{tip}</p>}
        <button className="btn-more c-submit" type="submit" disabled={submitting}>
          📮 贴上去（审核后展示）
        </button>
      </form>
    </section>
  );
}
