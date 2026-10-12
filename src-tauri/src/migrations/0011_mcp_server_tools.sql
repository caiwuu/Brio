-- 缓存 MCP server 探测出来的工具列表 + 每工具的启用开关。
--
-- 用途：
--   1) agent 启动时读该表生成可用 tool 白名单（未启用的 tool 不会暴露给 LLM）
--   2) UI 在列表 / 详情里显示"上次探测看到的工具"，不必每次打开页面都 probe
--
-- 数据流：
--   - probe_mcp_server_by_id 成功 → 后端 upsert 到这里，保留已存在行的 enabled
--   - UI 侧调 mcp_server_set_tool_enabled 切开关
--   - 删掉 server 时 ON DELETE CASCADE 一并清掉 tools

CREATE TABLE IF NOT EXISTS mcp_server_tools (
    server_id    TEXT    NOT NULL,
    name         TEXT    NOT NULL,
    description  TEXT    NOT NULL DEFAULT '',
    enabled      INTEGER NOT NULL DEFAULT 1,     -- 0/1
    last_seen_at INTEGER NOT NULL,               -- 最后一次 probe 看到这个 tool 的时间
    PRIMARY KEY (server_id, name),
    FOREIGN KEY (server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_mcp_server_tools_server ON mcp_server_tools(server_id);
