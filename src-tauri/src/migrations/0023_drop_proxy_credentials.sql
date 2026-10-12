-- 移除全局 proxy 凭证（identity 分享时代的遗留设计）。
--
-- 占位符机制（proxy_url 存 ${user}/${pass} 模板 + app_settings 存真值）原本是
-- 为了让导出的 identity zip 不携带明文凭证。identity 功能删除后失去意义，
-- 改为 profile 直接存完整代理 URL。
--
-- 迁移：把现有模板 URL 用已配置的凭证展开写回明文；凭证为空时保留占位符
-- 原样（这类 profile 之前也无法启动，用户编辑 proxy URL 时自行补全）。

UPDATE browsers
   SET proxy_url = replace(proxy_url, '${user}',
       (SELECT value FROM app_settings WHERE key = 'proxy.username'))
 WHERE proxy_url LIKE '%${user}%'
   AND COALESCE((SELECT value FROM app_settings WHERE key = 'proxy.username'), '') != '';

UPDATE browsers
   SET proxy_url = replace(proxy_url, '${pass}',
       (SELECT value FROM app_settings WHERE key = 'proxy.password'))
 WHERE proxy_url LIKE '%${pass}%'
   AND COALESCE((SELECT value FROM app_settings WHERE key = 'proxy.password'), '') != '';

DROP TABLE IF EXISTS app_settings;
