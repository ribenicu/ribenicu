//! 邮箱验证码登录 / 登出 / 当前用户
//!
//! 登录流程：POST /auth/email-code 发送 6 位验证码到管理员邮箱，
//! POST /auth/email-login 校验验证码后建立 Redis Session。
//! Session：登录成功后生成随机 token，写入 Redis `session:{token} -> user_id`，
//! Cookie `riben_session`（HttpOnly; SameSite=Lax; HTTPS 下 Secure），见计划 §2/§6。

use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthUser;
use crate::middleware::rate_limit::{check_limit, reset_login};
use crate::models::user::{User, UserOut};
use crate::models::{self, ok, Empty};
use crate::services::cache::keys;
use crate::services::email::send_verification_code;
use crate::state::AppState;
use axum::extract::{ConnectInfo, Request, State};
use axum::response::{IntoResponse, Response};
use rand::Rng;
use redis::AsyncCommands;
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

fn generate_code() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..1_000_000))
}

fn is_valid_email(email: &str) -> bool {
    use validator::ValidateEmail;
    email.validate_email()
}

/// 发送登录验证码到管理员邮箱
pub async fn send_code(State(state): State<AppState>, req: Request) -> AppResult<Response> {
    let body = axum::body::to_bytes(req.into_body(), 4096)
        .await
        .map_err(|_| AppError::BadRequest("请求体解析失败".into()))?;
    let payload: SendCodeReq =
        serde_json::from_slice(&body).map_err(|_| AppError::BadRequest("请求体格式错误".into()))?;
    let email = payload.email.trim().to_lowercase();
    if email.is_empty() || !is_valid_email(&email) {
        return Err(AppError::Validation("请输入正确的邮箱地址".into()));
    }

    // 邮箱维度限流：60 秒 1 次、1 小时 5 次
    check_limit(
        &state,
        &keys::rate_email_send(&format!("email:{email}")),
        60,
        1,
    )
    .await?;
    check_limit(
        &state,
        &keys::rate_email_send(&format!("email_hour:{email}")),
        3600,
        5,
    )
    .await?;

    // 仅管理员邮箱可发送；不存在的邮箱也返回成功提示，避免枚举
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
        .bind(&email)
        .fetch_optional(&state.db)
        .await?;
    if user.is_none() {
        tracing::warn!(email = %email, "send_code requested for unknown email");
        return Ok(axum::Json(ok(Empty {})).into_response());
    }

    let code = generate_code();

    let Some(mut conn) = state.redis.clone() else {
        return Err(AppError::Internal("会话服务不可用，请稍后再试".into()));
    };
    conn.set_ex::<_, _, ()>(&keys::email_code(&email), &code, keys::TTL_EMAIL_CODE)
        .await
        .map_err(|e| AppError::Internal(format!("验证码存储失败: {e}")))?;
    // 清除旧的尝试计数
    let _: redis::RedisResult<()> = conn.del(keys::email_code_attempts(&email)).await;

    send_verification_code(&state.config, &email, &code).await?;

    tracing::info!(email = %email, "verification code sent");
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
    let email = payload.email.trim().to_lowercase();
    let code = payload.code.trim();
    if email.is_empty() || code.is_empty() {
        return Err(AppError::Validation("请输入邮箱和验证码".into()));
    }

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
        .bind(&email)
        .fetch_optional(&state.db)
        .await?;
    let user = match user {
        Some(u) => u,
        None => return Err(AppError::Unauthorized("邮箱或验证码错误".into())),
    };

    // Redis 不可用则拒绝登录（session 无法持久化）
    let Some(mut conn) = state.redis.clone() else {
        return Err(AppError::Internal("会话服务不可用，请稍后再试".into()));
    };

    let stored_code: Option<String> = conn.get(keys::email_code(&email)).await.map_err(|e| {
        AppError::Internal(format!("验证码读取失败: {e}"))
    })?;
    let Some(stored_code) = stored_code else {
        return Err(AppError::Unauthorized("验证码已过期，请重新发送".into()));
    };

    if stored_code != code {
        // 记录错误尝试次数，5 次后需重新发送
        let attempts: i64 = conn
            .incr(&keys::email_code_attempts(&email), 1)
            .await
            .unwrap_or(0);
        if attempts == 1 {
            let _: redis::RedisResult<()> = conn.expire(
                &keys::email_code_attempts(&email),
                keys::TTL_EMAIL_CODE as i64,
            ).await;
        }
        if attempts >= 5 {
            let _: redis::RedisResult<()> = conn.del(keys::email_code(&email)).await;
            return Err(AppError::Unauthorized(
                "验证码错误次数过多，请重新获取验证码".into(),
            ));
        }
        return Err(AppError::Unauthorized("验证码错误，请重试".into()));
    }

    // 验证成功：清除验证码和计数
    let _: redis::RedisResult<()> = conn.del(keys::email_code(&email)).await;
    let _: redis::RedisResult<()> = conn.del(keys::email_code_attempts(&email)).await;

    let token = uuid::Uuid::new_v4().to_string();
    let ttl_secs = state.config.session_ttl_days * 86400;
    conn.set_ex::<_, _, ()>(&keys::session(&token), user.id, ttl_secs)
        .await
        .map_err(|e| AppError::Internal(format!("session write failed: {e}")))?;

    reset_login(&state, &ip).await;
    tracing::info!(username = %user.username, "login ok via email code");

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
