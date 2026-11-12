-- 为用户表添加邮箱字段（邮箱验证码登录）

ALTER TABLE users
    ADD COLUMN email VARCHAR(190) NULL AFTER username;

CREATE UNIQUE INDEX uq_users_email ON users (email);
