//! 配置：环境变量优先（.env 文件通过 dotenvy 加载）。

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub redis_url: String,
    /// session:{token} 的过期时间（天），滑动续期
    pub session_ttl_days: u64,
    /// HTTPS 部署时置 true，Cookie 加 Secure
    pub cookie_secure: bool,
    /// 静态资源目录；None 表示不托管静态资源（由 Caddy/Nginx 托管前端）
    pub frontend_dir: Option<String>,
    /// 首次启动且 users 表为空时播种管理员
    pub admin_username: Option<String>,
    /// 管理员邮箱（邮箱验证码登录的目标邮箱）
    pub admin_email: Option<String>,
    /// SMTP 邮件发送配置
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    /// true 使用 SMTPS（SSL），false 使用 STARTTLS
    pub smtp_secure: bool,
    pub smtp_user: Option<String>,
    pub smtp_pass: Option<String>,
    /// 发件人地址，默认与 smtp_user 相同
    pub smtp_from: Option<String>,
    /// 首次启动且 posts 表为空时写入演示文章（原静态站内容）
    pub seed_demo: bool,
    pub log_directive: String,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let frontend_dir = std::env::var("FRONTEND_DIR")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let smtp_user = non_empty("SMTP_USER");

        Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(9015),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "mysql://riben:riben@127.0.0.1:3306/riben".into()),
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".into()),
            session_ttl_days: std::env::var("SESSION_TTL_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(7),
            cookie_secure: std::env::var("COOKIE_SECURE")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            frontend_dir,
            admin_username: non_empty("ADMIN_USERNAME"),
            admin_email: non_empty("ADMIN_EMAIL"),
            smtp_host: non_empty("SMTP_HOST"),
            smtp_port: std::env::var("SMTP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(465),
            smtp_secure: std::env::var("SMTP_SECURE")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            smtp_user: smtp_user.clone(),
            smtp_pass: non_empty("SMTP_PASS"),
            smtp_from: non_empty("SMTP_FROM").or(smtp_user),
            seed_demo: std::env::var("SEED_DEMO")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            log_directive: std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "riben_server=info,tower_http=info,sqlx=warn".into()),
        }
    }

    pub fn cookie_name() -> &'static str {
        "riben_session"
    }
}

fn non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
