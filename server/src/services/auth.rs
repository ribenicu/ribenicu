//! 邮箱验证码登录：发送验证码、校验并创建 Session、解析/销毁 Session。
//!
//! REST（`/api/auth/*`）与 MCP（`/mcp` 的 auth_* 工具）共用这一层，
//! 避免两套登录逻辑出现行为差异。

use crate::error::{AppError, AppResult};
use crate::middleware::rate_limit::check_limit;
use crate::models::user::User;
use crate::services::cache::keys;
use crate::services::email::send_verification_code;
use crate::state::AppState;
use rand::Rng;
use redis::AsyncCommands;

fn generate_code() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..1_000_000))
}

pub fn is_valid_email(email: &str) -> bool {
    use validator::ValidateEmail;
    email.validate_email()
}

/// 发送登录验证码。
///
/// 仅管理员邮箱会真正发信；未注册邮箱返回成功但不发信，避免邮箱枚举。
pub async fn send_login_code(state: &AppState, email: &str) -> AppResult<()> {
    let email = email.trim().to_lowercase();
    if email.is_empty() || !is_valid_email(&email) {
        return Err(AppError::Validation("请输入正确的邮箱地址".into()));
    }

    // 邮箱维度限流：60 秒 1 次、1 小时 5 次
    check_limit(
        state,
        &keys::rate_email_send(&format!("email:{email}")),
        60,
        1,
    )
    .await?;
    check_limit(
        state,
        &keys::rate_email_send(&format!("email_hour:{email}")),
        3600,
        5,
    )
    .await?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
        .bind(&email)
        .fetch_optional(&state.db)
        .await?;
    if user.is_none() {
        tracing::warn!(email = %email, "send_code requested for unknown email");
        return Ok(());
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
    Ok(())
}

/// 校验邮箱验证码，成功后在 Redis 建立 Session。
///
/// 返回 `(用户, session token, ttl 秒数)`；Cookie / MCP 会话由调用方按需处理。
pub async fn login_with_code(
    state: &AppState,
    email: &str,
    code: &str,
) -> AppResult<(User, String, u64)> {
    let email = email.trim().to_lowercase();
    let code = code.trim();
    if email.is_empty() || code.is_empty() {
        return Err(AppError::Validation("请输入邮箱和验证码".into()));
    }

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
        .bind(&email)
        .fetch_optional(&state.db)
        .await?;
    let Some(user) = user else {
        return Err(AppError::Unauthorized("邮箱或验证码错误".into()));
    };

    // Redis 不可用则拒绝登录（session 无法持久化）
    let Some(mut conn) = state.redis.clone() else {
        return Err(AppError::Internal("会话服务不可用，请稍后再试".into()));
    };

    let stored_code: Option<String> = conn
        .get(keys::email_code(&email))
        .await
        .map_err(|e| AppError::Internal(format!("验证码读取失败: {e}")))?;
    let Some(stored_code) = stored_code else {
        return Err(AppError::Unauthorized("验证码已过期，请重新发送".into()));
    };

    if stored_code != code {
        // 记录错误尝试次数，5 次后需重新发送
        let attempts: i64 = conn
            .incr(keys::email_code_attempts(&email), 1)
            .await
            .unwrap_or(0);
        if attempts == 1 {
            let _: redis::RedisResult<()> = conn
                .expire(
                    keys::email_code_attempts(&email),
                    keys::TTL_EMAIL_CODE as i64,
                )
                .await;
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

    tracing::info!(username = %user.username, "login ok via email code");
    Ok((user, token, ttl_secs))
}

/// 解析 Session token 为当前用户；不存在/过期返回 None，并滑动续期。
pub async fn resolve_session(state: &AppState, token: &str) -> AppResult<Option<User>> {
    let Some(mut conn) = state.redis.as_ref().cloned() else {
        return Ok(None);
    };
    let user_id: Option<u64> = conn
        .get(keys::session(token))
        .await
        .map_err(|e| AppError::Internal(format!("会话服务不可用: {e}")))?;
    let Some(user_id) = user_id else {
        return Ok(None);
    };

    let ttl = (state.config.session_ttl_days * 86400) as i64;
    let _: redis::RedisResult<()> = conn.expire(keys::session(token), ttl).await;

    Ok(
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?,
    )
}

/// 删除 Session（登出）。Redis 不可用时静默忽略。
pub async fn destroy_session(state: &AppState, token: &str) {
    if let Some(mut conn) = state.redis.as_ref().cloned() {
        let _: redis::RedisResult<()> = conn.del(keys::session(token)).await;
    }
}

/// 库中第一位用户即管理员（与 ADMIN_API_TOKEN 旁路的身份约定一致）。
pub async fn first_admin(state: &AppState) -> AppResult<Option<User>> {
    Ok(
        sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY id ASC LIMIT 1")
            .fetch_optional(&state.db)
            .await?,
    )
}

pub async fn user_by_id(state: &AppState, id: u64) -> AppResult<Option<User>> {
    Ok(
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.db)
            .await?,
    )
}
