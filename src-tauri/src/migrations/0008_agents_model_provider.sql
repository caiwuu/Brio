-- agent 真消费 Model Provider：
--   model_provider_id → model_providers.id（agent 默认绑定的 provider）
--   model             → 在该 provider.models 里选一个；为空时 sidecar 报错
--
-- 旧字段 identity_id 保留（agent 仍可绑 identity 决定用哪个浏览器身份），
-- 但模型走向跟它彻底解耦，不再走 identity → provider_id (plugin) → 本地 web2api
-- 这条隐式链路。
ALTER TABLE agents ADD COLUMN model_provider_id TEXT REFERENCES model_providers(id) ON DELETE SET NULL;
ALTER TABLE agents ADD COLUMN model             TEXT NOT NULL DEFAULT '';
