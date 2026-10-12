"""sidecar 与 Tauri Rust 端之间的事件协议。

每个事件以 JSON 一行（NDJSON）写到 stdout。Rust 端按行解析后通过 emit
转发到前端窗口。
"""

from __future__ import annotations

import json
import sys
import threading
from typing import Any, Callable

# 阶段枚举：与前端 UI 状态机对齐
STAGE_PREPARING = "preparing"
STAGE_DETECT_TZ = "detect_tz"
STAGE_PROXY_CHECK = "proxy_check"
STAGE_FORWARDER_STARTED = "forwarder_started"
STAGE_BROWSER_STARTING = "browser_starting"
STAGE_BROWSER_RUNNING = "browser_running"
STAGE_DONE = "done"
STAGE_ERROR = "error"


class EventEmitter:
    """线程安全的 NDJSON emitter。"""

    def __init__(self, stream=None) -> None:
        self._stream = stream if stream is not None else sys.stdout
        self._lock = threading.Lock()

    def emit(self, event: str, **payload: Any) -> None:
        record: dict[str, Any] = {"event": event, **payload}
        line = json.dumps(record, ensure_ascii=False)
        with self._lock:
            try:
                self._stream.write(line + "\n")
                self._stream.flush()
            except Exception:
                # Rust 端已退出、管道断开时事件没人收，丢掉即可；
                # 不能抛出去打断还在进行的关浏览器流程。
                pass

    # 便捷方法 ---------------------------------------------------------

    def stage(self, stage: str, **payload: Any) -> None:
        self.emit("stage", stage=stage, **payload)

    def log(self, message: str, level: str = "info") -> None:
        self.emit("log", level=level, message=message)

    def error(self, message: str, **payload: Any) -> None:
        self.emit("error", message=message, **payload)


EventCallback = Callable[[str, dict[str, Any]], None]
