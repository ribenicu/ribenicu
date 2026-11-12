//! Redis 计数器限流：评论 60s 1 次/IP，登录 10min 5 次/IP。
//! Redis 不可用时 fail-open（告警日志），不阻断服务。

use crate::error::AppError;
use crate::services::cache::keys;
use crate::state::AppState;
use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use redis::AsyncCommands;
use std::net::SocketAddr;

fn client_ip(req: &Request, conn: Option<&ConnectInfo<SocketAddr>>) -> String {
    if let Some(v) = req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()) {
        return v.trim().to_string();
    }
    if let Some(v) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        return v.split(',').next().unwrap_or("").trim().to_string();
    }
    conn.map(|c| c.0.ip().to_string())
        .unwrap_or_else(|| "unknown".into())
}

async fn hit(state: &AppState, key: &str, window_secs: u64, max: i64) -> Result<(), AppError> {
    let Some(mut conn) = state.redis.as_ref().cloned() else {
        tracing::warn!(key, "redis unavailable, rate limit skipped");
        return Ok(());
    };
    let count: redis::RedisResult<i64> = conn.incr(key, 1).await;
    match count {
        Ok(n) => {
            if n == 1 {
                let _: redis::RedisResult<()> = conn.expire(key, window_secs as i64).await;
            }
            if n > max {
                return Err(AppError::RateLimited("操作太频繁，请稍后再试".into()));
            }
            Ok(())
        }
        Err(e) => {
            tracing::warn!(key, error = %e, "rate limit check failed, allowed");
            Ok(())
        }
    }
}

/// 评论限流：60 秒 1 次 / IP
pub async fn comment_limit(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let ip = client_ip(&req, req.extensions().get::<ConnectInfo<SocketAddr>>());
    hit(&state, &keys::rate_comment(&ip), 60, 1).await?;
    Ok(next.run(req).await)
}

/// 登录限流：10 分钟最多 5 次 / IP（成功登录后由路由层清零）
pub async fn login_limit(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let ip = client_ip(&req, req.extensions().get::<ConnectInfo<SocketAddr>>());
    hit(&state, &keys::rate_login(&ip), 600, 5).await?;
    Ok(next.run(req).await)
}

/// 发送验证码限流：10 分钟最多 5 次 / IP
pub async fn email_send_limit(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let ip = client_ip(&req, req.extensions().get::<ConnectInfo<SocketAddr>>());
    hit(&state, &keys::rate_email_send(&format!("ip:{ip}")), 600, 5).await?;
    Ok(next.run(req).await)
}

/// 登录成功后清零失败计数
pub async fn reset_login(state: &AppState, ip: &str) {
    if let Some(mut conn) = state.redis.as_ref().cloned() {
        let _: redis::RedisResult<()> = conn.del(keys::rate_login(ip)).await;
    }
}

/// 辅助：按 key 限流检查（供路由层做邮箱维度限流时复用）
pub async fn check_limit(state: &AppState, key: &str, window_secs: u64, max: i64) -> Result<(), AppError> {
    hit(state, key, window_secs, max).await
}
