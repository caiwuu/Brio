-- ephemeral browser 标记位。
--
-- 之前 web2api 启动时拉起的 ephemeral browser 和用户从 Identities 列表点
-- "Launch browser" 拉起的持久化 browser 都写成 source='identity'，区别只在
-- notes，没法稳定区分。后果：
--   - app 重启 / 用户没点 Stop API → ephemeral row 留在 DB 永不回收
--   - 下次 start API 时 ensure 流程只看 supervisor 内存（必空），又 INSERT 一条
--   - 同一个 fingerprint 的 ephemeral row 越攒越多（截图里 5 条）
--
-- 加 is_ephemeral=1 让 boot sweep 能精确清掉这类孤儿。
ALTER TABLE browsers ADD COLUMN is_ephemeral INTEGER NOT NULL DEFAULT 0;

-- 把现有的疑似 ephemeral row 也标上，让 boot sweep 一并清掉
-- 用户当下账上累出来的那 5 条都是这种。
UPDATE browsers
   SET is_ephemeral = 1
 WHERE source = 'identity'
   AND notes  = 'Ephemeral browser launched by web2api';
