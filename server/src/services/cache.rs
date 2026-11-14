//! Redis 缓存封装：Cache-Aside + 写后失效。
//!
//! 所有函数对 Redis 不可用（None 或命令失败）都降级为空操作并告警，
//! 服务行为退化为「全部回源 MySQL」，见 docs/development-plan.md §5。

use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};

/// Key 模式与 TTL（秒），与计划 §5 对应
pub mod keys {
    pub const TTL_POST_DETAIL: u64 = 600; // 10 min
    pub const TTL_POST_HOME: u64 = 300; // 5 min
    pub const TTL_POST_ARCHIVE: u64 = 600; // 10 min
    pub const TTL_TAG_ALL: u64 = 1800; // 30 min
    pub const TTL_COMMENT_LIST: u64 = 120; // 2 min
    pub const TTL_EMAIL_CODE: u64 = 300; // 5 min

    pub fn post_detail(slug: &str) -> String {
        format!("post:detail:{slug}")
    }
    pub fn post_home() -> String {
        "post:list:home".into()
    }
    pub fn post_archive() -> String {
        "post:list:archive".into()
    }
    pub fn tag_all() -> String {
        "tag:all".into()
    }
    pub fn comment_list(post_id: u64) -> String {
        format!("comment:list:{post_id}")
    }
    pub fn session(token: &str) -> String {
        format!("session:{token}")
    }
    pub fn rate_comment(ip: &str) -> String {
        format!("rate:comment:{ip}")
    }
    pub fn rate_login(ip: &str) -> String {
        format!("rate:login:{ip}")
    }
    /// 邮箱验证码：email_code:{email} -> 6 位数字码
    pub fn email_code(email: &str) -> String {
        format!("email_code:{email}")
    }
    /// 验证码错误尝试次数：email_code_attempts:{email} -> INCR 计数
    pub fn email_code_attempts(email: &str) -> String {
        format!("email_code_attempts:{email}")
    }
    /// 发送验证码限流：rate:email_send:{ip_or_email}
    pub fn rate_email_send(identifier: &str) -> String {
        format!("rate:email_send:{identifier}")
    }
}

/// 读取并反序列化；miss / 故障 → None
pub async fn get_json<T: DeserializeOwned>(
    redis: &Option<redis::aio::MultiplexedConnection>,
    key: &str,
) -> Option<T> {
    let mut conn = redis.as_ref()?.clone();
    match conn.get::<_, Option<String>>(key).await {
        Ok(Some(raw)) => match serde_json::from_str(&raw) {
            Ok(v) => {
                tracing::debug!(key, "cache hit");
                Some(v)
            }
            Err(e) => {
                tracing::warn!(key, error = %e, "cache value corrupt, deleting");
                let _ = conn.del::<_, ()>(key).await;
                None
            }
        },
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(key, error = %e, "redis GET failed");
            None
        }
    }
}

/// 写入 JSON；故障仅告警
pub async fn set_json<T: Serialize>(
    redis: &Option<redis::aio::MultiplexedConnection>,
    key: &str,
    value: &T,
    ttl_secs: u64,
) {
    let Some(mut conn) = redis.as_ref().cloned() else {
        return;
    };
    let Ok(raw) = serde_json::to_string(value) else {
        return;
    };
    if let Err(e) = conn.set_ex::<_, _, ()>(key, raw, ttl_secs).await {
        tracing::warn!(key, error = %e, "redis SET failed");
    }
}

/// 主动失效（写后失效的核心）；故障仅告警
pub async fn del(redis: &Option<redis::aio::MultiplexedConnection>, keys: &[String]) {
    if keys.is_empty() {
        return;
    }
    let Some(mut conn) = redis.as_ref().cloned() else {
        return;
    };
    let owned: Vec<String> = keys.to_vec();
    if let Err(e) = conn.del::<_, ()>(owned).await {
        tracing::warn!(keys = ?keys, error = %e, "redis DEL failed");
    }
}

/// 按模式批量失效（SCAN + DEL），用于标签改名/删除后清理全部文章详情缓存。
/// 站点规模很小，SCAN 的成本可以接受；故障仅告警。
pub async fn del_pattern(redis: &Option<redis::aio::MultiplexedConnection>, pattern: &str) {
    let Some(mut conn) = redis.as_ref().cloned() else {
        return;
    };
    let mut cursor = "0".to_string();
    loop {
        let result: redis::RedisResult<(String, Vec<String>)> = redis::cmd("SCAN")
            .arg(&cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(200)
            .query_async(&mut conn)
            .await;
        let (next, keys) = match result {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(pattern, error = %e, "redis SCAN failed");
                return;
            }
        };
        if !keys.is_empty() {
            if let Err(e) = conn.del::<_, ()>(keys).await {
                tracing::warn!(pattern, error = %e, "redis DEL by pattern failed");
            }
        }
        if next == "0" {
            break;
        }
        cursor = next;
    }
}
