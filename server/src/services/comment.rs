//! 评论业务层：发表（待审核）、公开查询（仅已通过）、后台审核

use crate::error::{AppError, AppResult};
use crate::models::comment::{build_tree, Comment, CommentAdmin, CommentNode, CreateCommentReq};
use crate::services::cache::{self, keys};
use crate::state::AppState;
use chrono::NaiveDateTime;

/// 公开：某文章已通过的评论（树形，缓存 2 min）
pub async fn list_approved(state: &AppState, post_id: u64) -> AppResult<Vec<CommentNode>> {
    let key = keys::comment_list(post_id);
    if let Some(cached) = cache::get_json(&state.redis, &key).await {
        return Ok(cached);
    }
    let flat = sqlx::query_as::<_, Comment>(
        "SELECT * FROM comments WHERE post_id = ? AND status = 1 ORDER BY created_at ASC, id ASC",
    )
    .bind(post_id)
    .fetch_all(&state.db)
    .await?;
    let tree = build_tree(flat);
    cache::set_json(&state.redis, &key, &tree, keys::TTL_COMMENT_LIST).await;
    Ok(tree)
}

/// 游客发表评论：默认待审核（status=0）；楼中楼最多两层
pub async fn create(state: &AppState, mut req: CreateCommentReq) -> AppResult<Comment> {
    // 内容去首尾空白后非空（配合 validator 长度限制）
    req.name = req.name.trim().to_string();
    req.content = req.content.trim().to_string();
    if req.name.is_empty() || req.content.is_empty() {
        return Err(AppError::Validation("昵称和内容不能为空".into()));
    }
    req.url = req
        .url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if let Some(u) = &req.url {
        // validator 的 url 校验允许 ftp 等，这里收窄到 http/https
        if !(u.starts_with("http://") || u.starts_with("https://")) {
            return Err(AppError::Validation("网址仅支持 http/https".into()));
        }
    }

    let post_id = if let Some(id) = req.post_id {
        let exists: Option<u64> =
            sqlx::query_scalar("SELECT id FROM posts WHERE id = ? AND status = 1")
                .bind(id)
                .fetch_optional(&state.db)
                .await?;
        match exists {
            Some(_) => id,
            None => return Err(AppError::NotFound("文章不存在或未发布".into())),
        }
    } else if let Some(slug) = &req.slug {
        let post = crate::services::post::published_by_slug(&state.db, slug).await?;
        match post {
            Some(p) => p.id,
            None => return Err(AppError::NotFound("文章不存在或未发布".into())),
        }
    } else {
        return Err(AppError::Validation("缺少文章标识".into()));
    };

    // 校验父评论：必须属于同一篇文章；回复的回复拍扁到顶层（最多两层）
    let parent_id = if let Some(pid) = req.parent_id {
        let parent: Option<(Option<u64>, u64)> =
            sqlx::query_as("SELECT parent_id, post_id FROM comments WHERE id = ?")
                .bind(pid)
                .fetch_optional(&state.db)
                .await?;
        match parent {
            Some((Some(grand), _)) => Some(grand), // 拍扁
            Some((None, p_post)) if p_post == post_id => Some(pid),
            Some((None, _)) => return Err(AppError::Validation("父评论不属于这篇文章".into())),
            None => return Err(AppError::Validation("父评论不存在".into())),
        }
    } else {
        None
    };

    let result = sqlx::query(
        "INSERT INTO comments (post_id, parent_id, author_name, author_email, author_url, content, status) \
         VALUES (?, ?, ?, ?, ?, ?, 0)",
    )
    .bind(post_id)
    .bind(parent_id)
    .bind(&req.name)
    .bind(req.email.trim())
    .bind(&req.url)
    .bind(&req.content)
    .execute(&state.db)
    .await?;
    let id = result.last_insert_id();

    // 新评论不影响已通过列表，无需失效缓存（审核通过时才失效）
    let comment = sqlx::query_as::<_, Comment>("SELECT * FROM comments WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(comment)
}

/// 管理端：评论列表（分页 + 按状态筛选）
pub async fn admin_list(
    state: &AppState,
    status: Option<i8>,
    page: u32,
    per_page: u32,
) -> AppResult<(Vec<CommentAdmin>, i64)> {
    let offset = (page.saturating_sub(1)) * per_page;
    let (where_sql, total): (String, i64) = match status {
        Some(s) => {
            let t: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comments WHERE status = ?")
                .bind(s)
                .fetch_one(&state.db)
                .await?;
            ("WHERE c.status = ?".into(), t)
        }
        None => {
            let t: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comments")
                .fetch_one(&state.db)
                .await?;
            (String::new(), t)
        }
    };
    let sql = format!(
        "SELECT c.*, p.title AS post_title FROM comments c \
         JOIN posts p ON p.id = c.post_id {where_sql} \
         ORDER BY c.created_at DESC LIMIT ? OFFSET ?"
    );
    let mut q = sqlx::query_as::<_, CommentRow>(&sql);
    if let Some(s) = status {
        q = q.bind(s);
    }
    let rows = q
        .bind(per_page as i64)
        .bind(offset as i64)
        .fetch_all(&state.db)
        .await?;
    let out = rows
        .into_iter()
        .map(|r| CommentAdmin {
            id: r.id,
            post_id: r.post_id,
            post_title: r.post_title,
            parent_id: r.parent_id,
            author_name: r.author_name,
            author_email: r.author_email,
            author_url: r.author_url,
            content: r.content,
            status: r.status,
            created_at: chrono::DateTime::from_naive_utc_and_offset(r.created_at, chrono::Utc),
        })
        .collect();
    Ok((out, total))
}

#[derive(sqlx::FromRow)]
struct CommentRow {
    id: u64,
    post_id: u64,
    post_title: String,
    parent_id: Option<u64>,
    author_name: String,
    author_email: String,
    author_url: Option<String>,
    content: String,
    status: i8,
    created_at: NaiveDateTime,
}

/// 审核通过 / 拒绝（软删）。变更后失效该文章的评论缓存。
pub async fn moderate(state: &AppState, id: u64, status: i8) -> AppResult<()> {
    let row: Option<(u64, i8)> =
        sqlx::query_as("SELECT post_id, status FROM comments WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let Some((post_id, old_status)) = row else {
        return Err(AppError::NotFound("评论不存在".into()));
    };
    sqlx::query("UPDATE comments SET status = ? WHERE id = ?")
        .bind(status)
        .bind(id)
        .execute(&state.db)
        .await?;
    // 待审核 → 已通过：公开列表变化，需要失效
    // 已通过 → 拒绝：同样变化；其余状态流转对公开列表无影响
    if old_status == 1 || status == 1 {
        cache::del(&state.redis, &[keys::comment_list(post_id)]).await;
    }
    Ok(())
}

/// 待审核数量（后台角标用）
pub async fn pending_count(state: &AppState) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM comments WHERE status = 0")
            .fetch_one(&state.db)
            .await?,
    )
}
