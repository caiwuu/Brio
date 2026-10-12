-- identities 表加 freezes_json 列：记录 web2api sidecar 上报的 (type → unfreeze_at)。
-- web2api 抓到 429 / 限流时会 emit `account_frozen` NDJSON 事件，
-- ApiSupervisor 收到后 upsert 到这一列，UI 据此展示"哪个 type 暂时被冻"。
-- 单一 source of truth：web2api 进程不再持有任何 sqlite，desktop 是唯一存储。
ALTER TABLE identities ADD COLUMN freezes_json TEXT NOT NULL DEFAULT '{}';
