//! 邮件发送服务：通过 SMTP 发送验证码邮件（lettre + tokio + rustls）。

use crate::config::Config;
use crate::error::{AppError, AppResult};
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

fn smtp_transport(cfg: &Config) -> AppResult<AsyncSmtpTransport<Tokio1Executor>> {
    let host = cfg
        .smtp_host
        .as_deref()
        .ok_or_else(|| AppError::Internal("SMTP_HOST 未配置".into()))?;
    let creds = match (&cfg.smtp_user, &cfg.smtp_pass) {
        (Some(user), Some(pass)) => Some(Credentials::new(user.clone(), pass.clone())),
        _ => None,
    };
    let tls_params = lettre::transport::smtp::client::TlsParameters::new(host.to_string())
        .map_err(|e| AppError::Internal(format!("smtp tls params: {e}")))?;
    let mut builder = if cfg.smtp_secure {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(cfg.smtp_port)
            .tls(lettre::transport::smtp::client::Tls::Wrapper(tls_params))
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(cfg.smtp_port)
            .tls(lettre::transport::smtp::client::Tls::Required(tls_params))
    };
    if let Some(creds) = creds {
        builder = builder.credentials(creds);
    }
    Ok(builder.build())
}

/// 发送 6 位数字登录验证码邮件
pub async fn send_verification_code(cfg: &Config, email: &str, code: &str) -> AppResult<()> {
    let from = cfg
        .smtp_from
        .as_deref()
        .ok_or_else(|| AppError::Internal("SMTP_FROM 未配置".into()))?;

    let body = format!(
        "您的一本日记后台登录验证码是：{code}\n\n验证码 5 分钟内有效，请勿泄露给他人。\n\n—— riben.icu"
    );

    let message = Message::builder()
        .from(
            from.parse()
                .map_err(|_| AppError::Internal("SMTP_FROM 格式错误".into()))?,
        )
        .to(email
            .parse()
            .map_err(|_| AppError::Validation("邮箱格式错误".into()))?)
        .subject("一本日记 · 登录验证码")
        .header(ContentType::TEXT_PLAIN)
        .body(body)
        .map_err(|e| AppError::Internal(format!("构建邮件失败: {e}")))?;

    let transport = smtp_transport(cfg)?;
    transport
        .send(message)
        .await
        .map_err(|e| AppError::Internal(format!("邮件发送失败: {e}")))?;

    tracing::info!(email = %email, "verification code email sent");
    Ok(())
}
