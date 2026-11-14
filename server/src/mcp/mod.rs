//! MCP（Model Context Protocol）服务入口。
//!
//! 使用官方 Rust SDK `rmcp` 的 Streamable HTTP 传输，挂载在 `/mcp`：
//! - `POST /mcp`：JSON-RPC 请求（initialize、tools/list、tools/call 等）
//! - `GET /mcp`：SSE 通知流
//! - `DELETE /mcp`：结束会话
//!
//! 每个 MCP 会话拥有独立的 `RibenMcp` 实例，因此邮箱登录状态按会话隔离。

mod server;
mod types;

pub use server::RibenMcp;

use std::sync::Arc;

use axum::Router;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tokio_util::sync::CancellationToken;

use crate::state::AppState;

/// 构造 `/mcp` 路由。未启用时调用方不会调用它。
pub fn router(state: AppState) -> Router {
    let config = StreamableHttpServerConfig::default()
        .with_allowed_hosts(state.config.mcp_allowed_hosts.clone())
        .with_allowed_origins(state.config.mcp_allowed_origins.clone())
        .with_cancellation_token(CancellationToken::new());

    let service: StreamableHttpService<RibenMcp, LocalSessionManager> = StreamableHttpService::new(
        {
            let state = state.clone();
            move || Ok(RibenMcp::new(state.clone()))
        },
        Arc::new(LocalSessionManager::default()),
        config,
    );

    Router::new().nest_service("/mcp", service)
}
