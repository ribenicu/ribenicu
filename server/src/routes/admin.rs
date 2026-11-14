//! 管理接口：文章 CRUD、评论审核、标签管理、Markdown 预览
//! 整组挂载 RequireAuth 中间件，未登录一律 401。

use crate::error::{AppError, AppResult};
use crate::middleware::auth::{require_auth, AuthUser};
use crate::models::comment::CommentAdmin;
use crate::models::post::PostAdmin;
use crate::models::tag::{CreateTagReq, TagOut, UpdateTagReq};
use crate::models::{self, ok, Empty};
use crate::services::comment as comment_svc;
use crate::services::post as post_svc;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::middleware;
use axum::routing::{get, post, put};
use axum::Router;
use serde::{Deserialize, Serialize};
use validator::Validate;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/posts", get(admin_list_posts).post(create_post))
        .route("/posts/{id}", put(update_post).delete(delete_post))
        .route("/comments", get(admin_list_comments))
        .route("/comments/{id}/approve", put(approve_comment))
        .route("/comments/{id}/reject", put(reject_comment))
        .route("/tags", get(list_tags).post(create_tag))
        .route("/tags/{id}", put(update_tag).delete(delete_tag))
        .route("/preview", post(preview_markdown))
        .route_layer(middleware::from_fn_with_state(state, require_auth))
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

impl PageQuery {
    fn normalize(&self) -> (u32, u32) {
        let page = self.page.unwrap_or(1).max(1);
        let per_page = self.per_page.unwrap_or(20).clamp(1, 100);
        (page, per_page)
    }
}

#[derive(Debug, Serialize)]
struct Paged<T: Serialize> {
    list: Vec<T>,
    total: i64,
    page: u32,
    per_page: u32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SavePostReq {
    #[validate(length(min = 1, max = 190))]
    pub title: String,
    #[validate(length(min = 1, max = 100_000))]
    pub content_md: String,
    #[validate(length(max = 32))]
    pub sticker: Option<String>,
    #[validate(range(min = 0, max = 1))]
    pub status: i8,
    pub tag_ids: Vec<u64>,
}

impl SavePostReq {
    fn into_input(self) -> post_svc::PostInput {
        post_svc::PostInput {
            title: self.title,
            content_md: self.content_md,
            sticker: self.sticker,
            status: self.status,
            tag_ids: self.tag_ids,
        }
    }
}

async fn admin_list_posts(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> AppResult<models::ApiResponse<Paged<PostAdmin>>> {
    let (page, per_page) = q.normalize();
    let (list, total) = post_svc::admin_list(&state, page, per_page).await?;
    Ok(ok(Paged {
        list,
        total,
        page,
        per_page,
    }))
}

async fn create_post(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<SavePostReq>,
) -> AppResult<models::ApiResponse<PostAdmin>> {
    req.validate()
        .map_err(|e| AppError::Validation(format!("{e}")))?;
    let post = post_svc::create(&state, req.into_input()).await?;
    tracing::info!(id = post.id, title = %post.title, "post created");
    Ok(ok(post))
}

async fn update_post(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    axum::Json(req): axum::Json<SavePostReq>,
) -> AppResult<models::ApiResponse<PostAdmin>> {
    req.validate()
        .map_err(|e| AppError::Validation(format!("{e}")))?;
    let post = post_svc::update(&state, id, req.into_input()).await?;
    tracing::info!(id = post.id, title = %post.title, "post updated");
    Ok(ok(post))
}

async fn delete_post(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> AppResult<models::ApiResponse<Empty>> {
    post_svc::delete(&state, id).await?;
    tracing::info!(id, "post deleted");
    Ok(ok(Empty {}))
}

#[derive(Debug, Deserialize)]
pub struct CommentQuery {
    pub status: Option<i8>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

async fn admin_list_comments(
    State(state): State<AppState>,
    Query(q): Query<CommentQuery>,
) -> AppResult<models::ApiResponse<serde_json::Value>> {
    let (page, per_page) = PageQuery {
        page: q.page,
        per_page: q.per_page,
    }
    .normalize();
    let status = q.status.filter(|s| (0..=2).contains(s));
    let (list, total) = comment_svc::admin_list(&state, status, page, per_page).await?;
    let pending = comment_svc::pending_count(&state).await?;
    #[derive(Serialize)]
    struct CommentsOut {
        list: Vec<CommentAdmin>,
        total: i64,
        pending: i64,
        page: u32,
        per_page: u32,
    }
    Ok(ok(serde_json::to_value(CommentsOut {
        list,
        total,
        pending,
        page,
        per_page,
    })
    .unwrap()))
}

async fn approve_comment(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> AppResult<models::ApiResponse<Empty>> {
    comment_svc::moderate(&state, id, 1).await?;
    tracing::info!(id, "comment approved");
    Ok(ok(Empty {}))
}

async fn reject_comment(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> AppResult<models::ApiResponse<Empty>> {
    comment_svc::moderate(&state, id, 2).await?;
    tracing::info!(id, "comment rejected");
    Ok(ok(Empty {}))
}

async fn list_tags(State(state): State<AppState>) -> AppResult<models::ApiResponse<Vec<TagOut>>> {
    Ok(ok(post_svc::list_all_tags(&state).await?))
}

async fn create_tag(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<CreateTagReq>,
) -> AppResult<models::ApiResponse<TagOut>> {
    req.validate()
        .map_err(|e| AppError::Validation(format!("{e}")))?;
    Ok(ok(
        post_svc::create_tag(&state, &req.name, &req.color).await?
    ))
}

async fn update_tag(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    axum::Json(req): axum::Json<UpdateTagReq>,
) -> AppResult<models::ApiResponse<TagOut>> {
    req.validate()
        .map_err(|e| AppError::Validation(format!("{e}")))?;
    if req.name.is_none() && req.color.is_none() {
        return Err(AppError::Validation("至少提供 name 或 color".into()));
    }
    Ok(ok(post_svc::update_tag(
        &state,
        id,
        req.name.as_deref(),
        req.color.as_deref(),
    )
    .await?))
}

async fn delete_tag(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> AppResult<models::ApiResponse<Empty>> {
    post_svc::delete_tag(&state, id).await?;
    Ok(ok(Empty {}))
}

#[derive(Debug, Deserialize)]
pub struct PreviewReq {
    pub md: String,
}

async fn preview_markdown(
    _auth: AuthUser,
    axum::Json(req): axum::Json<PreviewReq>,
) -> models::ApiResponse<serde_json::Value> {
    ok(serde_json::json!({ "html": crate::util::render_markdown(&req.md) }))
}
