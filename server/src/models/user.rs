//! 作者账号（单一/少量）
//!
//! 登录方式已从「用户名密码」切换为「邮箱验证码」，password_hash 仅作为兼容旧数据的保留列。

use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: u64,
    pub username: String,
    #[allow(dead_code)]
    pub email: Option<String>,
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
