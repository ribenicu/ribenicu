//! 标签 DTO

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use validator::Validate;

#[derive(Debug, Clone, FromRow)]
pub struct Tag {
    pub id: u64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagOut {
    pub id: u64,
    pub name: String,
    pub color: String,
}

impl From<Tag> for TagOut {
    fn from(t: Tag) -> Self {
        TagOut {
            id: t.id,
            name: t.name,
            color: t.color,
        }
    }
}

#[derive(Debug, serde::Deserialize, Validate)]
pub struct CreateTagReq {
    #[validate(length(min = 1, max = 64))]
    pub name: String,
    #[validate(length(min = 1, max = 16))]
    pub color: String,
}
