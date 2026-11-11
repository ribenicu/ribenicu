//! 路由聚合

pub mod admin;
pub mod auth;
pub mod comment;
pub mod post;

use crate::middleware::auth::require_auth;
use crate::models::{self, ok};
use crate::state::AppState;
use axum::routing::{get, post};
use axum::{middleware, Router};
use serde::Serialize;

pub fn api_router(state: AppState) -> Router {
    let auth_layer = middleware::from_fn_with_state(state.clone(), require_auth);
    let comment_limit =
        middleware::from_fn_with_state(state.clone(), crate::middleware::rate_limit::comment_limit);
    let login_limit =
        middleware::from_fn_with_state(state.clone(), crate::middleware::rate_limit::login_limit);

    Router::new()
        .route("/health", get(health))
        .route("/posts", get(post::list_posts))
        .route("/posts/archive", get(post::archive))
        .route("/posts/{slug}", get(post::detail))
        .route("/posts/{slug}/comments", get(comment::list))
        .route(
            "/comments",
            post(comment::create).route_layer(comment_limit),
        )
        .route("/auth/login", post(auth::login).route_layer(login_limit))
        .route(
            "/auth/logout",
            post(auth::logout).route_layer(auth_layer.clone()),
        )
        .route("/auth/me", get(auth::me).route_layer(auth_layer.clone()))
        .nest("/admin", admin::router(state.clone()))
        .with_state(state)
}

#[derive(Serialize)]
pub struct Health {
    pub mysql: bool,
    pub redis: bool,
    pub version: &'static str,
}

async fn health(state: axum::extract::State<AppState>) -> models::ApiResponse<Health> {
    let mysql = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        sqlx::query("SELECT 1").execute(&state.db),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false);

    let redis = if let Some(mut conn) = state.redis.clone() {
        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .is_ok()
    } else {
        false
    };

    ok(Health {
        mysql,
        redis,
        version: env!("CARGO_PKG_VERSION"),
    })
}
