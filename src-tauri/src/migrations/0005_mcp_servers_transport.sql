-- MCP server 支持远程 transport（http / SSE）。
--
-- transport='stdio'  → 使用 command / args / env（旧字段）
-- transport='remote' → 使用 url / headers_json，command/args/env 留空
--
-- MCP spec 里 SSE 已并入 streamable-http transport，UI 上把它们合并成
-- 「远程 (http/sse)」一类。
ALTER TABLE mcp_servers ADD COLUMN transport    TEXT NOT NULL DEFAULT 'stdio';
ALTER TABLE mcp_servers ADD COLUMN url          TEXT NOT NULL DEFAULT '';
ALTER TABLE mcp_servers ADD COLUMN headers_json TEXT NOT NULL DEFAULT '{}';
