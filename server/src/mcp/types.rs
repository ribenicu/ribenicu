//! MCP 工具的输入参数定义。
//!
//! 每个结构体会由 rmcp/schemars 生成 JSON Schema，字段注释即工具参数说明，
//! 应保持简洁、面向调用方（LLM / Agent）。

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EmailInput {
    /// 管理员邮箱（收到 6 位登录验证码的邮箱）
    pub email: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LoginInput {
    /// 管理员邮箱
    pub email: String,
    /// 邮件中的 6 位数字验证码
    pub code: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SlugInput {
    /// 文章 slug（创建文章时由服务端生成，形如 `page-18f3a2b`）
    pub slug: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListAdminPostsInput {
    /// 可选状态过滤：0=草稿，1=已发布；省略则返回全部
    pub status: Option<i8>,
    /// 页码，从 1 开始，默认 1
    pub page: Option<u32>,
    /// 每页条数，默认 20，最大 100
    pub per_page: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PostIdInput {
    /// 文章 ID
    pub id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreatePostInput {
    /// 标题，1–190 个字符
    pub title: String,
    /// Markdown 正文，1–100000 个字符
    pub content_md: String,
    /// 手账贴纸文案（emoji 或短文本），最多 32 个字符
    pub sticker: Option<String>,
    /// 状态：0=草稿（默认），1=立即发布
    pub status: Option<i8>,
    /// 标签 ID 列表；可先用 admin_list_tags 获取
    pub tag_ids: Option<Vec<u64>>,
    /// 标签名列表（与 tag_ids 二选一或并用；名称必须已存在）
    pub tag_names: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdatePostInput {
    /// 文章 ID
    pub id: u64,
    /// 新标题；省略则保持不变
    pub title: Option<String>,
    /// 新 Markdown 正文；省略则保持不变
    pub content_md: Option<String>,
    /// 新手账贴纸文案；省略则保持不变，传空字符串表示清除
    pub sticker: Option<String>,
    /// 新状态：0=草稿，1=已发布；省略则保持不变
    pub status: Option<i8>,
    /// 新标签 ID 列表（整体替换）；省略则保持不变，传空数组表示清空
    pub tag_ids: Option<Vec<u64>>,
    /// 新标签名列表（整体替换，与 tag_ids 并用时取并集）；名称必须已存在
    pub tag_names: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListCommentsInput {
    /// 可选状态过滤：0=待审核，1=已通过，2=已拒绝；省略则返回全部
    pub status: Option<i8>,
    /// 页码，从 1 开始，默认 1
    pub page: Option<u32>,
    /// 每页条数，默认 20，最大 100
    pub per_page: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CommentIdInput {
    /// 评论 ID
    pub id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplyCommentInput {
    /// 要回复的评论 ID
    pub comment_id: u64,
    /// 回复内容（纯文本，1–2000 个字符）
    pub content: String,
    /// 回复显示的昵称，默认「站长」
    pub author_name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateTagInput {
    /// 标签名，1–64 个字符，不可重名
    pub name: String,
    /// 颜色样式，默认 `t-pink`（可选 t-pink / t-mint / t-blue / t-yellow）
    pub color: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TagIdInput {
    /// 标签 ID
    pub id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateTagInput {
    /// 标签 ID
    pub id: u64,
    /// 新名称；省略则保持不变
    pub name: Option<String>,
    /// 新颜色样式；省略则保持不变
    pub color: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PreviewMarkdownInput {
    /// 待渲染的 Markdown 源码
    pub md: String,
}
