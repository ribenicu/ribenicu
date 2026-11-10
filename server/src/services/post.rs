//! 文章业务层：查询 + 缓存读写 + 写后失效

use std::collections::HashMap;

use crate::error::AppResult;
use crate::models::post::{ArchiveMonth, HomeData, Post, PostAdmin, PostDetail, PostSummary};
use crate::models::tag::TagOut;
use crate::services::cache::{self, keys};
use crate::state::AppState;
use sqlx::MySqlPool;

const EXCERPT_LEN: usize = 120;
pub const HOME_LIMIT: i64 = 6;

// ---------- 标签 ----------

/// 批量取多篇文章的标签
pub async fn tags_for_posts(pool: &MySqlPool, ids: &[u64]) -> AppResult<HashMap<u64, Vec<TagOut>>> {
    let mut map: HashMap<u64, Vec<TagOut>> = HashMap::new();
    if ids.is_empty() {
        return Ok(map);
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT pt.post_id, t.id, t.name, t.color FROM post_tags pt \
         JOIN tags t ON t.id = pt.tag_id WHERE pt.post_id IN ({placeholders}) \
         ORDER BY t.id"
    );
    let mut q = sqlx::query_as::<_, (u64, u64, String, String)>(&sql);
    for id in ids {
        q = q.bind(id);
    }
    for row in q.fetch_all(pool).await? {
        map.entry(row.0).or_default().push(TagOut {
            id: row.1,
            name: row.2,
            color: row.3,
        });
    }
    Ok(map)
}

pub async fn tags_for_post(pool: &MySqlPool, post_id: u64) -> AppResult<Vec<TagOut>> {
    let rows = sqlx::query_as::<_, (u64, String, String)>(
        "SELECT t.id, t.name, t.color FROM post_tags pt \
         JOIN tags t ON t.id = pt.tag_id WHERE pt.post_id = ? ORDER BY t.id",
    )
    .bind(post_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, name, color)| TagOut { id, name, color })
        .collect())
}

pub async fn list_all_tags(state: &AppState) -> AppResult<Vec<TagOut>> {
    if let Some(cached) = cache::get_json(&state.redis, &keys::tag_all()).await {
        return Ok(cached);
    }
    let rows =
        sqlx::query_as::<_, (u64, String, String)>("SELECT id, name, color FROM tags ORDER BY id")
            .fetch_all(&state.db)
            .await?;
    let tags: Vec<TagOut> = rows
        .into_iter()
        .map(|(id, name, color)| TagOut { id, name, color })
        .collect();
    cache::set_json(&state.redis, &keys::tag_all(), &tags, keys::TTL_TAG_ALL).await;
    Ok(tags)
}

// ---------- 公开读（带缓存） ----------

fn summary(post: &Post, tags: Vec<TagOut>) -> PostSummary {
    let excerpt = post
        .excerpt
        .clone()
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| crate::util::make_excerpt(&post.content_md, EXCERPT_LEN));
    post.to_summary(tags, excerpt)
}

pub async fn list_home(state: &AppState) -> AppResult<HomeData> {
    if let Some(cached) = cache::get_json(&state.redis, &keys::post_home()).await {
        return Ok(cached);
    }
    let posts = sqlx::query_as::<_, Post>(
        "SELECT * FROM posts WHERE status = 1 ORDER BY published_at DESC LIMIT ?",
    )
    .bind(HOME_LIMIT)
    .fetch_all(&state.db)
    .await?;
    let ids: Vec<u64> = posts.iter().map(|p| p.id).collect();
    let tag_map = tags_for_posts(&state.db, &ids).await?;
    let posts: Vec<PostSummary> = posts
        .iter()
        .map(|p| summary(p, tag_map.get(&p.id).cloned().unwrap_or_default()))
        .collect();

    let total_posts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM posts WHERE status = 1")
        .fetch_one(&state.db)
        .await?;
    let recorded_days: i64 =
        sqlx::query_scalar("SELECT COUNT(DISTINCT DATE(published_at)) FROM posts WHERE status = 1")
            .fetch_one(&state.db)
            .await?;

    let data = HomeData {
        posts,
        total_posts,
        recorded_days,
    };
    cache::set_json(&state.redis, &keys::post_home(), &data, keys::TTL_POST_HOME).await;
    Ok(data)
}

pub async fn archive(state: &AppState) -> AppResult<Vec<ArchiveMonth>> {
    if let Some(cached) = cache::get_json(&state.redis, &keys::post_archive()).await {
        return Ok(cached);
    }
    let posts = sqlx::query_as::<_, Post>(
        "SELECT * FROM posts WHERE status = 1 ORDER BY published_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    let ids: Vec<u64> = posts.iter().map(|p| p.id).collect();
    let tag_map = tags_for_posts(&state.db, &ids).await?;

    let mut months: Vec<ArchiveMonth> = Vec::new();
    for p in &posts {
        let month = p
            .published_at
            .map(|d| d.format("%Y-%m").to_string())
            .unwrap_or_else(|| "0000-00".into());
        let s = summary(p, tag_map.get(&p.id).cloned().unwrap_or_default());
        if let Some(last) = months.last_mut() {
            if last.month == month {
                last.posts.push(s);
                continue;
            }
        }
        months.push(ArchiveMonth {
            month,
            posts: vec![s],
        });
    }
    cache::set_json(
        &state.redis,
        &keys::post_archive(),
        &months,
        keys::TTL_POST_ARCHIVE,
    )
    .await;
    Ok(months)
}

pub async fn detail_by_slug(state: &AppState, slug: &str) -> AppResult<Option<PostDetail>> {
    let key = keys::post_detail(slug);
    if let Some(cached) = cache::get_json(&state.redis, &key).await {
        return Ok(Some(cached));
    }
    let Some(post) = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE slug = ? AND status = 1")
        .bind(slug)
        .fetch_optional(&state.db)
        .await?
    else {
        return Ok(None);
    };
    let detail = detail_from_row(state, &post).await?;
    cache::set_json(&state.redis, &key, &detail, keys::TTL_POST_DETAIL).await;
    Ok(Some(detail))
}

async fn detail_from_row(state: &AppState, post: &Post) -> AppResult<PostDetail> {
    let tags = tags_for_post(&state.db, post.id).await?;
    let comment_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM comments WHERE post_id = ? AND status = 1")
            .bind(post.id)
            .fetch_one(&state.db)
            .await?;
    let excerpt = post
        .excerpt
        .clone()
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| crate::util::make_excerpt(&post.content_md, EXCERPT_LEN));
    let published_at = post.published_at.unwrap_or(post.created_at);
    Ok(PostDetail {
        id: post.id,
        slug: post.slug.clone(),
        title: post.title.clone(),
        content_html: crate::util::render_markdown(&post.content_md),
        excerpt,
        sticker: post.sticker.clone(),
        published_at: chrono::DateTime::from_naive_utc_and_offset(published_at, chrono::Utc),
        updated_at: chrono::DateTime::from_naive_utc_and_offset(post.updated_at, chrono::Utc),
        tags,
        comment_count,
    })
}

/// 供评论接口内部使用：按 slug 找已发布文章
pub async fn published_by_slug(pool: &MySqlPool, slug: &str) -> AppResult<Option<Post>> {
    Ok(
        sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE slug = ? AND status = 1")
            .bind(slug)
            .fetch_optional(pool)
            .await?,
    )
}

// ---------- 管理端（含草稿，不走缓存） ----------

pub async fn admin_list(
    state: &AppState,
    page: u32,
    per_page: u32,
) -> AppResult<(Vec<PostAdmin>, i64)> {
    let offset = (page.saturating_sub(1)) * per_page;
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM posts")
        .fetch_one(&state.db)
        .await?;
    let posts = sqlx::query_as::<_, Post>(
        "SELECT * FROM posts ORDER BY COALESCE(published_at, created_at) DESC LIMIT ? OFFSET ?",
    )
    .bind(per_page as i64)
    .bind(offset as i64)
    .fetch_all(&state.db)
    .await?;
    let ids: Vec<u64> = posts.iter().map(|p| p.id).collect();
    let tag_map = tags_for_posts(&state.db, &ids).await?;
    let mut out = Vec::with_capacity(posts.len());
    for p in &posts {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM comments WHERE post_id = ? AND status != 2")
                .bind(p.id)
                .fetch_one(&state.db)
                .await?;
        out.push(p.to_admin(tag_map.get(&p.id).cloned().unwrap_or_default(), count));
    }
    Ok((out, total))
}

pub struct PostInput {
    pub title: String,
    pub content_md: String,
    pub sticker: Option<String>,
    pub status: i8,
    pub tag_ids: Vec<u64>,
}

pub async fn create(state: &AppState, input: PostInput) -> AppResult<PostAdmin> {
    let base = crate::util::unique_slug_base(&input.title);
    let slug = crate::util::ensure_unique_slug(&base, |s| {
        let pool = state.db.clone();
        async move {
            let exists: Option<u64> = sqlx::query_scalar("SELECT id FROM posts WHERE slug = ?")
                .bind(&s)
                .fetch_optional(&pool)
                .await?;
            Ok(exists.is_some())
        }
    })
    .await?;
    let excerpt = crate::util::make_excerpt(&input.content_md, EXCERPT_LEN);
    let published_at: Option<chrono::NaiveDateTime> = if input.status == 1 {
        Some(chrono::Utc::now().naive_utc())
    } else {
        None
    };

    let result = sqlx::query(
        "INSERT INTO posts (slug, title, content_md, excerpt, sticker, status, published_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&slug)
    .bind(&input.title)
    .bind(&input.content_md)
    .bind(&excerpt)
    .bind(&input.sticker)
    .bind(input.status)
    .bind(published_at)
    .execute(&state.db)
    .await?;
    let post_id = result.last_insert_id();
    replace_tags(&state.db, post_id, &input.tag_ids).await?;

    invalidate_post_caches(state, Some(&slug)).await;
    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_one(&state.db)
        .await?;
    let tags = tags_for_post(&state.db, post_id).await?;
    Ok(post.to_admin(tags, 0))
}

pub async fn update(state: &AppState, id: u64, input: PostInput) -> AppResult<PostAdmin> {
    let old = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    let Some(old) = old else {
        return Err(crate::error::AppError::NotFound("文章不存在".into()));
    };

    let excerpt = crate::util::make_excerpt(&input.content_md, EXCERPT_LEN);
    // 草稿首次转发布时补发布时间
    let published_at: Option<chrono::NaiveDateTime> = if input.status == 1 {
        Some(
            old.published_at
                .unwrap_or_else(|| chrono::Utc::now().naive_utc()),
        )
    } else {
        old.published_at
    };

    sqlx::query(
        "UPDATE posts SET title = ?, content_md = ?, excerpt = ?, sticker = ?, status = ?, \
         published_at = ? WHERE id = ?",
    )
    .bind(&input.title)
    .bind(&input.content_md)
    .bind(&excerpt)
    .bind(&input.sticker)
    .bind(input.status)
    .bind(published_at)
    .bind(id)
    .execute(&state.db)
    .await?;
    replace_tags(&state.db, id, &input.tag_ids).await?;

    invalidate_post_caches(state, Some(&old.slug)).await;
    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM comments WHERE post_id = ? AND status != 2")
            .bind(id)
            .fetch_one(&state.db)
            .await?;
    let tags = tags_for_post(&state.db, id).await?;
    Ok(post.to_admin(tags, count))
}

pub async fn delete(state: &AppState, id: u64) -> AppResult<()> {
    let old = sqlx::query_as::<_, Post>("SELECT slug FROM posts WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    let Some(old) = old else {
        return Err(crate::error::AppError::NotFound("文章不存在".into()));
    };
    sqlx::query("DELETE FROM posts WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    invalidate_post_caches(state, Some(&old.slug)).await;
    Ok(())
}

async fn replace_tags(pool: &MySqlPool, post_id: u64, tag_ids: &[u64]) -> AppResult<()> {
    sqlx::query("DELETE FROM post_tags WHERE post_id = ?")
        .bind(post_id)
        .execute(pool)
        .await?;
    for tag_id in tag_ids {
        sqlx::query("INSERT IGNORE INTO post_tags (post_id, tag_id) VALUES (?, ?)")
            .bind(post_id)
            .bind(tag_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// 写后失效：详情（按 slug）、首页、归档、标签
pub async fn invalidate_post_caches(state: &AppState, slug: Option<&str>) {
    let mut ks = vec![keys::post_home(), keys::post_archive(), keys::tag_all()];
    if let Some(s) = slug {
        ks.push(keys::post_detail(s));
    }
    cache::del(&state.redis, &ks).await;
}
