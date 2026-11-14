//! 系统状态：健康检查与站点统计（REST `/api/health` 与 MCP 工具共用）。

use crate::error::AppResult;
use crate::state::AppState;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Health {
    pub mysql: bool,
    pub redis: bool,
    pub version: &'static str,
}

pub async fn health(state: &AppState) -> Health {
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

    Health {
        mysql,
        redis,
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[derive(Debug, Serialize)]
pub struct SiteStats {
    pub total_posts: i64,
    pub published_posts: i64,
    pub draft_posts: i64,
    pub total_comments: i64,
    pub pending_comments: i64,
    pub approved_comments: i64,
    pub rejected_comments: i64,
    pub total_tags: i64,
    pub recorded_days: i64,
    pub latest_published_at: Option<DateTime<Utc>>,
}

pub async fn stats(state: &AppState) -> AppResult<SiteStats> {
    let (total_posts, published_posts, draft_posts): (i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(IF(status = 1, 1, NULL)), COUNT(IF(status = 0, 1, NULL)) FROM posts",
    )
    .fetch_one(&state.db)
    .await?;

    let (total_comments, pending_comments, approved_comments, rejected_comments): (
        i64,
        i64,
        i64,
        i64,
    ) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(IF(status = 0, 1, NULL)), COUNT(IF(status = 1, 1, NULL)), \
         COUNT(IF(status = 2, 1, NULL)) FROM comments",
    )
    .fetch_one(&state.db)
    .await?;

    let total_tags: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags")
        .fetch_one(&state.db)
        .await?;

    let recorded_days: i64 =
        sqlx::query_scalar("SELECT COUNT(DISTINCT DATE(published_at)) FROM posts WHERE status = 1")
            .fetch_one(&state.db)
            .await?;

    let latest: Option<NaiveDateTime> =
        sqlx::query_scalar("SELECT MAX(published_at) FROM posts WHERE status = 1")
            .fetch_one(&state.db)
            .await?;

    Ok(SiteStats {
        total_posts,
        published_posts,
        draft_posts,
        total_comments,
        pending_comments,
        approved_comments,
        rejected_comments,
        total_tags,
        recorded_days,
        latest_published_at: latest.map(|d| DateTime::<Utc>::from_naive_utc_and_offset(d, Utc)),
    })
}
