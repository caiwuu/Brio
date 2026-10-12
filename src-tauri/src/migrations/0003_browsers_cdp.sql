-- 让手动启动的 browser 默认带 CDP 端口，方便 web2api 通过 connect_over_cdp 接入。
-- source 字段用于区分用户手动建的 browser 与从 identity 临时还原的 ephemeral browser。
ALTER TABLE browsers ADD COLUMN cdp_port      INTEGER NOT NULL DEFAULT 0;
ALTER TABLE browsers ADD COLUMN source        TEXT    NOT NULL DEFAULT 'manual'; -- manual | identity
ALTER TABLE browsers ADD COLUMN source_ref    TEXT    NOT NULL DEFAULT '';       -- 对应 identity_id（source='identity' 时）
