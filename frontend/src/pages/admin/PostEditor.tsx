/* Markdown 文章编辑器：标题 / 贴纸 / 状态 / 标签勾选 / 正文 + 防抖预览（后端渲染）。
 * 逻辑对照原 js/admin.js 的 openEditor/savePost 移植。 */
import { useEffect, useRef, useState, type FormEvent } from 'react';
import { api, type AdminPost, type Tag } from '../../lib/api';

interface Props {
  post: AdminPost | null; // null = 新建
  tags: Tag[];
  onDone: () => void;
  onCancel: () => void;
}

export default function PostEditor({ post, tags, onDone, onCancel }: Props) {
  const [contentMd, setContentMd] = useState(post?.content_md ?? '');
  const [previewHtml, setPreviewHtml] = useState<string | null>(null); // null = 提示语
  const [tip, setTip] = useState('');
  const [saving, setSaving] = useState(false);
  const previewTimer = useRef<number | null>(null);

  const runPreview = useRef((md: string) => {
    if (!md.trim()) {
      setPreviewHtml(null);
      return;
    }
    api
      .post<{ html: string }>('/api/admin/preview', { md })
      .then((data) => setPreviewHtml(data.html))
      .catch(() => setPreviewHtml('__error__'));
  });

  // 打开编辑器时先预览一次已有正文
  useEffect(() => {
    runPreview.current(contentMd);
    return () => {
      if (previewTimer.current) window.clearTimeout(previewTimer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function onContentInput(value: string) {
    setContentMd(value);
    if (previewTimer.current) window.clearTimeout(previewTimer.current);
    previewTimer.current = window.setTimeout(() => runPreview.current(value), 500);
  }

  function save(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = e.currentTarget;
    setTip('');
    const tagIds = Array.from(form.querySelectorAll<HTMLInputElement>('#ef-tags input:checked')).map(
      (cb) => Number(cb.value)
    );
    const data = {
      title: (form.elements.namedItem('title') as HTMLInputElement).value.trim(),
      sticker: (form.elements.namedItem('sticker') as HTMLInputElement).value.trim() || null,
      status: Number((form.elements.namedItem('status') as HTMLSelectElement).value),
      tag_ids: tagIds,
      content_md: contentMd,
    };
    setSaving(true);
    const req = post ? api.put(`/api/admin/posts/${post.id}`, data) : api.post('/api/admin/posts', data);
    req.then(onDone).catch((err: unknown) => {
      setTip(`📭 ${err instanceof Error ? err.message : '保存失败'}`);
      setSaving(false);
    });
  }

  const selectedTagIds = post ? (post.tags || []).map((t) => t.id) : [];

  return (
    <div id="post-editor-wrap">
      <form className="editor" id="editor-form" onSubmit={save}>
        <span className="sticker s2">{post ? '编辑这一页' : '新的一篇'}</span>
        <label className="c-field">
          <span>标题 *</span>
          <input name="title" maxLength={190} required defaultValue={post?.title ?? ''} />
        </label>
        <div className="editor-row">
          <label className="c-field">
            <span>贴纸文案（可空，卡片角标）</span>
            <input name="sticker" maxLength={32} defaultValue={post?.sticker ?? ''} placeholder="下雪啦！" />
          </label>
          <label className="c-field">
            <span>状态</span>
            <select name="status" defaultValue={post ? String(post.status) : '1'}>
              <option value="1">发布</option>
              <option value="0">草稿</option>
            </select>
          </label>
        </div>
        <span className="c-label">标签</span>
        <div className="editor-tags" id="ef-tags">
          {tags.map((t) => (
            <label key={t.id}>
              <input type="checkbox" value={t.id} defaultChecked={selectedTagIds.includes(t.id)} /> {t.name}
            </label>
          ))}
        </div>
        <div className="editor-split">
          <label className="c-field">
            <span>正文（Markdown）*</span>
            <textarea
              name="content"
              rows={16}
              required
              value={contentMd}
              onChange={(e) => onContentInput(e.target.value)}
            ></textarea>
          </label>
          <div>
            <span className="c-label">预览（后端渲染）</span>
            <div className="editor-preview" id="ef-preview">
              {previewHtml === null && <div className="load-hint">输入正文后自动预览…</div>}
              {previewHtml === '__error__' && <div className="load-error">📭 预览失败</div>}
              {previewHtml !== null && previewHtml !== '__error__' && (
                <div className="post-body" dangerouslySetInnerHTML={{ __html: previewHtml }} />
              )}
            </div>
          </div>
        </div>
        {tip && <p className="c-tip">{tip}</p>}
        <div className="admin-actions" style={{ marginTop: 16 }}>
          <button className="ai-btn primary" type="submit" disabled={saving}>💾 保存</button>
          <button className="ai-btn" type="button" onClick={onCancel}>取消</button>
        </div>
      </form>
    </div>
  );
}
