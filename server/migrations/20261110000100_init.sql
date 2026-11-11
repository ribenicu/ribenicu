-- 一本日记 · 初始表结构（MySQL 8, utf8mb4）
-- 对应 docs/development-plan.md §4.1

CREATE TABLE IF NOT EXISTS posts (
    id            BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    slug          VARCHAR(190)    NOT NULL,
    title         VARCHAR(190)    NOT NULL,
    content_md    MEDIUMTEXT      NOT NULL,
    excerpt       VARCHAR(500)    NULL,
    sticker       VARCHAR(32)     NULL COMMENT '卡片贴纸文案，可空',
    status        TINYINT         NOT NULL DEFAULT 0 COMMENT '0=草稿 1=已发布',
    published_at  DATETIME        NULL,
    created_at    DATETIME        NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at    DATETIME        NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uq_posts_slug (slug),
    KEY idx_posts_status_published (status, published_at DESC)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS tags (
    id    BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    name  VARCHAR(64)     NOT NULL,
    color VARCHAR(16)     NOT NULL DEFAULT 't-pink' COMMENT 't-pink / t-mint / t-blue / t-yellow',
    UNIQUE KEY uq_tags_name (name)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS post_tags (
    post_id BIGINT UNSIGNED NOT NULL,
    tag_id  BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (post_id, tag_id),
    KEY idx_post_tags_tag (tag_id, post_id),
    CONSTRAINT fk_post_tags_post FOREIGN KEY (post_id) REFERENCES posts (id) ON DELETE CASCADE,
    CONSTRAINT fk_post_tags_tag  FOREIGN KEY (tag_id)  REFERENCES tags  (id) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS comments (
    id           BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    post_id      BIGINT UNSIGNED NOT NULL,
    parent_id    BIGINT UNSIGNED NULL,
    author_name  VARCHAR(64)     NOT NULL,
    author_email VARCHAR(190)    NOT NULL,
    author_url   VARCHAR(190)    NULL,
    content      TEXT            NOT NULL,
    status       TINYINT         NOT NULL DEFAULT 0 COMMENT '0=待审核 1=已通过 2=已拒绝/删除',
    created_at   DATETIME        NOT NULL DEFAULT CURRENT_TIMESTAMP,
    KEY idx_comments_post (post_id, status, created_at),
    CONSTRAINT fk_comments_post   FOREIGN KEY (post_id)   REFERENCES posts (id)    ON DELETE CASCADE,
    CONSTRAINT fk_comments_parent FOREIGN KEY (parent_id) REFERENCES comments (id) ON DELETE SET NULL
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS users (
    id            BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    username      VARCHAR(64)     NOT NULL,
    password_hash VARCHAR(190)    NOT NULL,
    created_at    DATETIME        NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uq_users_username (username)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_unicode_ci;
