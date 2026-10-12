-- chat_fork 需要两块数据：
--   1) transcript_seed_path：一个 JSONL 文件路径，里面是 {role, content} 的消息列表；
--      sidecar 启动时 load 它当成 LLM 已经"看过"的上下文（不会再次 emit 给 UI）。
--   2) forked_from_* ：血缘信息，用来在 UI 里显示 "Forked from chat X"。
--
-- 使用模式：
--   chat_fork 命令接收前端收集好的消息列表，写文件，INSERT 新 chat row 时把
--   transcript_seed_path 指向它；chat_start 照常把路径透传到 sidecar 的
--   --transcript-seed。

ALTER TABLE chats ADD COLUMN transcript_seed_path TEXT;
ALTER TABLE chats ADD COLUMN forked_from_chat_id  TEXT;
ALTER TABLE chats ADD COLUMN forked_from_seq      INTEGER;
