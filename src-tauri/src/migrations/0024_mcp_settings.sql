-- 重建通用应用设置 KV 表，用于存放内嵌 MCP server 的配置。
--
-- 0020 曾建过 app_settings（proxy 凭证），0023 随凭证功能一起 DROP。这里重新
-- 建回同样结构的 KV 表，专用于 MCP server 开关 / 端口 / 访问 token 等全局设置。
-- 约定用点分前缀自治 namespace（"mcp.enabled" / "mcp.port" / "mcp.token"）。
--
-- 安全：value 明文存 sqlite（token 属本机 loopback 访问凭证，与其它本机数据同级
-- 由 OS 文件系统保护）。
CREATE TABLE IF NOT EXISTS app_settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL
);
