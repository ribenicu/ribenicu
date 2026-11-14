//! Session 鉴权中间件：校验 Cookie → Redis session:{token} → 用户。
//! 失败一律 401，由前端跳登录；session 滑动续期。

use crate::error::AppError;
use crate::models::user::User;
use crate::services::cache::keys;
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use redis::AsyncCommands;

fn token_from_cookie(req: &Request) -> Option<String> {
    let header = req.headers().get(http_header())?;
    let value = header.to_str().ok()?;
    for pair in value.split(';') {
        let (k, v) = pair.split_once('=')?;
        if k.trim() == crate::config::Config::cookie_name() {
            let token = v.trim().trim_matches('"');
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

fn http_header() -> axum::http::HeaderName {
    axum::http::header::COOKIE
}

/// 从请求头提取 ADMIN_API_TOKEN：支持 `x-admin-token` 与 `Authorization: Bearer <token>`
fn token_from_header(req: &Request) -> Option<String> {
    if let Some(value) = req.headers().get("x-admin-token") {
        let token = value.to_str().ok()?.trim();
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }
    if let Some(value) = req.headers().get(axum::http::header::AUTHORIZATION) {
        let value = value.to_str().ok()?;
        if let Some(token) = value.strip_prefix("Bearer ") {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

pub(crate) fn tokens_equal(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let unauthorized = || AppError::Unauthorized("未登录或会话已过期".into());

    // ADMIN_API_TOKEN 旁路：配置后携带正确 token 的请求直接以管理员身份放行，
    // 不依赖 Redis 会话（Redis 故障时仍可管理）
    if let Some(admin_token) = &state.config.admin_api_token {
        if let Some(presented) = token_from_header(&req) {
            if tokens_equal(admin_token, &presented) {
                let user = sqlx::query_as::<_, User>(
                    "SELECT id, username, email, password_hash FROM users ORDER BY id ASC LIMIT 1",
                )
                .fetch_optional(&state.db)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "user lookup failed");
                    unauthorized()
                })?
                .ok_or_else(unauthorized)?;
                tracing::info!(username = %user.username, "admin api token auth");
                req.extensions_mut().insert(AuthUser {
                    id: user.id,
                    username: user.username.clone(),
                    token: presented,
                });
                return Ok(next.run(req).await);
            }
        }
    }

    let token = token_from_cookie(&req).ok_or_else(unauthorized)?;
    let Some(mut conn) = state.redis.as_ref().cloned() else {
        return Err(unauthorized());
    };
    let user_id: Option<u64> = conn.get(keys::session(&token)).await.map_err(|e| {
        tracing::warn!(error = %e, "session lookup failed");
        unauthorized()
    })?;
    let Some(user_id) = user_id else {
        return Err(unauthorized());
    };

    // 滑动续期
    let ttl = (state.config.session_ttl_days * 86400) as i64;
    let _: redis::RedisResult<()> = conn.expire(keys::session(&token), ttl).await;

    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "user lookup failed");
        unauthorized()
    })?
    .ok_or_else(unauthorized)?;

    req.extensions_mut().insert(AuthUser {
        id: user.id,
        username: user.username.clone(),
        token,
    });
    Ok(next.run(req).await)
}

/// 注入到 extensions 的当前登录用户
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: u64,
    pub username: String,
    pub token: String,
}

impl<S> axum::extract::FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let parts = parts.extensions.get::<AuthUser>().cloned();
        async move { parts.ok_or_else(|| AppError::Unauthorized("未登录或会话已过期".into())) }
    }
}
