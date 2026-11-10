//! 公开评论接口：查询（仅已通过）+ 发表（限流 + 校验，默认待审核）

use crate::error::{AppError, AppResult};
use crate::models::comment::{CommentNode, CreateCommentReq};
use crate::models::{self, ok, Empty};
use crate::services::comment as svc;
use crate::services::post as post_svc;
use crate::state::AppState;
use axum::extract::{Path, State};
use validator::Validate;

pub async fn list(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<models::ApiResponse<Vec<CommentNode>>> {
    let post = post_svc::published_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("文章不存在或未发布".into()))?;
    Ok(ok(svc::list_approved(&state, post.id).await?))
}

pub async fn create(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<CreateCommentReq>,
) -> AppResult<models::ApiResponse<Empty>> {
    req.validate()
        .map_err(|e| AppError::Validation(format!("{e}")))?;
    let comment = svc::create(&state, req).await?;
    tracing::info!(
        id = comment.id,
        post_id = comment.post_id,
        "comment submitted (pending)"
    );
    Ok(ok(Empty {}))
}
