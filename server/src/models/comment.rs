//! 评论：DB 行结构 + DTO（游客以 昵称+邮箱+可选网址 发布，无需注册）

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use validator::Validate;

#[derive(Debug, Clone, FromRow)]
pub struct Comment {
    pub id: u64,
    pub post_id: u64,
    pub parent_id: Option<u64>,
    pub author_name: String,
    #[allow(dead_code)]
    pub author_email: String, // 仅存储与通知用，永不对外输出
    pub author_url: Option<String>,
    pub content: String,
    /// 查询时已带状态条件（status = 1），行本身的 status 仅供调试
    #[allow(dead_code)]
    pub status: i8,
    pub created_at: NaiveDateTime,
}

/// 公开输出的评论节点（树形）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentNode {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub author_name: String,
    pub author_url: Option<String>,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub children: Vec<CommentNode>,
}

/// 管理端评论（附文章标题）
#[derive(Debug, Serialize)]
pub struct CommentAdmin {
    pub id: u64,
    pub post_id: u64,
    pub post_title: String,
    pub parent_id: Option<u64>,
    pub author_name: String,
    pub author_email: String,
    pub author_url: Option<String>,
    pub content: String,
    pub status: i8,
    pub created_at: DateTime<Utc>,
}

impl Comment {
    pub fn to_node(&self) -> CommentNode {
        CommentNode {
            id: self.id,
            parent_id: self.parent_id,
            author_name: self.author_name.clone(),
            author_url: self.author_url.clone(),
            content: self.content.clone(),
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(self.created_at, Utc),
            children: Vec::new(),
        }
    }
}

/// 把扁平列表组装成两层树（顶层 parent_id IS NULL，其余挂到顶层节点下）。
/// 后台创建时已把「回复的回复」拍扁到顶层，这里不再递归加深。
pub fn build_tree(flat: Vec<Comment>) -> Vec<CommentNode> {
    let mut roots: Vec<CommentNode> = Vec::new();
    let mut index: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
    for c in &flat {
        if c.parent_id.is_none() {
            index.insert(c.id, roots.len());
            roots.push(c.to_node());
        }
    }
    for c in &flat {
        if let Some(pid) = c.parent_id {
            if let Some(&i) = index.get(&pid) {
                roots[i].children.push(c.to_node());
            } else {
                // 父评论已被删除/拒绝：作为顶层展示，避免内容丢失
                roots.push(c.to_node());
            }
        }
    }
    roots
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateCommentReq {
    /// 文章 slug 或 post_id 二选一
    pub slug: Option<String>,
    pub post_id: Option<u64>,
    pub parent_id: Option<u64>,
    #[validate(length(min = 1, max = 64))]
    pub name: String,
    #[validate(email, length(max = 190))]
    pub email: String,
    #[validate(url, length(max = 190))]
    pub url: Option<String>,
    #[validate(length(min = 1, max = 2000))]
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: u64, parent: Option<u64>, name: &str) -> Comment {
        Comment {
            id,
            post_id: 1,
            parent_id: parent,
            author_name: name.into(),
            author_email: "a@b.c".into(),
            author_url: None,
            content: "hi".into(),
            status: 1,
            created_at: chrono::NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
        }
    }

    #[test]
    fn tree_nests_replies_under_top_level() {
        let flat = vec![c(1, None, "甲"), c(2, Some(1), "乙"), c(3, None, "丙")];
        let tree = build_tree(flat);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].author_name, "乙");
    }

    #[test]
    fn orphan_reply_becomes_root() {
        let flat = vec![c(9, Some(404), "孤儿")];
        let tree = build_tree(flat);
        assert_eq!(tree.len(), 1);
    }
}
