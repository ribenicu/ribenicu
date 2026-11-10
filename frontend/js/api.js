/* 一本日记 · API 封装：统一解析 { code, data, message }，错误抛出 Error 附带 code/status */
(function () {
  'use strict';

  async function request(path, options) {
    var opts = options || {};
    var res;
    try {
      res = await fetch(path, opts);
    } catch (e) {
      throw new Error('网络连接失败，请稍后再试');
    }
    var body = null;
    try {
      body = await res.json();
    } catch (e) {
      throw new Error('服务响应格式错误');
    }
    if (!res.ok || !body || body.code !== 0) {
      var err = new Error((body && body.message) || '请求失败');
      err.code = body ? body.code : -1;
      err.status = res.status;
      throw err;
    }
    return body.data;
  }

  function withJson(method, path, data) {
    return request(path, {
      method: method,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data || {})
    });
  }

  window.Riben = window.Riben || {};
  window.Riben.api = {
    get: function (path) { return request(path); },
    post: function (path, data) { return withJson('POST', path, data); },
    put: function (path, data) { return withJson('PUT', path, data); },
    del: function (path) { return request(path, { method: 'DELETE' })
      .catch(function (e) {
        // DELETE 部分响应可能无 JSON 体，按状态码兜底
        if (e.code === -1 && (e.status === 200 || e.status === 204)) return null;
        throw e;
      }); }
  };
})();
