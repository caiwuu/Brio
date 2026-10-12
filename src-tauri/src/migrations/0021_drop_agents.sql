-- v1 agents / agent_runs 已废弃，前端入口与 sidecar 一并移除。
-- 这里把残留的两张表也清掉，避免老库继续保留无主行。

DROP TABLE IF EXISTS agent_runs;
DROP TABLE IF EXISTS agents;
