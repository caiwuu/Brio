-- MCP server 配置：每条记录 = 一个可挂载到 agent run 的 MCP server。
--
-- 字段对齐 Anthropic / Codex 通用的 stdio MCP server 描述：
--   command + args + env
-- 启用项（enabled=1）会被 agent run 时打包注入。
-- 暂不支持 SSE / WebSocket transport（M4 主体只覆盖最常见的 stdio）。
CREATE TABLE mcp_servers (
    id          TEXT PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE,
    description TEXT    NOT NULL DEFAULT '',
    command     TEXT    NOT NULL,
    args_json   TEXT    NOT NULL DEFAULT '[]',  -- JSON: string[]
    env_json    TEXT    NOT NULL DEFAULT '{}',  -- JSON: Record<string, string>
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE INDEX idx_mcp_servers_enabled ON mcp_servers(enabled);
