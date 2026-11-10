/* 一本日记 · 管理后台页面逻辑（单页原生 JS） */
(function () {
  'use strict';

  var api = window.Riben.api;
  var R = window.Riben.render;
  var esc = R.escapeHtml;

  var els = {};
  var tags = [];          // 全部标签缓存
  var editingPost = null; // 当前编辑的文章 id（null = 新建）
  var commentStatus = '0';

  document.addEventListener('DOMContentLoaded', boot);

  function boot() {
    ['admin-login', 'login-form', 'login-tip', 'admin-main', 'admin-who',
     'admin-logout', 'pending-badge', 'btn-new-post', 'post-editor-wrap',
     'admin-posts', 'admin-comments', 'admin-tags', 'tag-form', 'tag-tip', 'comment-filter'
    ].forEach(function (id) { els[id] = document.getElementById(id); });

    els['login-form'].addEventListener('submit', doLogin);
    els['admin-logout'].addEventListener('click', doLogout);
    els['btn-new-post'].addEventListener('click', function () { openEditor(null); });

    document.querySelectorAll('.admin-tab').forEach(function (tab) {
      tab.addEventListener('click', function () { switchTab(tab.dataset.tab); });
    });

    els['comment-filter'].addEventListener('click', function (e) {
      var btn = e.target.closest('[data-status]');
      if (!btn) return;
      commentStatus = btn.dataset.status;
      loadComments();
    });

    els['tag-form'].addEventListener('submit', createTag);

    // 会话探测：已登录直接进入后台
    api.get('/api/auth/me')
      .then(function (user) { enterAdmin(user); })
      .catch(function () { els['admin-login'].hidden = false; });
  }

  /* ---------- 登录 / 登出 ---------- */
  function doLogin(e) {
    e.preventDefault();
    var tip = els['login-tip'];
    tip.hidden = true;
    api.post('/api/auth/login', {
      username: document.getElementById('lf-username').value,
      password: document.getElementById('lf-password').value
    })
      .then(enterAdmin)
      .catch(function (err) {
        tip.textContent = err.status === 429 ? '⏳ 尝试太多次了，歇 10 分钟再来' : '📭 ' + err.message;
        tip.hidden = false;
      });
  }

  function enterAdmin(user) {
    els['admin-login'].hidden = true;
    els['admin-main'].hidden = false;
    els['admin-who'].textContent = user.username || '';
    loadTags().then(function () {
      loadPosts();
      loadComments();
    });
  }

  function doLogout() {
    api.post('/api/auth/logout').catch(function () {})
      .then(function () { location.reload(); });
  }

  /* ---------- Tab ---------- */
  function switchTab(name) {
    document.querySelectorAll('.admin-tab').forEach(function (t) {
      t.classList.toggle('is-active', t.dataset.tab === name);
    });
    ['posts', 'comments', 'tags'].forEach(function (n) {
      document.getElementById('panel-' + n).hidden = n !== name;
    });
  }

  /* ---------- 标签 ---------- */
  function loadTags() {
    return api.get('/api/admin/tags').then(function (list) {
      tags = list || [];
      renderTagList();
    }).catch(function () {});
  }

  function renderTagList() {
    els['admin-tags'].innerHTML = tags.map(function (t) {
      return '<li class="admin-item"><span class="ai-title">' + R.tagHtml(t) + '</span>' +
        '<span class="ai-meta">#' + t.id + '</span></li>';
    }).join('') || '<li class="admin-empty">还没有标签</li>';
  }

  function createTag(e) {
    e.preventDefault();
    var tip = els['tag-tip'];
    tip.hidden = true;
    api.post('/api/admin/tags', {
      name: document.getElementById('tf-name').value.trim(),
      color: document.getElementById('tf-color').value
    })
      .then(function () {
        document.getElementById('tf-name').value = '';
        loadTags();
      })
      .catch(function (err) {
        tip.textContent = '📭 ' + err.message;
        tip.hidden = false;
      });
  }

  /* ---------- 文章 ---------- */
  function loadPosts() {
    api.get('/api/admin/posts?page=1&per_page=100')
      .then(function (data) {
        els['admin-posts'].innerHTML = (data.list || []).map(postItemHtml).join('') ||
          '<li class="admin-empty">还没有文章，点「写新的一篇」开始吧</li>';
      })
      .catch(function (e) {
        els['admin-posts'].innerHTML = R.errorBoxHtml(e.message);
      });
  }

  function postItemHtml(p) {
    var status = p.status === 1
      ? '<span class="ai-status st-pub">已发布</span>'
      : '<span class="ai-status st-draft">草稿</span>';
    var tagChips = (p.tags || []).map(R.tagHtml).join(' ');
    var date = p.published_at ? R.fmtDate(p.published_at) : '未发布';
    return '<li class="admin-item" data-id="' + p.id + '">' +
      '<span class="ai-title">' + esc(p.title) + '</span>' +
      status + tagChips +
      '<span class="ai-meta">' + date + ' · 💬 ' + p.comment_count + '</span>' +
      '<a class="ai-btn" href="post.html?p=' + encodeURIComponent(p.slug) + '" target="_blank">查看</a>' +
      '<button class="ai-btn" data-act="edit" type="button">编辑</button>' +
      '<button class="ai-btn danger" data-act="del" type="button">删除</button>' +
      '</li>';
  }

  els['admin-posts'].addEventListener('click', function (e) {
    var btn = e.target.closest('[data-act]');
    if (!btn) return;
    var id = Number(btn.closest('.admin-item').dataset.id);
    if (btn.dataset.act === 'edit') {
      api.get('/api/admin/posts?page=1&per_page=100').then(function (data) {
        var post = (data.list || []).find(function (p) { return p.id === id; });
        if (post) openEditor(post);
      });
    } else if (btn.dataset.act === 'del') {
      if (!confirm('确定删除这篇文章？评论会一起消失。')) return;
      api.del('/api/admin/posts/' + id).then(loadPosts);
    }
  });

  /* ---------- 编辑器 ---------- */
  function openEditor(post) {
    editingPost = post ? post.id : null;
    var wrap = els['post-editor-wrap'];
    var title = post ? post.title : '';
    var sticker = post && post.sticker ? post.sticker : '';
    var status = post ? post.status : 1;
    var selected = post ? (post.tags || []).map(function (t) { return t.id; }) : [];

    wrap.innerHTML =
      '<form class="editor" id="editor-form">' +
      '<span class="sticker s2">' + (post ? '编辑这一页' : '新的一篇') + '</span>' +
      '<label class="c-field"><span>标题 *</span>' +
      '<input id="ef-title" maxlength="190" required value="' + esc(title) + '"></label>' +
      '<div class="editor-row">' +
      '<label class="c-field"><span>贴纸文案（可空，卡片角标）</span>' +
      '<input id="ef-sticker" maxlength="32" value="' + esc(sticker) + '" placeholder="下雪啦！"></label>' +
      '<label class="c-field"><span>状态</span>' +
      '<select id="ef-status">' +
      '<option value="1"' + (status === 1 ? ' selected' : '') + '>发布</option>' +
      '<option value="0"' + (status === 0 ? ' selected' : '') + '>草稿</option>' +
      '</select></label>' +
      '</div>' +
      '<span class="c-label">标签</span>' +
      '<div class="editor-tags" id="ef-tags">' +
      tags.map(function (t) {
        return '<label><input type="checkbox" value="' + t.id + '"' +
          (selected.indexOf(t.id) >= 0 ? ' checked' : '') + '> ' + esc(t.name) + '</label>';
      }).join('') +
      '</div>' +
      '<div class="editor-split">' +
      '<label class="c-field"><span>正文（Markdown）*</span>' +
      '<textarea id="ef-content" rows="16" required></textarea></label>' +
      '<div><span class="c-label">预览（后端渲染）</span>' +
      '<div class="editor-preview" id="ef-preview"><div class="load-hint">输入正文后自动预览…</div></div></div>' +
      '</div>' +
      '<p class="c-tip" id="ef-tip" hidden></p>' +
      '<div class="admin-actions" style="margin-top:16px">' +
      '<button class="ai-btn primary" type="submit">💾 保存</button>' +
      '<button class="ai-btn" type="button" id="ef-cancel">取消</button>' +
      '</div>' +
      '</form>';

    wrap.hidden = false;

    var contentEl = document.getElementById('ef-content');
    contentEl.value = post ? (post.content_md || '') : '';
    var previewTimer = null;
    contentEl.addEventListener('input', function () {
      clearTimeout(previewTimer);
      previewTimer = setTimeout(runPreview, 500);
    });

    document.getElementById('ef-cancel').addEventListener('click', function () {
      wrap.hidden = true;
      wrap.innerHTML = '';
    });

    document.getElementById('editor-form').addEventListener('submit', savePost);
    runPreview();

    function runPreview() {
      var md = contentEl.value;
      if (!md.trim()) {
        document.getElementById('ef-preview').innerHTML = '<div class="load-hint">输入正文后自动预览…</div>';
        return;
      }
      api.post('/api/admin/preview', { md: md })
        .then(function (data) {
          document.getElementById('ef-preview').innerHTML = '<div class="post-body">' + data.html + '</div>';
        })
        .catch(function () {
          document.getElementById('ef-preview').innerHTML = R.errorBoxHtml('预览失败');
        });
    }
  }

  function savePost(e) {
    e.preventDefault();
    var tip = document.getElementById('ef-tip');
    tip.hidden = true;
    var tagIds = Array.prototype.map.call(
      document.querySelectorAll('#ef-tags input:checked'),
      function (cb) { return Number(cb.value); }
    );
    var data = {
      title: document.getElementById('ef-title').value.trim(),
      sticker: document.getElementById('ef-sticker').value.trim() || null,
      status: Number(document.getElementById('ef-status').value),
      tag_ids: tagIds,
      content_md: document.getElementById('ef-content').value
    };
    var req = editingPost
      ? api.put('/api/admin/posts/' + editingPost, data)
      : api.post('/api/admin/posts', data);
    req.then(function () {
        els['post-editor-wrap'].hidden = true;
        els['post-editor-wrap'].innerHTML = '';
        loadPosts();
      })
      .catch(function (err) {
        tip.textContent = '📭 ' + err.message;
        tip.hidden = false;
      });
  }

  /* ---------- 评论审核 ---------- */
  function loadComments() {
    var qs = commentStatus === '' ? '' : '?status=' + commentStatus + '&page=1&per_page=100';
    api.get('/api/admin/comments' + qs)
      .then(function (data) {
        var badge = els['pending-badge'];
        if (data.pending > 0) {
          badge.textContent = data.pending;
          badge.hidden = false;
        } else {
          badge.hidden = true;
        }
        els['admin-comments'].innerHTML = (data.list || []).map(commentItemHtml).join('') ||
          '<li class="admin-empty">这个分类箱是空的</li>';
      })
      .catch(function (e) {
        els['admin-comments'].innerHTML = R.errorBoxHtml(e.message);
      });
  }

  function commentItemHtml(c) {
    var statusCls = ['st-pending', 'st-approved', 'st-rejected'][c.status] || 'st-pending';
    var statusTxt = ['待审核', '已通过', '已拒绝'][c.status] || '待审核';
    var btns = '';
    if (c.status !== 1) {
      btns += '<button class="ai-btn primary" data-act="approve" type="button">通过</button>';
    }
    if (c.status !== 2) {
      btns += '<button class="ai-btn danger" data-act="reject" type="button">拒绝</button>';
    }
    return '<li class="admin-item" data-id="' + c.id + '">' +
      '<span class="ai-title">' + esc(c.author_name) + ' <span class="ai-meta">说给《' + esc(c.post_title) + '》</span></span>' +
      '<span class="ai-status ' + statusCls + '">' + statusTxt + '</span>' +
      '<span class="ai-meta">' + R.fmtDate(c.created_at) + '</span>' +
      btns +
      '<span class="ai-excerpt">' + esc(c.content.slice(0, 120)) + (c.content.length > 120 ? '…' : '') + '</span>' +
      '</li>';
  }

  els['admin-comments'].addEventListener('click', function (e) {
    var btn = e.target.closest('[data-act]');
    if (!btn) return;
    var id = btn.closest('.admin-item').dataset.id;
    var act = btn.dataset.act === 'approve' ? 'approve' : 'reject';
    btn.disabled = true;
    api.put('/api/admin/comments/' + id + '/' + act)
      .then(loadComments)
      .catch(function () { btn.disabled = false; });
  });
})();
