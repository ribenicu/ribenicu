//! 公开文章接口（带缓存）

use crate::error::{AppError, AppResult};
use crate::models::post::{ArchiveMonth, HomeData, PostDetail};
use crate::models::{self, ok};
use crate::services::post as svc;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
}

pub async fn list_posts(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> AppResult<models::ApiResponse<HomeData>> {
    // 首页数据带统计，默认/固定以服务端 HOME_LIMIT 为准；
    // limit 参数预留，超出范围时收窄，避免缓存键分裂。
    let _limit = q.limit.unwrap_or(svc::HOME_LIMIT).clamp(1, 20);
    Ok(ok(svc::list_home(&state).await?))
}

pub async fn archive(
    State(state): State<AppState>,
) -> AppResult<models::ApiResponse<Vec<ArchiveMonth>>> {
    Ok(ok(svc::archive(&state).await?))
}

pub async fn detail(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<models::ApiResponse<PostDetail>> {
    match svc::detail_by_slug(&state, &slug).await? {
        Some(d) => Ok(ok(d)),
        None => Err(AppError::NotFound("文章不存在或未发布".into())),
    }
}
