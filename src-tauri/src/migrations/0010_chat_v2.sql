-- Agent v2：chat 表族
--
-- 与老的 agents / agent_runs 并存；M4 清理老表时再 ALTER/DROP。
-- chat_events = NDJSON envelope 的单一真相源（主键 (chat_id, seq) 保证严格单调）。

CREATE TABLE IF NOT EXISTS chats (
    id                TEXT PRIMARY KEY,               -- ULID
    project_id        TEXT,                           -- 未来关联 projects 表；M1 留 NULL
    title             TEXT NOT NULL DEFAULT '',
    workspace_dir     TEXT NOT NULL,
    model             TEXT NOT NULL,
    mode              TEXT NOT NULL DEFAULT 'default', -- default | acceptEdits | bypass | plan
    effort            TEXT,                           -- low | medium | high | max | NULL
    api_base          TEXT NOT NULL,                  -- provider endpoint
    api_key           TEXT NOT NULL DEFAULT '',       -- 明文（M1 简化；M3 换凭证 ref）
    model_provider_id TEXT,                           -- 可选外键到 model_providers
    mcp_config_path   TEXT,
    system_prompt     TEXT,
    max_turns         INTEGER NOT NULL DEFAULT 20,
    status            TEXT NOT NULL DEFAULT 'idle',   -- idle | running | closed | error
    branch_at_start   TEXT,                           -- git rev-parse --short HEAD at creation
    last_event_seq    INTEGER NOT NULL DEFAULT 0,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    FOREIGN KEY (model_provider_id) REFERENCES model_providers(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_chats_project ON chats(project_id);
CREATE INDEX IF NOT EXISTS idx_chats_updated ON chats(updated_at);

-- 每一行对应 sidecar 发出的一个 envelope；前端可通过 chat_events(id, since_seq) 重放。
CREATE TABLE IF NOT EXISTS chat_events (
    chat_id     TEXT NOT NULL,
    seq         INTEGER NOT NULL,
    ts          INTEGER NOT NULL,                     -- epoch ms
    type        TEXT NOT NULL,                        -- 见 docs/AGENT_V2_RFC.md §6
    payload     TEXT NOT NULL,                        -- JSON 字符串
    PRIMARY KEY (chat_id, seq),
    FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_chat_events_chat_ts ON chat_events(chat_id, ts);

-- 每一轮对话的聚合行（便于列表/统计，不用 scan chat_events）
CREATE TABLE IF NOT EXISTS chat_turns (
    chat_id            TEXT NOT NULL,
    turn_id            TEXT NOT NULL,
    turn_index         INTEGER NOT NULL,
    started_at         INTEGER NOT NULL,
    ended_at           INTEGER,
    had_tool_calls     INTEGER NOT NULL DEFAULT 0,
    prompt_tokens      INTEGER,
    completion_tokens  INTEGER,
    total_tokens       INTEGER,
    PRIMARY KEY (chat_id, turn_id),
    FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_chat_turns_chat_idx ON chat_turns(chat_id, turn_index);
