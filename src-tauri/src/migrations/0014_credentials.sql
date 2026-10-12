-- 凭证 (credential)：供 model_provider / chat / 后续功能公用的一个间接层。
--
-- DB 只存元数据：id / label / kind / scope_hint。真正的 secret 写进操作系统
-- keychain（macOS Keychain, Windows Cred Manager, Linux Secret Service），
-- 位置由 Rust 侧 commands/credentials.rs 封装：service="com.brio.desktop.credentials",
-- account=id。preview 是可选的脱敏摘要（比如 "sk-…abcd"），列表页能给用户
-- 看到"是哪个 key"而不必 reveal 真值。
--
-- 迁移策略：
--   - model_providers / chats 增加 credential_id，旧的 api_key 字段保留。
--   - 解析 key 时优先走 credential_id（查 keychain），回退到 api_key 字段。
--   - 前端新建 provider 时，如果填了 api_key 字符串，会被 Rust 侧"自动
--     搬"到一个新 credential，并把 credential_id 绑上；老 api_key 字段
--     再后续迁移里会被清零/删除。

CREATE TABLE IF NOT EXISTS credentials (
    id          TEXT PRIMARY KEY,
    label       TEXT NOT NULL DEFAULT '',
    kind        TEXT NOT NULL DEFAULT 'bearer',  -- 'bearer' | 'oauth' | 'custom'
    scope_hint  TEXT NOT NULL DEFAULT '',        -- 可选：用户写的"干啥用的"说明
    preview     TEXT NOT NULL DEFAULT '',        -- 脱敏摘要，如 "sk-…abcd"
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

ALTER TABLE model_providers ADD COLUMN credential_id TEXT;
ALTER TABLE chats           ADD COLUMN credential_id TEXT;
