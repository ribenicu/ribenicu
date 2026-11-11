/* 一本日记 · API 封装：统一解析 { code, data, message }，错误抛出 Error 附带 code/status。
 * 对应原 js/api.js，类型化移植。 */

export class ApiError extends Error {
  code: number;
  status: number;

  constructor(message: string, code: number, status: number) {
    super(message);
    this.name = 'ApiError';
    this.code = code;
    this.status = status;
  }
}

interface ApiEnvelope<T> {
  code: number;
  data: T;
  message: string;
}

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  let res: Response;
  try {
    res = await fetch(path, options);
  } catch {
    throw new ApiError('网络连接失败，请稍后再试', -1, 0);
  }
  let body: ApiEnvelope<T> | null = null;
  try {
    body = (await res.json()) as ApiEnvelope<T>;
  } catch {
    throw new ApiError('服务响应格式错误', -1, res.status);
  }
  if (!res.ok || !body || body.code !== 0) {
    throw new ApiError((body && body.message) || '请求失败', body ? body.code : -1, res.status);
  }
  return body.data;
}

function withJson<T>(method: string, path: string, data?: unknown): Promise<T> {
  return request<T>(path, {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data ?? {}),
  });
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, data?: unknown) => withJson<T>('POST', path, data),
  put: <T>(path: string, data?: unknown) => withJson<T>('PUT', path, data),
  del: (path: string) =>
    request<null>(path, { method: 'DELETE' }).catch((e: unknown) => {
      // DELETE 部分响应可能无 JSON 体，按状态码兜底
      if (e instanceof ApiError && e.code === -1 && (e.status === 200 || e.status === 204)) {
        return null;
      }
      throw e;
    }),
};

/* ---------- 数据类型（与后端 serde DTO 对应） ---------- */

export interface Tag {
  id: number;
  name: string;
  color: string;
}

export interface PostCard {
  slug: string;
  title: string;
  excerpt: string | null;
  sticker: string | null;
  published_at: string;
  tags: Tag[];
}

export interface HomeData {
  posts: PostCard[];
  recorded_days?: number;
  total_posts?: number;
}

export interface ArchivePost {
  slug: string;
  title: string;
  published_at: string;
  tags: Tag[];
}

export interface ArchiveMonth {
  month: string; // "2026-11"
  posts: ArchivePost[];
}

export interface PostDetail {
  id?: number;
  slug: string;
  title: string;
  excerpt: string | null;
  sticker: string | null;
  status?: number;
  published_at: string;
  content_html: string; // 后端 comrak 渲染 + ammonia 消毒
  tags: Tag[];
}

export interface CommentNode {
  id: number;
  author_name: string;
  author_url: string | null;
  content: string;
  created_at: string;
  children: CommentNode[];
}

export interface CommentInput {
  slug: string;
  parent_id: number | null;
  name: string;
  email: string;
  url: string | null;
  content: string;
}

export interface User {
  username: string;
}

export interface AdminPost {
  id: number;
  slug: string;
  title: string;
  sticker: string | null;
  status: number; // 0=草稿 1=已发布
  published_at: string | null;
  comment_count: number;
  tags: Tag[];
  content_md: string;
}

export interface Paged<T> {
  list: T[];
  pending?: number;
}

export interface AdminComment {
  id: number;
  author_name: string;
  content: string;
  status: number; // 0=待审核 1=已通过 2=已拒绝
  created_at: string;
  post_title: string;
}
