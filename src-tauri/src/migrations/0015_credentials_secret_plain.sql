-- 凭证 secret 改为与元数据同库明文存储（SQLite），不再使用系统 keychain。
-- 已有行 secret 默认为空：若此前 secret 只在 keychain 里，需用户在 UI 里重新填一次 key。

ALTER TABLE credentials ADD COLUMN secret TEXT NOT NULL DEFAULT '';
