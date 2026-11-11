//! 文章：DB 行结构 + serde DTO

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct Post {
    pub id: u64,
    pub slug: String,
    pub title: String,
    pub content_md: String,
    pub excerpt: Option<String>,
    pub sticker: Option<String>,
    pub status: i8,
    pub published_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

fn rfc3339(ndt: NaiveDateTime) -> DateTime<Utc> {
    DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc)
}

/// 列表/归档用的文章摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostSummary {
    pub id: u64,
    pub slug: String,
    pub title: String,
    pub excerpt: String,
    pub sticker: Option<String>,
    pub published_at: DateTime<Utc>,
    pub tags: Vec<super::tag::TagOut>,
}

/// 文章详情：Markdown 已由后端渲染并消毒
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostDetail {
    pub id: u64,
    pub slug: String,
    pub title: String,
    pub content_html: String,
    pub excerpt: String,
    pub sticker: Option<String>,
    pub published_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<super::tag::TagOut>,
    pub comment_count: i64,
}

/// 管理端文章（含草稿，含 Markdown 原文便于回填编辑器）
#[derive(Debug, Serialize)]
pub struct PostAdmin {
    pub id: u64,
    pub slug: String,
    pub title: String,
    pub content_md: String,
    pub status: i8,
    pub sticker: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<super::tag::TagOut>,
    pub comment_count: i64,
}

/// 首页数据：最新 N 篇 + 站点统计（驱动「已记录 N 天」徽标）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeData {
    pub posts: Vec<PostSummary>,
    pub total_posts: i64,
    pub recorded_days: i64,
}

/// 归档：按月分组
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveMonth {
    pub month: String, // "2026-11"
    pub posts: Vec<PostSummary>,
}

impl Post {
    pub fn to_summary(&self, tags: Vec<super::tag::TagOut>, excerpt: String) -> PostSummary {
        PostSummary {
            id: self.id,
            slug: self.slug.clone(),
            title: self.title.clone(),
            excerpt,
            sticker: self.sticker.clone(),
            published_at: self.published_at.map(rfc3339).unwrap_or_else(Utc::now),
            tags,
        }
    }

    pub fn to_admin(&self, tags: Vec<super::tag::TagOut>, comment_count: i64) -> PostAdmin {
        PostAdmin {
            id: self.id,
            slug: self.slug.clone(),
            title: self.title.clone(),
            content_md: self.content_md.clone(),
            status: self.status,
            sticker: self.sticker.clone(),
            published_at: self.published_at.map(rfc3339),
            updated_at: rfc3339(self.updated_at),
            tags,
            comment_count,
        }
    }
}
