//! MCP 服务实现：会话级邮箱登录鉴权 + 文章/评论/标签/系统工具。
//!
//! 传输层由 rmcp 的 Streamable HTTP 负责；本模块只关心工具语义。
//! 鉴权状态保存在每个 MCP 会话自己的 `RibenMcp` 实例里：
//! `auth_send_email_code` + `auth_login` 成功后，同一会话的后续工具调用即为管理员身份。
//! 也支持在 HTTP 请求头携带 `x-admin-token` / `Authorization: Bearer <ADMIN_API_TOKEN>` 直接鉴权。

use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, Implementation, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router, ErrorData, ServerHandler};
use serde::Serialize;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::middleware::auth::{tokens_equal, AuthUser};
use crate::services::{auth as auth_svc, comment as comment_svc, post as post_svc, system};
use crate::state::AppState;

use super::types::*;

/// MCP 服务说明，客户端在 initialize 时可见。
const INSTRUCTIONS: &str = r#"一本日记（riben.icu）管理 MCP。

登录流程：
1. 调用 auth_send_email_code，传入管理员邮箱；
2. 从邮箱读取 6 位验证码（5 分钟内有效）；
3. 调用 auth_login 完成登录，之后同一 MCP 会话即可调用所有 admin_* 工具。

也可以在 MCP 客户端配置请求头 x-admin-token: <ADMIN_API_TOKEN>，跳过邮箱登录。
未登录时 admin_* 工具会返回登录提示。

约定：
- 文章状态：0=草稿，1=已发布；评论状态：0=待审核，1=已通过，2=已拒绝。
- slug 由服务端在创建文章时生成，之后不可修改。
- 创建/更新文章可用 tag_ids 或 tag_names 指定标签（名称须已存在）。
- 不确定标签时先调用 admin_list_tags；发布前可用 preview_markdown 预览渲染结果。"#;

#[derive(Clone)]
pub struct RibenMcp {
    state: AppState,
    auth: Arc<Mutex<Option<AuthSession>>>,
    tool_router: ToolRouter<Self>,
}

#[derive(Clone)]
struct AuthSession {
    user_id: u64,
    username: String,
    kind: AuthKind,
}

#[derive(Clone)]
enum AuthKind {
    /// 邮箱验证码登录产生的 Redis Session token
    Session(String),
    /// 通过 ADMIN_API_TOKEN 请求头鉴权
    AdminToken,
}

impl RibenMcp {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            auth: Arc::new(Mutex::new(None)),
            tool_router: Self::tool_router(),
        }
    }

    fn auth_snapshot(&self) -> Option<AuthSession> {
        self.auth.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn set_auth(&self, session: AuthSession) {
        *self.auth.lock().unwrap_or_else(|e| e.into_inner()) = Some(session);
    }

    fn clear_auth(&self) {
        *self.auth.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// 若请求头携带正确的 ADMIN_API_TOKEN，把本会话标记为管理员。
    /// 无效 token 不会影响已有的邮箱登录状态。
    async fn try_header_auth(&self, ctx: &RequestContext<RoleServer>) {
        let Some(parts) = ctx.extensions.get::<axum::http::request::Parts>() else {
            return;
        };
        let Some(presented) = header_admin_token(parts) else {
            return;
        };
        let Some(expected) = self.state.config.admin_api_token.as_deref() else {
            return;
        };
        if !tokens_equal(expected, &presented) {
            return;
        }
        match auth_svc::first_admin(&self.state).await {
            Ok(Some(user)) => {
                tracing::info!(username = %user.username, "mcp auth via admin api token");
                self.set_auth(AuthSession {
                    user_id: user.id,
                    username: user.username,
                    kind: AuthKind::AdminToken,
                });
            }
            Ok(None) => tracing::warn!("mcp admin api token auth: users table is empty"),
            Err(e) => tracing::warn!(error = %e, "mcp admin api token auth: user lookup failed"),
        }
    }

    /// 校验当前会话的管理员身份；邮箱登录的会话每次调用都会回源 Redis 校验并滑动续期。
    async fn require_admin(&self) -> Result<AuthUser, String> {
        let Some(current) = self.auth_snapshot() else {
            return Err(login_hint());
        };
        match current.kind {
            AuthKind::Session(token) => {
                match auth_svc::resolve_session(&self.state, &token).await {
                    Ok(Some(user)) => Ok(AuthUser {
                        id: user.id,
                        username: user.username,
                        token,
                    }),
                    Ok(None) => {
                        self.clear_auth();
                        Err("登录已过期，请重新调用 auth_send_email_code / auth_login".into())
                    }
                    Err(e) => Err(err(e)),
                }
            }
            AuthKind::AdminToken => Ok(AuthUser {
                id: current.user_id,
                username: current.username,
                token: String::new(),
            }),
        }
    }

    /// 解析 tag_ids + tag_names 为去重后的标签 ID 列表；名称或 ID 不存在时给出可用列表。
    async fn resolve_tag_ids(
        &self,
        tag_ids: Vec<u64>,
        tag_names: Vec<String>,
    ) -> Result<Vec<u64>, String> {
        let all = post_svc::list_all_tags(&self.state).await.map_err(err)?;
        let mut out = Vec::with_capacity(tag_ids.len() + tag_names.len());
        for id in tag_ids {
            if !all.iter().any(|t| t.id == id) {
                return Err(format!(
                    "标签 ID {id} 不存在。可用标签：{}",
                    tag_catalog(&all)
                ));
            }
            out.push(id);
        }
        for name in tag_names {
            let name = name.trim();
            match all.iter().find(|t| t.name == name) {
                Some(tag) => out.push(tag.id),
                None => {
                    return Err(format!(
                        "标签「{name}」不存在。可用标签：{}",
                        tag_catalog(&all)
                    ));
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        Ok(out)
    }

    async fn admin_email(&self, user_id: u64) -> String {
        if let Ok(Some(user)) = auth_svc::user_by_id(&self.state, user_id).await {
            if let Some(email) = user.email {
                return email;
            }
        }
        self.state
            .config
            .admin_email
            .clone()
            .unwrap_or_else(|| "admin@riben.icu".into())
    }
}

#[tool_router]
impl RibenMcp {
    // ---------- 登录 ----------

    /// 向管理员邮箱发送 6 位登录验证码。
    #[tool(
        name = "auth_send_email_code",
        description = "向管理员邮箱发送 6 位登录验证码（5 分钟有效）。仅已注册的管理员邮箱会收到邮件。"
    )]
    async fn auth_send_email_code(
        &self,
        Parameters(req): Parameters<EmailInput>,
    ) -> Result<Json<Value>, String> {
        auth_svc::send_login_code(&self.state, &req.email)
            .await
            .map_err(err)?;
        Ok(Json(json!({
            "sent": true,
            "message": "若该邮箱是管理员邮箱，验证码已发送。请从邮箱读取 6 位验证码后调用 auth_login（5 分钟内有效）。"
        })))
    }

    /// 用邮箱 + 验证码登录，建立当前 MCP 会话的管理员身份。
    #[tool(
        name = "auth_login",
        description = "用邮箱和 6 位验证码登录。成功后当前 MCP 会话即可调用全部 admin_* 工具。"
    )]
    async fn auth_login(
        &self,
        Parameters(req): Parameters<LoginInput>,
    ) -> Result<Json<Value>, String> {
        let (user, token, ttl_secs) = auth_svc::login_with_code(&self.state, &req.email, &req.code)
            .await
            .map_err(err)?;
        self.set_auth(AuthSession {
            user_id: user.id,
            username: user.username.clone(),
            kind: AuthKind::Session(token),
        });
        Ok(Json(json!({
            "authenticated": true,
            "id": user.id,
            "username": user.username,
            "expires_in_secs": ttl_secs,
            "message": "登录成功，现在可以调用全部 admin_* 工具。"
        })))
    }

    /// 退出当前 MCP 会话。
    #[tool(
        name = "auth_logout",
        description = "退出当前 MCP 会话，销毁邮箱登录产生的服务端 Session。"
    )]
    async fn auth_logout(&self) -> Result<Json<Value>, String> {
        if let Some(auth) = self.auth_snapshot() {
            if let AuthKind::Session(token) = auth.kind {
                auth_svc::destroy_session(&self.state, &token).await;
            }
        }
        self.clear_auth();
        Ok(Json(json!({ "authenticated": false })))
    }

    /// 查看当前 MCP 会话的鉴权状态。
    #[tool(
        name = "auth_whoami",
        description = "查看当前 MCP 会话是否已登录，以及登录用户名。",
        annotations(read_only_hint = true)
    )]
    async fn auth_whoami(&self) -> Result<Json<Value>, String> {
        match self.require_admin().await {
            Ok(user) => Ok(Json(json!({
                "authenticated": true,
                "id": user.id,
                "username": user.username
            }))),
            Err(_) => Ok(Json(json!({
                "authenticated": false,
                "hint": login_hint()
            }))),
        }
    }

    // ---------- 公开读取 ----------

    /// 服务健康检查（MySQL / Redis / 版本）。
    #[tool(
        name = "health_check",
        description = "检查服务健康状态：MySQL 是否可用、Redis 是否可用、服务版本。",
        annotations(read_only_hint = true)
    )]
    async fn health_check(&self) -> Result<Json<Value>, String> {
        value(system::health(&self.state).await)
    }

    /// 首页数据：最新 6 篇已发布文章 + 站点统计。
    #[tool(
        name = "get_home",
        description = "获取首页数据：最新 6 篇已发布文章摘要、总文章数、记录天数。",
        annotations(read_only_hint = true)
    )]
    async fn get_home(&self) -> Result<Json<Value>, String> {
        value(post_svc::list_home(&self.state).await.map_err(err)?)
    }

    /// 按月归档的全部已发布文章。
    #[tool(
        name = "list_archive",
        description = "获取按月归档的已发布文章列表（含摘要与标签）。",
        annotations(read_only_hint = true)
    )]
    async fn list_archive(&self) -> Result<Json<Value>, String> {
        value(post_svc::archive(&self.state).await.map_err(err)?)
    }

    /// 按 slug 获取已发布文章详情（渲染后的 HTML）。
    #[tool(
        name = "get_post",
        description = "按 slug 获取已发布文章详情：标题、渲染后的 HTML、摘要、标签、评论数。",
        annotations(read_only_hint = true)
    )]
    async fn get_post(
        &self,
        Parameters(req): Parameters<SlugInput>,
    ) -> Result<Json<Value>, String> {
        match post_svc::detail_by_slug(&self.state, req.slug.trim())
            .await
            .map_err(err)?
        {
            Some(detail) => value(detail),
            None => Err("文章不存在或未发布".into()),
        }
    }

    /// 获取某篇文章已通过审核的评论树。
    #[tool(
        name = "list_public_comments",
        description = "获取某篇已发布文章已通过审核的评论树（两层结构）。",
        annotations(read_only_hint = true)
    )]
    async fn list_public_comments(
        &self,
        Parameters(req): Parameters<SlugInput>,
    ) -> Result<Json<Value>, String> {
        let slug = req.slug.trim();
        let post = post_svc::published_by_slug(&self.state.db, slug)
            .await
            .map_err(err)?
            .ok_or_else(|| "文章不存在或未发布".to_string())?;
        value(
            comment_svc::list_approved(&self.state, post.id)
                .await
                .map_err(err)?,
        )
    }

    // ---------- 文章管理 ----------

    /// 管理端文章列表（含草稿），可按状态过滤和分页。
    #[tool(
        name = "admin_list_posts",
        description = "管理端文章列表（含草稿与 Markdown 原文），可筛选状态并分页。",
        annotations(read_only_hint = true)
    )]
    async fn admin_list_posts(
        &self,
        Parameters(req): Parameters<ListAdminPostsInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        if let Some(status) = req.status {
            validate_status(status)?;
        }
        let page = req.page.unwrap_or(1).max(1);
        let per_page = req.per_page.unwrap_or(20).clamp(1, 100);
        let (list, total) = post_svc::admin_list_filtered(&self.state, req.status, page, per_page)
            .await
            .map_err(err)?;
        value(json!({
            "list": list,
            "total": total,
            "page": page,
            "per_page": per_page
        }))
    }

    /// 管理端文章详情（含 Markdown 原文、草稿）。
    #[tool(
        name = "admin_get_post",
        description = "按 ID 获取管理端文章详情，包含 Markdown 原文、状态、标签和评论数（可读取草稿）。",
        annotations(read_only_hint = true)
    )]
    async fn admin_get_post(
        &self,
        Parameters(req): Parameters<PostIdInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        value(
            post_svc::admin_get(&self.state, req.id)
                .await
                .map_err(err)?,
        )
    }

    /// 新建文章（草稿或直接发布）。
    #[tool(
        name = "admin_create_post",
        description = "新建文章。status 默认 0（草稿），传 1 立即发布。标签可用 tag_ids 或 tag_names 指定。"
    )]
    async fn admin_create_post(
        &self,
        Parameters(req): Parameters<CreatePostInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        let status = req.status.unwrap_or(0);
        validate_status(status)?;
        let sticker = req
            .sticker
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        validate_post(&req.title, &req.content_md, sticker.as_deref())?;
        let tag_ids = self
            .resolve_tag_ids(
                req.tag_ids.unwrap_or_default(),
                req.tag_names.unwrap_or_default(),
            )
            .await?;
        let created = post_svc::create(
            &self.state,
            post_svc::PostInput {
                title: req.title.trim().to_string(),
                content_md: req.content_md,
                sticker,
                status,
                tag_ids,
            },
        )
        .await
        .map_err(err)?;
        value(created)
    }

    /// 更新文章（支持部分字段）。
    #[tool(
        name = "admin_update_post",
        description = "更新文章，支持只传需要修改的字段。标签整体替换：传 tag_ids/tag_names 会覆盖原标签，传空数组表示清空。"
    )]
    async fn admin_update_post(
        &self,
        Parameters(req): Parameters<UpdatePostInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        if let Some(status) = req.status {
            validate_status(status)?;
        }
        if req.title.is_none()
            && req.content_md.is_none()
            && req.sticker.is_none()
            && req.status.is_none()
            && req.tag_ids.is_none()
            && req.tag_names.is_none()
        {
            return Err("没有提供任何要修改的字段".into());
        }

        let old = post_svc::admin_get(&self.state, req.id)
            .await
            .map_err(err)?;
        let title = req.title.unwrap_or(old.title);
        let content_md = req.content_md.unwrap_or(old.content_md);
        let sticker = match req.sticker {
            Some(s) if s.trim().is_empty() => None, // 空字符串 = 清除贴纸
            Some(s) => Some(s.trim().to_string()),
            None => old.sticker,
        };
        let status = req.status.unwrap_or(old.status);
        validate_post(&title, &content_md, sticker.as_deref())?;

        let tag_ids = if req.tag_ids.is_some() || req.tag_names.is_some() {
            self.resolve_tag_ids(
                req.tag_ids.unwrap_or_default(),
                req.tag_names.unwrap_or_default(),
            )
            .await?
        } else {
            old.tags.into_iter().map(|t| t.id).collect()
        };

        let updated = post_svc::update(
            &self.state,
            req.id,
            post_svc::PostInput {
                title: title.trim().to_string(),
                content_md,
                sticker,
                status,
                tag_ids,
            },
        )
        .await
        .map_err(err)?;
        value(updated)
    }

    /// 删除文章（连同其评论）。
    #[tool(
        name = "admin_delete_post",
        description = "删除文章及其全部评论，不可恢复。",
        annotations(destructive_hint = true)
    )]
    async fn admin_delete_post(
        &self,
        Parameters(req): Parameters<PostIdInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        post_svc::delete(&self.state, req.id).await.map_err(err)?;
        Ok(Json(json!({ "deleted": true, "id": req.id })))
    }

    /// 发布草稿（保留其余字段）。
    #[tool(
        name = "admin_publish_post",
        description = "把文章状态改为已发布（status=1），保留标题、正文、标签等字段。",
        annotations(idempotent_hint = true)
    )]
    async fn admin_publish_post(
        &self,
        Parameters(req): Parameters<PostIdInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        value(
            post_svc::set_status(&self.state, req.id, 1)
                .await
                .map_err(err)?,
        )
    }

    /// 把文章转为草稿（保留其余字段）。
    #[tool(
        name = "admin_unpublish_post",
        description = "把文章转为草稿（status=0），保留标题、正文、标签等字段。",
        annotations(idempotent_hint = true)
    )]
    async fn admin_unpublish_post(
        &self,
        Parameters(req): Parameters<PostIdInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        value(
            post_svc::set_status(&self.state, req.id, 0)
                .await
                .map_err(err)?,
        )
    }

    // ---------- 评论管理 ----------

    /// 管理端评论列表（含待审核数量），可按状态过滤。
    #[tool(
        name = "admin_list_comments",
        description = "管理端评论列表，可按状态过滤（0=待审核，1=已通过，2=已拒绝），返回待审核总数。",
        annotations(read_only_hint = true)
    )]
    async fn admin_list_comments(
        &self,
        Parameters(req): Parameters<ListCommentsInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        if let Some(status) = req.status {
            if !(0..=2).contains(&status) {
                return Err("评论状态只能是 0（待审核）、1（已通过）或 2（已拒绝）".into());
            }
        }
        let page = req.page.unwrap_or(1).max(1);
        let per_page = req.per_page.unwrap_or(20).clamp(1, 100);
        let (list, total) = comment_svc::admin_list(&self.state, req.status, page, per_page)
            .await
            .map_err(err)?;
        let pending = comment_svc::pending_count(&self.state).await.map_err(err)?;
        value(json!({
            "list": list,
            "total": total,
            "pending": pending,
            "page": page,
            "per_page": per_page
        }))
    }

    /// 通过评论审核。
    #[tool(
        name = "admin_approve_comment",
        description = "通过评论审核，使其出现在文章公开评论区。",
        annotations(idempotent_hint = true)
    )]
    async fn admin_approve_comment(
        &self,
        Parameters(req): Parameters<CommentIdInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        comment_svc::moderate(&self.state, req.id, 1)
            .await
            .map_err(err)?;
        Ok(Json(json!({ "id": req.id, "status": 1 })))
    }

    /// 拒绝评论（软删除）。
    #[tool(
        name = "admin_reject_comment",
        description = "拒绝评论（软删除），该评论不再出现在公开评论区。",
        annotations(destructive_hint = true)
    )]
    async fn admin_reject_comment(
        &self,
        Parameters(req): Parameters<CommentIdInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        comment_svc::moderate(&self.state, req.id, 2)
            .await
            .map_err(err)?;
        Ok(Json(json!({ "id": req.id, "status": 2 })))
    }

    /// 以站长身份回复评论（直接公开）。
    #[tool(
        name = "admin_reply_comment",
        description = "以站长身份回复某条评论，回复直接通过审核并公开显示。"
    )]
    async fn admin_reply_comment(
        &self,
        Parameters(req): Parameters<ReplyCommentInput>,
    ) -> Result<Json<Value>, String> {
        let admin = self.require_admin().await?;
        let email = self.admin_email(admin.id).await;
        let comment = comment_svc::reply_as_admin(
            &self.state,
            req.comment_id,
            &req.content,
            req.author_name.as_deref(),
            Some(&email),
        )
        .await
        .map_err(err)?;
        Ok(Json(json!({
            "id": comment.id,
            "post_id": comment.post_id,
            "parent_id": comment.parent_id,
            "status": comment.status,
            "author_name": comment.author_name,
            "content": comment.content
        })))
    }

    // ---------- 标签管理 ----------

    /// 列出全部标签。
    #[tool(
        name = "admin_list_tags",
        description = "列出全部标签（id、名称、颜色），创建/更新文章前可用它获取 tag_ids。",
        annotations(read_only_hint = true)
    )]
    async fn admin_list_tags(&self) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        value(post_svc::list_all_tags(&self.state).await.map_err(err)?)
    }

    /// 新建标签。
    #[tool(
        name = "admin_create_tag",
        description = "新建标签。name 不可重名，color 默认 t-pink（可选 t-pink / t-mint / t-blue / t-yellow）。"
    )]
    async fn admin_create_tag(
        &self,
        Parameters(req): Parameters<CreateTagInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        let color = req.color.as_deref().unwrap_or("t-pink");
        value(
            post_svc::create_tag(&self.state, &req.name, color)
                .await
                .map_err(err)?,
        )
    }

    /// 修改标签名称或颜色。
    #[tool(
        name = "admin_update_tag",
        description = "修改标签名称或颜色（至少提供一个字段），改名后所有文章上的标签同步更新。",
        annotations(idempotent_hint = true)
    )]
    async fn admin_update_tag(
        &self,
        Parameters(req): Parameters<UpdateTagInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        if req.name.is_none() && req.color.is_none() {
            return Err("至少提供 name 或 color".into());
        }
        value(
            post_svc::update_tag(
                &self.state,
                req.id,
                req.name.as_deref(),
                req.color.as_deref(),
            )
            .await
            .map_err(err)?,
        )
    }

    /// 删除标签（文章上的引用一并移除）。
    #[tool(
        name = "admin_delete_tag",
        description = "删除标签，文章与该标签的关联会一并移除，文章本身不受影响。",
        annotations(destructive_hint = true)
    )]
    async fn admin_delete_tag(
        &self,
        Parameters(req): Parameters<TagIdInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        post_svc::delete_tag(&self.state, req.id)
            .await
            .map_err(err)?;
        Ok(Json(json!({ "deleted": true, "id": req.id })))
    }

    // ---------- 系统 ----------

    /// 站点统计：文章、评论、标签数量与最近发布时间。
    #[tool(
        name = "admin_site_stats",
        description = "站点统计：文章总数/已发布/草稿、评论总数/待审核/已通过/已拒绝、标签数、记录天数、最近发布时间。",
        annotations(read_only_hint = true)
    )]
    async fn admin_site_stats(&self) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        value(system::stats(&self.state).await.map_err(err)?)
    }

    /// 渲染 Markdown 预览（与文章详情同一消毒管线）。
    #[tool(
        name = "preview_markdown",
        description = "把 Markdown 渲染为与文章详情一致的、已消毒的 HTML，用于发布前预览。",
        annotations(read_only_hint = true)
    )]
    async fn preview_markdown(
        &self,
        Parameters(req): Parameters<PreviewMarkdownInput>,
    ) -> Result<Json<Value>, String> {
        self.require_admin().await?;
        Ok(Json(
            json!({ "html": crate::util::render_markdown(&req.md) }),
        ))
    }
}

impl ServerHandler for RibenMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("riben-mcp", env!("CARGO_PKG_VERSION")))
            // 邮箱登录需要会话级状态；2026-07-28 起 MCP 移除了会话，所以刻意把
            // 协商上限固定在仍带 initialize 的最后一个版本，客户端会自动降级。
            .with_protocol_version(ProtocolVersion::V_2025_11_25)
            .with_instructions(INSTRUCTIONS)
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(ProtocolVersion::known_up_to(&ProtocolVersion::V_2025_11_25))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: self.tool_router.list_all(),
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        self.try_header_auth(&context).await;
        tracing::info!(tool = %request.name, "mcp tool call");
        let call = ToolCallContext::new(self, request, context);
        self.tool_router.call(call).await
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tool_router.get(name).cloned()
    }
}

// ---------- 辅助 ----------

fn err(e: AppError) -> String {
    e.to_string()
}

fn login_hint() -> String {
    "未登录：请先调用 auth_send_email_code 获取验证码，再调用 auth_login 登录；\
     或在 MCP 客户端配置请求头 x-admin-token: <ADMIN_API_TOKEN>。"
        .into()
}

fn value<T: Serialize>(data: T) -> Result<Json<Value>, String> {
    serde_json::to_value(data)
        .map(Json)
        .map_err(|e| format!("结果序列化失败: {e}"))
}

fn validate_status(status: i8) -> Result<(), String> {
    if status == 0 || status == 1 {
        Ok(())
    } else {
        Err("文章状态只能是 0（草稿）或 1（已发布）".into())
    }
}

fn validate_post(title: &str, content_md: &str, sticker: Option<&str>) -> Result<(), String> {
    let title_len = title.trim().chars().count();
    if title_len == 0 || title_len > 190 {
        return Err("标题长度需为 1–190 个字符".into());
    }
    let content_len = content_md.chars().count();
    if content_len == 0 || content_len > 100_000 {
        return Err("正文长度需为 1–100000 个字符".into());
    }
    if let Some(sticker) = sticker {
        if sticker.chars().count() > 32 {
            return Err("贴纸文案不能超过 32 个字符".into());
        }
    }
    Ok(())
}

fn tag_catalog(tags: &[crate::models::tag::TagOut]) -> String {
    if tags.is_empty() {
        return "（暂无标签，可先用 admin_create_tag 创建）".into();
    }
    tags.iter()
        .map(|t| format!("{}（id={}）", t.name, t.id))
        .collect::<Vec<_>>()
        .join("、")
}

fn header_admin_token(parts: &axum::http::request::Parts) -> Option<String> {
    if let Some(value) = parts.headers.get("x-admin-token") {
        if let Ok(value) = value.to_str() {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    let value = parts
        .headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let token = value.strip_prefix("Bearer ")?.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> AppState {
        AppState {
            db: sqlx::MySqlPool::connect_lazy("mysql://root@127.0.0.1:3306/riben")
                .expect("lazy pool"),
            redis: None,
            config: crate::config::Config::from_env(),
        }
    }

    #[tokio::test]
    async fn exposes_expected_tools() {
        let server = RibenMcp::new(test_state());
        let names: Vec<String> = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect();
        for expected in [
            "auth_send_email_code",
            "auth_login",
            "auth_logout",
            "auth_whoami",
            "health_check",
            "get_home",
            "list_archive",
            "get_post",
            "list_public_comments",
            "admin_list_posts",
            "admin_get_post",
            "admin_create_post",
            "admin_update_post",
            "admin_delete_post",
            "admin_publish_post",
            "admin_unpublish_post",
            "admin_list_comments",
            "admin_approve_comment",
            "admin_reject_comment",
            "admin_reply_comment",
            "admin_list_tags",
            "admin_create_tag",
            "admin_update_tag",
            "admin_delete_tag",
            "admin_site_stats",
            "preview_markdown",
        ] {
            assert!(
                names.iter().any(|n| n == expected),
                "missing tool {expected}"
            );
        }
        assert_eq!(names.len(), 26, "unexpected tools: {names:?}");
    }
}
