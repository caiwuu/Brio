-- 0001_init.sql 当年预想"代理池"做成独立表，但后来浏览器代理直接放在
-- browsers.proxy_url，proxies 表从未被读写。
DROP TABLE IF EXISTS proxies;
