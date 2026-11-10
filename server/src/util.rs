//! 纯逻辑工具：slug 生成、摘要提取、Markdown 渲染 + HTML 消毒

use crate::error::{AppError, AppResult};

/// 从标题生成 URL 友好 slug。
/// 中文字符天然没有拉丁字母，纯中文标题会退化为时间戳 slug，保证唯一即可。
pub fn slugify(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    let mut last_dash = false;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

/// 生成全局唯一 slug：标题 slug + 时间片后缀，冲突时递增序号。
pub fn unique_slug_base(title: &str) -> String {
    let base = slugify(title);
    let suffix = format!(
        "-{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    );
    if base.is_empty() {
        format!("page{}", suffix)
    } else {
        format!("{base}{suffix}")
    }
}

/// 保证 slug 唯一（在调用处传入闭包检查存在性）
pub async fn ensure_unique_slug<F, Fut>(base: &str, exists: F) -> AppResult<String>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = AppResult<bool>>,
{
    if !exists(base.to_string()).await? {
        return Ok(base.to_string());
    }
    for i in 2..1000 {
        let candidate = format!("{base}-{i}");
        if !exists(candidate.clone()).await? {
            return Ok(candidate);
        }
    }
    Err(AppError::Internal("无法生成唯一 slug".into()))
}

/// 由 Markdown 正文生成纯文本摘要（去除 Markdown 语法，压缩空白，截断）
pub fn make_excerpt(content_md: &str, max: usize) -> String {
    let mut out = String::with_capacity(max * 2);
    let mut in_code = false;
    for line in content_md.lines() {
        let line = line.trim();
        if line.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        let mut cleaned = String::with_capacity(line.len());
        let mut chars = line.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '#' | '*' | '`' | '>' | '~' | '[' | ']' | '(' | ')' | '!' => {}
                '\\' => {
                    if let Some(&next) = chars.peek() {
                        cleaned.push(next);
                        chars.next();
                    }
                }
                _ => cleaned.push(ch),
            }
        }
        let cleaned = cleaned.trim();
        if !cleaned.is_empty() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(cleaned);
        }
        if out.chars().count() >= max {
            break;
        }
    }
    truncate_chars(&out, max)
}

pub fn truncate_chars(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// Markdown → HTML：comrak(GFM) 渲染 + ammonia 白名单消毒
pub fn render_markdown(md: &str) -> String {
    use comrak::{markdown_to_html, Options};

    let mut opts = Options::default();
    opts.extension.table = true;
    opts.extension.strikethrough = true;
    opts.extension.tasklist = true;
    opts.extension.autolink = true;
    opts.render.github_pre_lang = true;
    opts.render.hardbreaks = true;

    let raw = markdown_to_html(md, &opts);
    sanitize_html(&raw)
}

/// HTML 消毒：白名单标签 + 属性，防 XSS（见计划 §12）
pub fn sanitize_html(raw: &str) -> String {
    ammonia::Builder::default()
        .add_tags(["img"])
        .add_tag_attributes("img", ["src", "alt", "title"])
        .add_tag_attributes("a", ["href", "title"])
        .add_tag_attributes("code", ["class"])
        .url_schemes(["http", "https", "mailto"].into_iter().collect())
        .link_rel(Some("noopener noreferrer"))
        .clean(raw)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_ascii() {
        assert_eq!(slugify("Hello, Rust World!"), "hello-rust-world");
        assert_eq!(slugify("  --a--  b  "), "a-b");
    }

    #[test]
    fn slugify_chinese_falls_back_to_empty() {
        assert_eq!(slugify("初雪比往年早了一些"), "");
    }

    #[test]
    fn unique_slug_base_non_empty() {
        assert!(!unique_slug_base("初雪比往年早了一些").is_empty());
        assert!(unique_slug_base("初雪比往年早了一些").starts_with("page-"));
        assert!(unique_slug_base("First Snow").starts_with("first-snow-"));
    }

    #[test]
    fn excerpt_strips_markdown() {
        let md = "# 标题\n\n这是 **正文**，带 [链接](https://x.com)。\n\n```\ncode 不应出现\n```\n";
        let ex = make_excerpt(md, 50);
        assert!(!ex.contains("code"));
        assert!(!ex.contains("**"));
        assert!(!ex.contains('#'));
        assert!(ex.contains("正文"));
    }

    #[test]
    fn excerpt_truncates() {
        let long = "字".repeat(100);
        let ex = make_excerpt(&long, 20);
        assert_eq!(ex.chars().count(), 21); // 20 + …
    }

    #[test]
    fn render_escapes_script() {
        let html = render_markdown("<script>alert(1)</script>\n\n[b](javascript:evil)");
        assert!(!html.contains("<script"));
        assert!(!html.contains("javascript:"));
        assert!(html.contains("<p>"));
    }

    #[test]
    fn render_allows_basic_markup() {
        let html = render_markdown("# 你好\n\n- a\n- b\n\n| x |\n|---|\n| 1 |");
        assert!(html.contains("<h1"));
        assert!(html.contains("<ul"));
        assert!(html.contains("<table"));
    }
}
