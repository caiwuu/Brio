-- 全局应用设置 KV 表。当前仅用于存放 proxy 凭证（username/password），
-- 但作为通用 KV 表保留，以便后续添加其他全局设置。
--
-- 设计：单值 settings 直接 (key, value) 一行；不强约束 namespace，使用方
-- 通过点分前缀（"proxy.username" / "proxy.password"）自治。
--
-- 安全：value 明文存 sqlite，与 credentials.secret 一致——本机加密由 OS
-- 文件系统级保护负责，desktop 这一层不做 keychain（可在后续 PR 加）。
CREATE TABLE IF NOT EXISTS app_settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL
);
