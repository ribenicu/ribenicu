/* 一本日记 · DOM 渲染模板：卡片 / 归档 / 评论。
 * 原则：所有用户输入一律 escapeHtml；仅文章正文 content_html 来自后端消毒后的 HTML。 */
(function () {
  'use strict';

  function escapeHtml(s) {
    if (s === null || s === undefined) return '';
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  var WEEK = ['星期日', '星期一', '星期二', '星期三', '星期四', '星期五', '星期六'];

  function toDate(iso) {
    // 后端返回 RFC3339（UTC），按本地时区展示
    var d = new Date(iso);
    return isNaN(d.getTime()) ? null : d;
  }

  function fmtDate(iso) {
    var d = toDate(iso);
    if (!d) return '';
    var m = String(d.getMonth() + 1).padStart(2, '0');
    var day = String(d.getDate()).padStart(2, '0');
    return d.getFullYear() + '.' + m + '.' + day;
  }

  function fmtDateWithWeek(iso) {
    var d = toDate(iso);
    if (!d) return fmtDate(iso);
    return fmtDate(iso) + ' · ' + WEEK[d.getDay()];
  }

  function fmtMonth(month) {
    // "2026-11" → "2026 年 11 月"
    var parts = String(month).split('-');
    if (parts.length !== 2) return escapeHtml(month);
    return escapeHtml(parts[0]) + ' 年 ' + escapeHtml(String(parseInt(parts[1], 10))) + ' 月';
  }

  function fmtDay(iso) {
    var d = toDate(iso);
    if (!d) return '';
    return String(d.getMonth() + 1).padStart(2, '0') + '.' + String(d.getDate()).padStart(2, '0');
  }

  function tagHtml(tag) {
    var color = /^t-(pink|mint|blue|yellow)$/.test(tag.color) ? tag.color : 't-pink';
    return '<span class="tag ' + color + '">' + escapeHtml(tag.name) + '</span>';
  }

  var ROTATE = ['c1', 'c2', 'c3'];
  var STICKER_STYLE = ['s1', 's2', 's3'];

  /* 首页卡片：与静态版结构一一对应 */
  function cardHtml(post, index) {
    var rot = ROTATE[index % ROTATE.length];
    var sticker = post.sticker
      ? '<span class="sticker ' + STICKER_STYLE[index % STICKER_STYLE.length] + '">' + escapeHtml(post.sticker) + '</span>'
      : '';
    var tags = (post.tags || []).map(tagHtml).join('');
    return (
      '<article class="card ' + rot + '">' +
      sticker +
      '<time class="date" datetime="' + escapeHtml(post.published_at) + '">' + fmtDate(post.published_at) + '</time>' +
      '<h3 class="card-title">' + escapeHtml(post.title) + '</h3>' +
      '<p class="card-text">' + escapeHtml(post.excerpt || '') + '</p>' +
      '<div class="card-foot">' +
      '<span class="card-tags">' + tags + '</span>' +
      '<a class="read" href="post.html?p=' + encodeURIComponent(post.slug) + '">翻开这一页 →</a>' +
      '</div>' +
      '</article>'
    );
  }

  function skeletonCards(n) {
    var out = '';
    for (var i = 0; i < n; i++) {
      out += '<article class="card ' + ROTATE[i % 3] + ' card-skeleton" aria-hidden="true">' +
        '<span class="sticker ' + STICKER_STYLE[i % 3] + '">贴上来中</span>' +
        '<div class="sk-line sk-date"></div>' +
        '<div class="sk-line sk-title"></div>' +
        '<div class="sk-line"></div><div class="sk-line sk-short"></div>' +
        '</article>';
    }
    return out;
  }

  /* 归档：一个月份块 */
  function archiveMonthHtml(month) {
    var items = month.posts.map(function (p) {
      var tag = p.tags && p.tags.length ? tagHtml(p.tags[0]) : '';
      return '<li><time datetime="' + escapeHtml(p.published_at) + '">' + fmtDay(p.published_at) + '</time>' +
        '<a href="post.html?p=' + encodeURIComponent(p.slug) + '">' + escapeHtml(p.title) + '</a>' + tag + '</li>';
    }).join('');
    return '<div class="month">' +
      '<h2 class="month-title"><span class="tape-label">' + fmtMonth(month.month) + '</span></h2>' +
      '<ul class="archive-list">' + items + '</ul>' +
      '</div>';
  }

  /* 评论：单层节点（children 由调用方递归） */
  function commentNodeHtml(node) {
    var author = node.author_url
      ? '<a class="c-author" href="' + escapeHtml(node.author_url) + '" target="_blank" rel="nofollow ugc noopener noreferrer">' + escapeHtml(node.author_name) + '</a>'
      : '<span class="c-author">' + escapeHtml(node.author_name) + '</span>';
    var content = escapeHtml(node.content).replace(/\n/g, '<br>');
    return '<div class="comment" data-id="' + node.id + '">' +
      '<div class="c-head">' + author +
      '<time datetime="' + escapeHtml(node.created_at) + '">' + fmtDate(node.created_at) + '</time>' +
      '<button class="c-reply-btn" type="button" data-id="' + node.id + '" data-name="' + escapeHtml(node.author_name) + '">回复</button>' +
      '</div>' +
      '<div class="c-body">' + content + '</div>' +
      '</div>';
  }

  function commentTreeHtml(nodes) {
    return nodes.map(function (n) {
      var children = (n.children || []).map(function (c) {
        return '<div class="c-children">' + commentNodeHtml(c) + '</div>';
      }).join('');
      return commentNodeHtml(n) + children;
    }).join('');
  }

  function errorBoxHtml(message) {
    return '<div class="load-error">📭 ' + escapeHtml(message || '加载失败，请稍后再试') + '</div>';
  }

  window.Riben = window.Riben || {};
  window.Riben.render = {
    escapeHtml: escapeHtml,
    fmtDate: fmtDate,
    fmtDateWithWeek: fmtDateWithWeek,
    fmtMonth: fmtMonth,
    tagHtml: tagHtml,
    cardHtml: cardHtml,
    skeletonCards: skeletonCards,
    archiveMonthHtml: archiveMonthHtml,
    commentTreeHtml: commentTreeHtml,
    errorBoxHtml: errorBoxHtml
  };
})();
