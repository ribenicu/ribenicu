//! 路由聚合

pub mod admin;
pub mod auth;
pub mod comment;
pub mod post;

use crate::middleware::auth::require_auth;
use crate::models;
use crate::services::system;
use crate::state::AppState;
use axum::routing::{get, post};
use axum::{middleware, Router};

pub fn api_router(state: AppState) -> Router {
    let auth_layer = middleware::from_fn_with_state(state.clone(), require_auth);
    let comment_limit =
        middleware::from_fn_with_state(state.clone(), crate::middleware::rate_limit::comment_limit);
    let login_limit =
        middleware::from_fn_with_state(state.clone(), crate::middleware::rate_limit::login_limit);
    let email_send_limit = middleware::from_fn_with_state(
        state.clone(),
        crate::middleware::rate_limit::email_send_limit,
    );

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
        .route(
            "/auth/email-code",
            post(auth::send_code).route_layer(email_send_limit),
        )
        .route(
            "/auth/email-login",
            post(auth::email_login).route_layer(login_limit),
        )
        .route(
            "/auth/logout",
            post(auth::logout).route_layer(auth_layer.clone()),
        )
        .route("/auth/me", get(auth::me).route_layer(auth_layer.clone()))
        .nest("/admin", admin::router(state.clone()))
        .with_state(state)
}

async fn health(state: axum::extract::State<AppState>) -> models::ApiResponse<system::Health> {
    models::ok(system::health(&state).await)
}
