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
    /// 可选：管理员 API Token。配置后请求头 `x-admin-token`（或
    /// `Authorization: Bearer <token>`）携带该值即可绕过登录鉴权，
    /// 以管理员身份调用全部接口；不配置则完全关闭该能力
    pub admin_api_token: Option<String>,
    /// 是否启用 MCP 服务（Streamable HTTP，挂载在 `/mcp`）
    pub mcp_enabled: bool,
    /// MCP 允许的 Host 头（防 DNS rebinding）。默认含本机回环地址，
    /// 生产环境需追加站点域名；`*` 表示关闭校验（不推荐）
    pub mcp_allowed_hosts: Vec<String>,
    /// MCP 允许的浏览器 Origin（留空表示不校验）
    pub mcp_allowed_origins: Vec<String>,
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

        let mcp_hosts = csv_env("MCP_ALLOWED_HOSTS");
        let mcp_allowed_hosts = if mcp_hosts.iter().any(|h| h == "*") {
            Vec::new() // 空列表 = 不校验 Host（仅限内网调试）
        } else {
            let mut hosts = vec!["localhost".into(), "127.0.0.1".into(), "::1".into()];
            for host in mcp_hosts {
                if !hosts.contains(&host) {
                    hosts.push(host);
                }
            }
            hosts
        };

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
            admin_api_token: non_empty("ADMIN_API_TOKEN"),
            mcp_enabled: bool_env("MCP_ENABLED", true),
            mcp_allowed_hosts,
            mcp_allowed_origins: csv_env("MCP_ALLOWED_ORIGINS"),
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

/// 逗号分隔的环境变量（自动去空白、去空项）。
fn csv_env(key: &str) -> Vec<String> {
    std::env::var(key)
        .ok()
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// 布尔环境变量：`false` / `0` 为假，其余为真。
fn bool_env(key: &str, default: bool) -> bool {
    std::env::var(key)
        .map(|v| v != "false" && v != "0")
        .unwrap_or(default)
}
