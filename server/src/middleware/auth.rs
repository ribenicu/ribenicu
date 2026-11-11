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

pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let unauthorized = || AppError::Unauthorized("未登录或会话已过期".into());

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

    let user =
        sqlx::query_as::<_, User>("SELECT id, username, password_hash FROM users WHERE id = ?")
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
