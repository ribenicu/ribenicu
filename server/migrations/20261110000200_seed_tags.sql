-- 种子标签：沿用现有 tag 色系（t-pink / t-mint / t-blue / t-yellow）
INSERT INTO tags (name, color)
VALUES ('日常', 't-pink'), ('生活改造', 't-mint'), ('食物', 't-blue'), ('淘货', 't-yellow'),
       ('动物', 't-pink'), ('风景', 't-blue'), ('城市', 't-blue'), ('手账', 't-yellow')
ON DUPLICATE KEY UPDATE color = VALUES(color);
