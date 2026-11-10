//! 应用状态：MySQL 连接池 + Redis 多路复用连接 + 配置。
//!
//! Redis 不可用时降级：缓存全部 miss、限流放行（fail-open），
//! 服务本身不中断，见 docs/development-plan.md §5。

use sqlx::MySqlPool;

#[derive(Clone)]
pub struct AppState {
    pub db: MySqlPool,
    pub redis: Option<redis::aio::MultiplexedConnection>,
    pub config: crate::config::Config,
}
