-- M? Identity 多 type 改造：一份 zip = 一份 identity，可同时支持多个 provider。
--
-- 老 schema：identities.provider_id 单选 + 同 fingerprint 多行（chatgpt 一行、deepseek 一行……）
-- 新 schema：identities.providers_json JSON 数组（["chatgpt","deepseek"]），每份 zip 一行。
--
-- 改造范围：
--   - 删除 idx_identities_provider 索引（列要被删）
--   - DROP TABLE identities + CREATE TABLE 重建（用户已确认丢历史）
--   - 重建时同时把 0018 的 freezes_json 列直接含进去，避免迁移之间互相依赖
--   - browser_id FK 保留（ON DELETE SET NULL），provider_id FK 整列删除
--
-- 这里关闭 FK 校验是为了让 DROP TABLE identities 时不被 agents.identity_id 外键拦下；
-- 重建表后会议会话级 PRAGMA 自动恢复（连接关闭即重置；db.rs 在每个新连接打开时重新 ON）。

PRAGMA foreign_keys = OFF;

DROP INDEX IF EXISTS idx_identities_provider;
DROP TABLE IF EXISTS identities;

CREATE TABLE identities (
    id              TEXT PRIMARY KEY,
    browser_id      TEXT,
    providers_json  TEXT NOT NULL DEFAULT '[]',   -- e.g. ["chatgpt","deepseek"]
    label           TEXT NOT NULL,
    fingerprint_id  TEXT NOT NULL,
    proxy_url       TEXT NOT NULL DEFAULT '',
    timezone        TEXT NOT NULL DEFAULT '',
    language        TEXT NOT NULL DEFAULT '',
    identity_dir    TEXT NOT NULL,
    zip_path        TEXT,
    status          TEXT NOT NULL DEFAULT 'ready',
    freezes_json    TEXT NOT NULL DEFAULT '{}',   -- {type: unfreeze_at_unix_seconds}
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    FOREIGN KEY (browser_id) REFERENCES browsers(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_identities_browser     ON identities(browser_id);
CREATE INDEX IF NOT EXISTS idx_identities_fingerprint ON identities(fingerprint_id);

PRAGMA foreign_keys = ON;
