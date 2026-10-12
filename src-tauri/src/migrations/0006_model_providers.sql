-- M4 后续：把"对外暴露的模型服务"独立成 `model_providers`。
--
-- 与 `providers`（identity 类型 = 上游站点 plugin）完全无关，互不引用。
-- web2api 启动时根据这张表动态注册路由：
--   protocol='openai'    → POST {base_path}/{provider_id}/v1/chat/completions
--   protocol='anthropic' → POST {base_path}/{provider_id}/v1/messages   (M0 仅占位)
--
-- models_json 形如：
--   [{"name": "s4.6", "plugin_id": "claude", "upstream_model": "claude-sonnet-4.6"}, ...]
-- 用户对外报告 model="s4.6" 时，web2api 会查到 plugin "claude" + 实际 upstream model 名。
--
-- auth_token：M0 只存不校验；后续接入 Bearer 校验时直接读这个字段。
CREATE TABLE IF NOT EXISTS model_providers (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    protocol    TEXT NOT NULL DEFAULT 'openai',   -- 'openai' | 'anthropic'
    auth_token  TEXT NOT NULL DEFAULT '',
    base_path   TEXT NOT NULL DEFAULT '',         -- 留空走默认 /<protocol>/<id>/v1
    models_json TEXT NOT NULL DEFAULT '[]',
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_model_providers_proto
    ON model_providers(protocol);
