-- "Skill" = 用户或系统预置的任务模板，可以被 $xxx 唤起。
--
-- 字段说明：
--   slug        : composer 里 $xxx 的那个 xxx；全局唯一
--   keywords    : JSON string[] — composer 输入时模糊匹配
--   variables   : JSON [{key, value}] — 当前版本简单作为记录；后续可以作为 env 注入
--   source      : 'user' | 'system' | 'shared'；'system' 的记录一般不可删
--   run_count   : 单纯统计用
--   last_run_at : 排序 "最近使用" 用

CREATE TABLE IF NOT EXISTS skills (
    id           TEXT PRIMARY KEY,
    name         TEXT    NOT NULL,
    slug         TEXT    NOT NULL UNIQUE,
    summary      TEXT    NOT NULL DEFAULT '',
    description  TEXT    NOT NULL DEFAULT '',
    tag          TEXT    NOT NULL DEFAULT 'data',
    keywords     TEXT    NOT NULL DEFAULT '[]',
    enabled      INTEGER NOT NULL DEFAULT 1,
    instructions TEXT    NOT NULL DEFAULT '',
    variables    TEXT    NOT NULL DEFAULT '[]',
    source       TEXT    NOT NULL DEFAULT 'user',
    run_count    INTEGER NOT NULL DEFAULT 0,
    last_run_at  INTEGER,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_skills_slug ON skills(slug);
CREATE INDEX IF NOT EXISTS idx_skills_tag ON skills(tag);
