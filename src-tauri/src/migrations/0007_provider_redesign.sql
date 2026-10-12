-- 重新分层：
--
-- 1. providers（= identity type / web2api plugin）补齐 model_mapping + payload，
--    对齐 web2api config.local.yaml 里的 `claude: / chatgpt: / deepseek:` 段。
--
-- 2. model_providers（= agent 调用的上游 LLM 端点配置）schema 大改：
--      auth_token  → api_key
--      base_path   → base_url   （含义从相对路径 → 完整 URL）
--      models_json 形态从 [{name, plugin_id, upstream_model}] → ["s4.6", ...]
--    因为旧表是空的（M4 才加，用户尚未录入数据），直接 DROP + CREATE。
--    后续 agents 表才会引用它，目前没有外键依赖。

ALTER TABLE providers ADD COLUMN model_mapping_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE providers ADD COLUMN payload_json       TEXT NOT NULL DEFAULT '{}';

DROP TABLE IF EXISTS model_providers;

CREATE TABLE model_providers (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    protocol    TEXT NOT NULL DEFAULT 'openai',   -- 'openai' | 'anthropic'
    base_url    TEXT NOT NULL DEFAULT '',         -- 完整 URL，如 http://127.0.0.1:9000/openai/claude/v1
    api_key     TEXT NOT NULL DEFAULT '',         -- 调上游时带的 Bearer token
    models_json TEXT NOT NULL DEFAULT '[]',       -- 字符串数组，如 ["s4.6", "s4"]
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_model_providers_proto
    ON model_providers(protocol);
