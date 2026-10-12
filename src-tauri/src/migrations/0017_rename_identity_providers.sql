-- providers 这张表的真实语义是"identity 站点 / web2api plugin 目录"
-- （claude.ai / chatgpt.com / deepseek），与 model_providers（LLM API 端点）
-- 完全不是一个概念。统一前缀为 identity_providers 让两者并列、自解释。
--
-- SQLite >= 3.25 的 ALTER TABLE RENAME 会同步更新外键引用，
-- identities.provider_id 的 FK 会自动指向新表。
-- 列名故意保留 provider_id，不再做 column rename，以避开 SQLite 重建表的代价；
-- "identity 表的 provider_id 指向 identity_providers" 读起来也通。
ALTER TABLE providers RENAME TO identity_providers;
