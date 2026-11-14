//! 邮箱验证码登录 / 登出 / 当前用户
//!
//! 登录流程：POST /auth/email-code 发送 6 位验证码到管理员邮箱，
//! POST /auth/email-login 校验验证码后建立 Redis Session。
//! Session：登录成功后生成随机 token，写入 Redis `session:{token} -> user_id`，
//! Cookie `riben_session`（HttpOnly; SameSite=Lax; HTTPS 下 Secure），见计划 §2/§6。

use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthUser;
use crate::middleware::rate_limit::reset_login;
use crate::models::user::UserOut;
use crate::models::{self, ok, Empty};
use crate::services::auth as auth_svc;
use crate::state::AppState;
use axum::extract::{ConnectInfo, Request, State};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Debug, Deserialize)]
pub struct SendCodeReq {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct EmailLoginReq {
    pub email: String,
    pub code: String,
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

/// 发送登录验证码到管理员邮箱
pub async fn send_code(State(state): State<AppState>, req: Request) -> AppResult<Response> {
    let body = axum::body::to_bytes(req.into_body(), 4096)
        .await
        .map_err(|_| AppError::BadRequest("请求体解析失败".into()))?;
    let payload: SendCodeReq =
        serde_json::from_slice(&body).map_err(|_| AppError::BadRequest("请求体格式错误".into()))?;

    auth_svc::send_login_code(&state, &payload.email).await?;
    Ok(axum::Json(ok(Empty {})).into_response())
}

/// 使用邮箱 + 验证码登录
pub async fn email_login(State(state): State<AppState>, req: Request) -> AppResult<Response> {
    let ip = client_ip(&req);

    let body = axum::body::to_bytes(req.into_body(), 4096)
        .await
        .map_err(|_| AppError::BadRequest("请求体解析失败".into()))?;
    let payload: EmailLoginReq =
        serde_json::from_slice(&body).map_err(|_| AppError::BadRequest("请求体格式错误".into()))?;

    let (user, token, ttl_secs) =
        auth_svc::login_with_code(&state, &payload.email, &payload.code).await?;

    reset_login(&state, &ip).await;

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
    auth_svc::destroy_session(&state, &auth.token).await;
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
