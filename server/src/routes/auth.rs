//! 登录 / 登出 / 当前用户
//!
//! Session：登录成功后生成随机 token，写入 Redis `session:{token} -> user_id`，
//! Cookie `riben_session`（HttpOnly; SameSite=Lax; HTTPS 下 Secure），见计划 §2/§6。

use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthUser;
use crate::middleware::rate_limit::reset_login;
use crate::models::user::{User, UserOut};
use crate::models::{self, ok, Empty};
use crate::services::cache::keys;
use crate::state::AppState;
use axum::extract::{ConnectInfo, Request, State};
use axum::response::{IntoResponse, Response};
use redis::AsyncCommands;
use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Debug, Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

fn client_ip(req: &Request) -> String {
    req.headers()
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .map(str::to_string)
        .or_else(|| {
            req.headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(',').next())
                .map(str::trim)
                .map(str::to_string)
        })
        .or_else(|| {
            req.extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|c| c.0.ip().to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}

fn session_cookie(token: &str, ttl_secs: u64, secure: bool) -> String {
    let mut cookie = format!(
        "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={ttl_secs}",
        crate::config::Config::cookie_name()
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

fn clear_cookie() -> String {
    format!(
        "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        crate::config::Config::cookie_name()
    )
}

pub async fn login(State(state): State<AppState>, req: Request) -> AppResult<Response> {
    let ip = client_ip(&req);

    let body = axum::body::to_bytes(req.into_body(), 4096)
        .await
        .map_err(|_| AppError::BadRequest("请求体解析失败".into()))?;
    let creds: LoginReq =
        serde_json::from_slice(&body).map_err(|_| AppError::BadRequest("请求体格式错误".into()))?;
    if creds.username.trim().is_empty() || creds.password.is_empty() {
        return Err(AppError::Validation("请输入用户名和密码".into()));
    }

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(creds.username.trim())
        .fetch_optional(&state.db)
        .await?;
    let user = match user {
        Some(u) => u,
        None => return Err(AppError::Unauthorized("用户名或密码错误".into())),
    };

    let parsed = argon2::PasswordHash::new(&user.password_hash)
        .map_err(|e| AppError::Internal(format!("password hash invalid: {e}")))?;
    use argon2::password_hash::PasswordVerifier;
    if argon2::Argon2::default()
        .verify_password(creds.password.as_bytes(), &parsed)
        .is_err()
    {
        return Err(AppError::Unauthorized("用户名或密码错误".into()));
    }

    // Redis 不可用则拒绝登录（session 无法持久化）
    let Some(mut conn) = state.redis.clone() else {
        return Err(AppError::Internal("会话服务不可用，请稍后再试".into()));
    };
    let token = uuid::Uuid::new_v4().to_string();
    let ttl_secs = state.config.session_ttl_days * 86400;
    conn.set_ex::<_, _, ()>(&keys::session(&token), user.id, ttl_secs)
        .await
        .map_err(|e| AppError::Internal(format!("session write failed: {e}")))?;

    reset_login(&state, &ip).await;
    tracing::info!(username = %user.username, "login ok");

    let cookie = session_cookie(&token, ttl_secs, state.config.cookie_secure);
    Ok((
        [(
            axum::http::header::SET_COOKIE,
            axum::http::HeaderValue::from_str(&cookie)
                .map_err(|_| AppError::Internal("cookie build failed".into()))?,
        )],
        axum::Json(ok(UserOut::from(user))),
    )
        .into_response())
}

pub async fn logout(State(state): State<AppState>, auth: AuthUser) -> AppResult<Response> {
    if let Some(mut conn) = state.redis.clone() {
        let _: redis::RedisResult<()> = conn.del(keys::session(&auth.token)).await;
    }
    tracing::info!(username = %auth.username, "logout");
    let cookie = clear_cookie();
    Ok((
        [(
            axum::http::header::SET_COOKIE,
            axum::http::HeaderValue::from_str(&cookie)
                .map_err(|_| AppError::Internal("cookie build failed".into()))?,
        )],
        axum::Json(ok(Empty {})),
    )
        .into_response())
}

pub async fn me(auth: AuthUser) -> models::ApiResponse<UserOut> {
    ok(UserOut {
        id: auth.id,
        username: auth.username.clone(),
    })
}
