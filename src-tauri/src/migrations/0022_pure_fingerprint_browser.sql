-- 爆改为纯指纹浏览器：删除 identity / agent / web2api / mcp / skills / credentials
-- 相关的所有表；browsers 只保留浏览器 profile 本身需要的列。
--
-- DROP 顺序按外键依赖：先子表后父表（foreign_keys=ON 时删被引用表会失败）。

DROP TABLE IF EXISTS identities;
DROP TABLE IF EXISTS identity_providers;
DROP TABLE IF EXISTS mcp_server_tools;
DROP TABLE IF EXISTS mcp_servers;
DROP TABLE IF EXISTS chat_events;
DROP TABLE IF EXISTS chat_turns;
DROP TABLE IF EXISTS chats;
DROP TABLE IF EXISTS model_providers;
DROP TABLE IF EXISTS skills;
DROP TABLE IF EXISTS credentials;

-- web2api 遗留的 ephemeral browser 行没有存在意义，连行带列一起清掉。
-- （这三列上没有索引/触发器，SQLite 3.35+ 支持 DROP COLUMN。）
DELETE FROM browsers WHERE is_ephemeral = 1;
ALTER TABLE browsers DROP COLUMN is_ephemeral;
ALTER TABLE browsers DROP COLUMN source;
ALTER TABLE browsers DROP COLUMN source_ref;
