//! 作者账号（单一/少量，argon2 哈希）

use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: u64,
    pub username: String,
    #[allow(dead_code)]
    pub password_hash: String,
}

#[derive(Debug, Serialize)]
pub struct UserOut {
    pub id: u64,
    pub username: String,
}

impl From<User> for UserOut {
    fn from(u: User) -> Self {
        UserOut {
            id: u.id,
            username: u.username,
        }
    }
}
