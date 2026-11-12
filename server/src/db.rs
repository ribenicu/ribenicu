//! 数据库连接池初始化 + migration + 种子数据

use crate::config::Config;
use crate::error::{AppError, AppResult};
use sqlx::MySqlPool;

pub async fn connect_pool(cfg: &Config) -> AppResult<MySqlPool> {
    // 先尝试真实连接（带超时），失败则退化为 lazy 池：
    // 服务可以启动并给出 /api/health 诊断，待数据库就绪后自然恢复。
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        MySqlPool::connect(&cfg.database_url),
    )
    .await
    {
        Ok(Ok(pool)) => {
            tracing::info!("mysql connected");
            Ok(pool)
        }
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "mysql not ready, falling back to lazy pool");
            Ok(MySqlPool::connect_lazy(&cfg.database_url)?)
        }
        Err(_) => {
            tracing::warn!("mysql connect timeout, falling back to lazy pool");
            Ok(MySqlPool::connect_lazy(&cfg.database_url)?)
        }
    }
}

/// 应用启动时执行 migration（sqlx::migrate! 内嵌 migrations/ 目录）
pub async fn run_migrations(pool: &MySqlPool) -> AppResult<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| AppError::Internal(format!("migration failed: {e}")))?;
    tracing::info!("migrations applied");
    Ok(())
}

/// 首次启动且 users 表为空时，用环境变量播种管理员。
/// password_hash 保留列（兼容旧数据），但登录已改为邮箱验证码，此处填充随机哈希占位。
pub async fn seed_admin(pool: &MySqlPool, cfg: &Config) -> AppResult<()> {
    let (username, email) = match (&cfg.admin_username, &cfg.admin_email) {
        (Some(u), Some(e)) => (u.clone(), e.clone()),
        _ => return Ok(()),
    };
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }
    let dummy_password = uuid::Uuid::new_v4().to_string();
    let salt = argon2::password_hash::SaltString::generate(argon2::password_hash::rand_core::OsRng);
    use argon2::password_hash::PasswordHasher;
    let hash = argon2::Argon2::default()
        .hash_password(dummy_password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("hash password failed: {e}")))?
        .to_string();
    sqlx::query("INSERT INTO users (username, email, password_hash) VALUES (?, ?, ?)")
        .bind(&username)
        .bind(&email)
        .bind(&hash)
        .execute(pool)
        .await?;
    tracing::info!(username = %username, "admin user seeded");
    Ok(())
}

/// 已有管理员但 email 为空时，用 ADMIN_EMAIL 回填（用于从旧版密码登录迁移）
pub async fn backfill_admin_email(pool: &MySqlPool, cfg: &Config) -> AppResult<()> {
    let Some(email) = &cfg.admin_email else {
        return Ok(());
    };
    let result = sqlx::query("UPDATE users SET email = ? WHERE email IS NULL")
        .bind(email)
        .execute(pool)
        .await?;
    if result.rows_affected() > 0 {
        tracing::info!(email = %email, "admin email backfilled");
    }
    Ok(())
}

/// 首次启动且 posts 表为空时，写入原静态站的 6 篇演示文章（SEED_DEMO=false 可关闭）
pub async fn seed_demo_posts(pool: &MySqlPool, cfg: &Config) -> AppResult<()> {
    if !cfg.seed_demo {
        return Ok(());
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM posts")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }

    // (title, sticker, tags, date, markdown)
    let demo: [(&str, &str, &[&str], &str, &str); 6] = [
        (
            "初雪比往年早了一些",
            "下雪啦！",
            &["日常", "城市"],
            "2026-11-08 09:30:00",
            "早上拉开窗帘的时候愣了三秒钟——屋顶已经白了。明明气象台说周末才下的。\n\n楼下的小孩比大人先知道这个消息。尖叫声穿透双层玻璃，比任何闹钟都管用。有个穿恐龙睡衣的小朋友直接在雪地里躺出了一个大字，被妈妈拎着帽子拽了回去。\n\n我泡了杯热茶，搬椅子坐到窗边。雪落下来的时候没有声音，整座城市好像被调成了静音模式。路上行人走路都轻了，连汽车按喇叭都变得犹豫。\n\n这样的天气适合原谅一切。适合把去年没回的消息回了，把搁置的手账补了，把「改天再说」改成「就今天吧」。\n\n初雪快乐。❄️",
        ),
        (
            "阳台变身小书房",
            "改造完成",
            &["生活改造"],
            "2026-11-03 20:00:00",
            "花整个周末清走杂物，摆上小木桌、暖光灯和一盆绿萝。\n\n傍晚坐在这里写字，风从窗缝进来，租房也有了家的形状～\n\n清单：小木桌一张、暖光台灯一盏、绿萝一盆、靠垫两个。预算三百，全部拿下。",
        ),
        (
            "失败的番茄面也香",
            "光盘行动",
            &["食物"],
            "2026-10-26 19:15:00",
            "面糊了、蛋散了、番茄出了水，但我一个人唏哩呼噜全吃完。\n\n这是本月最踏实的一顿饭，失败品也值得贴进手账！\n\n复盘：火太大、水太少、心太急。下次一定。",
        ),
        (
            "在旧书店捡到宝",
            "今日份快乐",
            &["淘货"],
            "2026-10-19 16:40:00",
            "街角旧书店淘到一本九十年代的菜谱，里面还夹着前任主人的手写便签。\n\n跨越三十年的笔迹，和我看的是同一道菜。\n\n老板说这本五块钱。我说这是无价之宝，然后还是付了五块。",
        ),
        (
            "楼下的猫又胖了一圈",
            "猫猫观察",
            &["动物"],
            "2026-10-12 12:00:00",
            "橘座今天占据了快递柜顶，居高临下地审视每一个取件人。\n\n据我观察，它的腰围和快递数量成正比。\n\n已投喂，无悔改迹象。",
        ),
        (
            "今天的晚霞是橘子味",
            "晚霞预警",
            &["风景"],
            "2026-10-05 18:20:00",
            "下班路上一抬头，整片天都是橘子汽水色。\n\n站在路口拍了五分钟，错过一班公交，但是值得。\n\n手机拍不出十分之一，有些东西只能现场领取。",
        ),
    ];

    for (title, sticker, tags, published_at, md) in demo {
        let base = crate::util::unique_slug_base(title);
        let slug = crate::util::ensure_unique_slug(&base, |s| async move {
            let exists: Option<u64> = sqlx::query_scalar("SELECT id FROM posts WHERE slug = ?")
                .bind(&s)
                .fetch_optional(pool)
                .await?;
            Ok(exists.is_some())
        })
        .await?;
        let excerpt = crate::util::make_excerpt(md, 120);

        let result = sqlx::query(
            "INSERT INTO posts (slug, title, content_md, excerpt, sticker, status, published_at) \
             VALUES (?, ?, ?, ?, ?, 1, ?)",
        )
        .bind(&slug)
        .bind(title)
        .bind(md)
        .bind(&excerpt)
        .bind(sticker)
        .bind(published_at)
        .execute(pool)
        .await?;
        let post_id = result.last_insert_id();

        for tag in tags {
            let tag_id: Option<u64> = sqlx::query_scalar("SELECT id FROM tags WHERE name = ?")
                .bind(tag)
                .fetch_optional(pool)
                .await?;
            let tag_id = match tag_id {
                Some(id) => id,
                None => {
                    sqlx::query("INSERT INTO tags (name, color) VALUES (?, 't-pink')")
                        .bind(tag)
                        .execute(pool)
                        .await?;
                    sqlx::query_scalar("SELECT id FROM tags WHERE name = ?")
                        .bind(tag)
                        .fetch_one(pool)
                        .await?
                }
            };
            sqlx::query("INSERT IGNORE INTO post_tags (post_id, tag_id) VALUES (?, ?)")
                .bind(post_id)
                .bind(tag_id)
                .execute(pool)
                .await?;
        }
    }
    tracing::info!("demo posts seeded");
    Ok(())
}
