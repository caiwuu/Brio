-- M3：种入默认 providers，让 identities.provider_id 有可用值。
-- 以后用户可以在 Providers 设置页里自定义，这里只是兜底。

INSERT OR IGNORE INTO providers (id, label, base_url, enabled, created_at, updated_at) VALUES
    ('claude',   'Claude',     'https://claude.ai',        1, strftime('%s','now'), strftime('%s','now')),
    ('chatgpt',  'ChatGPT',    'https://chatgpt.com',      1, strftime('%s','now'), strftime('%s','now')),
    ('deepseek', 'DeepSeek',   'https://chat.deepseek.com',1, strftime('%s','now'), strftime('%s','now'));
