/* 一本日记 · 入口：按页面 bootstrap 动态数据 */
(function () {
  'use strict';

  var api = window.Riben.api;
  var R = window.Riben.render;

  /* ---------- 首页 ---------- */
  function initHome() {
    var grid = document.querySelector('.card-grid');
    if (!grid) return;
    grid.innerHTML = R.skeletonCards(6);

    api.get('/api/posts?limit=6')
      .then(function (data) {
        grid.innerHTML = data.posts.length
          ? data.posts.map(R.cardHtml).join('')
          : '<div class="load-error">📔 还没有贴上来任何一页，敬请期待～</div>';
        var days = document.getElementById('badge-days');
        if (days) days.textContent = '已记录 ' + (data.recorded_days || data.total_posts || 0) + ' 天';
      })
      .catch(function (e) {
        grid.innerHTML = R.errorBoxHtml(e.message);
      });
  }

  /* ---------- 归档 ---------- */
  function initArchive() {
    var wrap = document.getElementById('archive-months');
    if (!wrap) return;
    wrap.innerHTML = '<div class="load-hint">📖 正在翻旧账…</div>';

    api.get('/api/posts/archive')
      .then(function (months) {
        wrap.innerHTML = months.length
          ? months.map(R.archiveMonthHtml).join('')
          : '<div class="load-error">📔 手账还是空白页～</div>';
      })
      .catch(function (e) {
        wrap.innerHTML = R.errorBoxHtml(e.message);
      });
  }

  /* ---------- 文章详情 ---------- */
  function currentSlug() {
    var params = new URLSearchParams(location.search);
    return params.get('p') || params.get('id') || '';
  }

  function initPost() {
    var article = document.getElementById('post-article');
    if (!article) return;
    var slug = currentSlug();
    if (!slug) {
      article.innerHTML = '<div class="load-error">📭 缺少文章地址参数（?p=slug）</div>';
      return;
    }

    api.get('/api/posts/' + encodeURIComponent(slug))
      .then(function (post) {
        document.title = post.title + ' · 一本日记';
        var desc = document.querySelector('meta[name="description"]');
        if (desc && post.excerpt) desc.setAttribute('content', post.excerpt.slice(0, 120) + ' 一本日记 ' + R.fmtDate(post.published_at) + '。');

        var sticker = document.getElementById('post-sticker');
        if (sticker) {
          if (post.sticker) {
            sticker.textContent = post.sticker;
            sticker.hidden = false;
          } else {
            sticker.hidden = true;
          }
        }
        document.getElementById('post-title').textContent = post.title;
        var meta = document.getElementById('post-meta');
        meta.innerHTML = '<time datetime="' + R.escapeHtml(post.published_at) + '">' +
          R.fmtDateWithWeek(post.published_at) + '</time>' +
          (post.tags || []).map(R.tagHtml).join('');
        document.getElementById('post-body').innerHTML = post.content_html;

        document.getElementById('post-content').hidden = false;
        initComments(post);
      })
      .catch(function (e) {
        document.getElementById('post-loading').hidden = true;
        document.getElementById('post-error').hidden = false;
        document.getElementById('post-error').innerHTML = R.errorBoxHtml(
          e.status === 404 ? '这一页还没有贴上来' : e.message
        );
      });
  }

  /* ---------- 评论区 ---------- */
  function initComments(post) {
    var list = document.getElementById('comments-list');
    var form = document.getElementById('comment-form');
    if (!list || !form) return;

    var replyTo = { id: null, name: '' };
    var replyTip = document.getElementById('comment-reply-tip');
    var cancelReply = document.getElementById('comment-cancel-reply');
    var tip = document.getElementById('comment-tip');

    function setReply(node) {
      replyTo = node
        ? { id: node.dataset.id, name: node.dataset.name }
        : { id: null, name: '' };
      if (node) {
        replyTip.textContent = '回复 @' + replyTo.name + '：';
        replyTip.hidden = false;
        cancelReply.hidden = false;
        document.getElementById('cf-content').focus();
      } else {
        replyTip.hidden = true;
        cancelReply.hidden = true;
      }
    }

    cancelReply.addEventListener('click', function () { setReply(null); });

    list.addEventListener('click', function (e) {
      var btn = e.target.closest('.c-reply-btn');
      if (btn) setReply(btn);
    });

    api.get('/api/posts/' + encodeURIComponent(post.slug) + '/comments')
      .then(function (tree) {
        if (tree.length) {
          list.innerHTML = R.commentTreeHtml(tree);
        } else {
          list.innerHTML = '<div class="comments-empty">还没有留言，来贴第一张～</div>';
        }
      })
      .catch(function (e) {
        list.innerHTML = R.errorBoxHtml(e.message);
      });

    form.addEventListener('submit', function (e) {
      e.preventDefault();
      tip.hidden = true;
      var data = {
        slug: post.slug,
        parent_id: replyTo.id,
        name: document.getElementById('cf-name').value,
        email: document.getElementById('cf-email').value,
        url: document.getElementById('cf-url').value || null,
        content: document.getElementById('cf-content').value
      };
      api.post('/api/comments', data)
        .then(function () {
          setReply(null);
          form.reset();
          tip.textContent = '📮 留言已收到，审核通过后会贴出来～';
          tip.hidden = false;
        })
        .catch(function (err) {
          tip.textContent = err.status === 429 ? '⏳ 说话慢一点啦，一分钟后再来' : '📭 ' + err.message;
          tip.hidden = false;
        });
    });
  }

  /* ---------- 入口 ---------- */
  document.addEventListener('DOMContentLoaded', function () {
    var page = document.body.dataset.page;
    if (page === 'home') initHome();
    else if (page === 'archive') initArchive();
    else if (page === 'post') initPost();
  });
})();
