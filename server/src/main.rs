//! 一本日记 · 后端入口
//! 启动流程：配置 → 日志 → MySQL 连接池 + migration + 种子 → Redis（可降级）→ 路由

mod config;
mod db;
mod error;
mod middleware;
mod models;
mod routes;
mod services;
mod state;
mod util;

use axum::Router;
use state::AppState;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    let cfg = config::Config::from_env();

    init_tracing(&cfg.log_directive);

    if cfg.admin_api_token.is_some() {
        tracing::warn!("ADMIN_API_TOKEN 已配置：携带该 token 的请求可绕过登录鉴权，请确保 token 足够随机且仅通过密文渠道分发");
    }

    let pool = match db::connect_pool(&cfg).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "mysql pool init failed");
            std::process::exit(1);
        }
    };

    if let Err(e) = db::run_migrations(&pool).await {
        tracing::error!(error = %e, "migration failed, continuing anyway (may retry on boot)");
    }
    if let Err(e) = db::seed_admin(&pool, &cfg).await {
        tracing::error!(error = %e, "admin seed failed");
    }
    if let Err(e) = db::backfill_admin_email(&pool, &cfg).await {
        tracing::error!(error = %e, "admin email backfill failed");
    }
    if let Err(e) = db::seed_demo_posts(&pool, &cfg).await {
        tracing::error!(error = %e, "demo seed failed");
    }

    // Redis 连接失败不阻断启动：缓存/限流/session 降级（见 §5）
    let redis = match redis::Client::open(cfg.redis_url.clone()) {
        Ok(client) => match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            client.get_multiplexed_tokio_connection(),
        )
        .await
        {
            Ok(Ok(conn)) => {
                tracing::info!("redis connected");
                Some(conn)
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "redis unavailable, cache/rate-limit/session degraded");
                None
            }
            Err(_) => {
                tracing::warn!("redis connect timeout, cache/rate-limit/session degraded");
                None
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "redis url invalid, cache/rate-limit/session degraded");
            None
        }
    };

    let state = AppState {
        db: pool,
        redis,
        config: cfg.clone(),
    };

    let api = routes::api_router(state.clone());

    let mut app = Router::new()
        .nest("/api", api)
        .layer(TraceLayer::new_for_http());

    // 可选：由 Rust 托管前端静态资源（FRONTEND_DIR 未设置时由 Caddy/Nginx 托管）
    if let Some(dir) = &cfg.frontend_dir {
        let path = std::path::Path::new(dir);
        if path.is_dir() {
            tracing::info!(dir, "serving static files");
            app = app.fallback_service(ServeDir::new(dir));
        } else {
            tracing::warn!(
                dir,
                "FRONTEND_DIR is not a directory, static serving disabled"
            );
        }
    }

    let addr = format!("{}:{}", cfg.host, cfg.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "bind failed");
            std::process::exit(1);
        });
    tracing::info!(addr, "riben-server listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .unwrap();
}

fn init_tracing(directive: &str) {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_new(directive).unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
}
