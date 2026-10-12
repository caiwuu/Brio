-- M1 数据层初始化：6 张表
-- 所有时间戳采用 UNIX 秒（INTEGER）

CREATE TABLE IF NOT EXISTS browsers (
    id              TEXT PRIMARY KEY,            -- ULID
    name            TEXT NOT NULL,
    proxy_url       TEXT NOT NULL DEFAULT '',
    timezone        TEXT NOT NULL DEFAULT '',
    language        TEXT NOT NULL DEFAULT 'en-US',
    fingerprint_id  TEXT NOT NULL,
    notes           TEXT NOT NULL DEFAULT '',
    user_data_dir   TEXT NOT NULL DEFAULT '',    -- 由 Rust 在创建时填充
    last_opened_at  INTEGER,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_browsers_fp ON browsers(fingerprint_id);

CREATE TABLE IF NOT EXISTS proxies (
    id              TEXT PRIMARY KEY,            -- ULID
    label           TEXT NOT NULL,
    url             TEXT NOT NULL,               -- socks5://user:pass@host:port
    region          TEXT NOT NULL DEFAULT '',
    timezone        TEXT NOT NULL DEFAULT '',
    notes           TEXT NOT NULL DEFAULT '',
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS providers (
    id              TEXT PRIMARY KEY,            -- e.g. 'claude' | 'chatgpt' | 'deepseek'
    label           TEXT NOT NULL,
    base_url        TEXT NOT NULL,
    enabled         INTEGER NOT NULL DEFAULT 1,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS identities (
    id              TEXT PRIMARY KEY,            -- ULID
    browser_id      TEXT,                        -- nullable，删除浏览器后保留 identity
    provider_id     TEXT NOT NULL,
    label           TEXT NOT NULL,
    fingerprint_id  TEXT NOT NULL,
    proxy_url       TEXT NOT NULL DEFAULT '',
    timezone        TEXT NOT NULL DEFAULT '',
    language        TEXT NOT NULL DEFAULT '',
    identity_dir    TEXT NOT NULL,
    zip_path        TEXT,
    status          TEXT NOT NULL DEFAULT 'ready', -- ready | stale | broken
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    FOREIGN KEY (browser_id)  REFERENCES browsers(id)  ON DELETE SET NULL,
    FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_identities_browser  ON identities(browser_id);
CREATE INDEX IF NOT EXISTS idx_identities_provider ON identities(provider_id);

CREATE TABLE IF NOT EXISTS agents (
    id              TEXT PRIMARY KEY,            -- ULID
    name            TEXT NOT NULL,
    workspace_dir   TEXT NOT NULL,
    identity_id     TEXT,                        -- 默认绑定的 identity
    permission_tier TEXT NOT NULL DEFAULT 'default', -- default | full
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    FOREIGN KEY (identity_id) REFERENCES identities(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS agent_runs (
    id              TEXT PRIMARY KEY,            -- ULID
    agent_id        TEXT NOT NULL,
    title           TEXT NOT NULL DEFAULT '',
    status          TEXT NOT NULL DEFAULT 'idle', -- idle | running | done | error
    started_at      INTEGER NOT NULL,
    ended_at        INTEGER,
    transcript_path TEXT,
    FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_agent_runs_agent ON agent_runs(agent_id);
