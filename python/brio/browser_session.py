"""指纹浏览器拉起器（launch-browser）。

职责：必要时探时区 → 起本地代理转发 → 起 Chromium → 等退出 / 响应 stop。
保留 user-data-dir 里的登录态与 cookie，不做任何 identity 导出。

由 `python -m brio launch-browser ...` 驱动；
通过 stdin 接收 `{"cmd":"stop"}` 主动退出。
"""

from __future__ import annotations

import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

from .browser_close import cdp_browser_close, send_wm_close
from .core import (
    DEFAULT_TIMEZONE,
    build_browser_args,
    check_proxy,
    detect_timezone,
    pick_free_port,
)
from .events import (
    STAGE_BROWSER_RUNNING,
    STAGE_BROWSER_STARTING,
    STAGE_DETECT_TZ,
    STAGE_DONE,
    STAGE_ERROR,
    STAGE_FORWARDER_STARTED,
    STAGE_PREPARING,
    STAGE_PROXY_CHECK,
    EventEmitter,
)
from .local_proxy_forwarder import (
    LocalProxyForwarder,
    UpstreamProxy,
    parse_proxy_server,
    parse_proxy_url,
)


@dataclass
class BrowserParams:
    proxy_url: str = ""
    timezone: Optional[str] = None
    language: str = "en-US"
    fingerprint_id: str = ""
    chromium_bin: str = ""
    fingerprint_platform: str = "windows"
    fingerprint_brand: str = "Edge"
    open_urls: list[str] = field(default_factory=list)
    # 是否分配 CDP 远程调试端口；为 True 则在 prepare 阶段挑一个空闲端口。
    enable_cdp: bool = True
    # 显式指定端口（>0 时覆盖 enable_cdp 的自动分配）。
    cdp_port: int = 0


@dataclass
class BrowserPaths:
    user_data_dir: Path


class BrowserLauncher:
    def __init__(
        self,
        params: BrowserParams,
        paths: BrowserPaths,
        emitter: EventEmitter,
    ) -> None:
        self.params = params
        self.paths = paths
        self.emitter = emitter

        self._forwarder: LocalProxyForwarder | None = None
        self._proc: subprocess.Popen | None = None
        self._proxy_arg: str | None = None
        self._timezone: str = params.timezone or ""
        self._stopping = False
        self._cdp_port: int = 0

    def prepare(self) -> None:
        self.emitter.stage(STAGE_PREPARING)

        if not self._timezone:
            self.emitter.stage(STAGE_DETECT_TZ)
            tz = detect_timezone(self.params.proxy_url or None)
            if tz:
                self._timezone = tz
                self.emitter.emit("tz", timezone=tz, detected=True)
            else:
                self._timezone = DEFAULT_TIMEZONE
                self.emitter.emit("tz", timezone=DEFAULT_TIMEZONE, detected=False)
        else:
            self.emitter.emit("tz", timezone=self._timezone, detected=False)

        if self.params.proxy_url:
            proxy_host, proxy_user, proxy_pass = parse_proxy_url(
                self.params.proxy_url
            )
            upstream_host, upstream_port = parse_proxy_server(proxy_host)
            self.emitter.stage(
                STAGE_PROXY_CHECK, host=upstream_host, port=upstream_port
            )
            ok = check_proxy(upstream_host, upstream_port)
            self.emitter.emit(
                "proxy_check", host=upstream_host, port=upstream_port, ok=ok
            )
            upstream = UpstreamProxy(
                host=upstream_host,
                port=upstream_port,
                username=proxy_user,
                password=proxy_pass,
            )
            self._forwarder = LocalProxyForwarder(
                upstream, listen_host="127.0.0.1", listen_port=0
            )
            self._forwarder.start()
            self._proxy_arg = f"--proxy-server={self._forwarder.proxy_url}"
            self.emitter.stage(
                STAGE_FORWARDER_STARTED, url=self._forwarder.proxy_url
            )

        self.paths.user_data_dir.mkdir(parents=True, exist_ok=True)

        if self.params.cdp_port and self.params.cdp_port > 0:
            self._cdp_port = int(self.params.cdp_port)
        elif self.params.enable_cdp:
            self._cdp_port = pick_free_port()
        if self._cdp_port:
            self.emitter.emit(
                "cdp_port",
                port=self._cdp_port,
                endpoint=f"http://127.0.0.1:{self._cdp_port}",
            )

    def start(self) -> int:
        if not Path(self.params.chromium_bin).exists():
            raise FileNotFoundError(
                f"chromium not found: {self.params.chromium_bin}"
            )
        args = build_browser_args(
            chromium_bin=self.params.chromium_bin,
            fingerprint_id=self.params.fingerprint_id,
            user_data_dir=self.paths.user_data_dir,
            timezone=self._timezone,
            proxy_arg=self._proxy_arg,
            open_urls=self.params.open_urls,
            fingerprint_platform=self.params.fingerprint_platform,
            fingerprint_brand=self.params.fingerprint_brand,
            remote_debugging_port=self._cdp_port or None,
        )
        # --lang 只影响 UI 语言，且 macOS 上被 Chromium 忽略（跟随系统语言）；
        # 网站可见的 Accept-Language 头 / navigator.language(s) 由
        # fingerprint-chromium 的 --accept-lang 控制，两个都传。
        if self.params.language:
            lang = self.params.language
            args.append(f"--lang={lang}")
            primary = lang.split("-")[0]
            accept = lang if primary == lang else f"{lang},{primary}"
            args.append(f"--accept-lang={accept}")

        # chromium 上次没干净退时（dev rebuild / SIGKILL / 崩溃）会在 udd 里留下
        # SingletonLock/SingletonCookie/SingletonSocket，新进程启动会立刻自杀。
        # 这三个文件是 runtime 锁、不是用户数据，无脑删掉是安全的。
        for name in ("SingletonLock", "SingletonCookie", "SingletonSocket"):
            stale = self.paths.user_data_dir / name
            try:
                if stale.is_symlink() or stale.exists():
                    stale.unlink(missing_ok=True)
            except OSError as e:
                self.emitter.log(
                    f"cleanup stale {name} failed: {e}", level="warn"
                )

        self.emitter.stage(STAGE_BROWSER_STARTING)
        # chromium stderr 不能丢，闪退时这是唯一的诊断线索；透到 sidecar 自身
        # stderr，Rust BrowserSupervisor 会把它打成 stderr envelope 给前端。
        self._proc = subprocess.Popen(
            args,
            stdout=subprocess.DEVNULL,
            stderr=None,
        )
        self.emitter.stage(STAGE_BROWSER_RUNNING, pid=self._proc.pid)
        self.emitter.emit(
            "browser_started",
            pid=self._proc.pid,
            cdp_port=self._cdp_port or None,
            cdp_endpoint=f"http://127.0.0.1:{self._cdp_port}" if self._cdp_port else None,
        )
        return self._proc.pid

    @property
    def cdp_port(self) -> int:
        return self._cdp_port

    @property
    def cdp_endpoint(self) -> str:
        return f"http://127.0.0.1:{self._cdp_port}" if self._cdp_port else ""

    def wait(self) -> int:
        if self._proc is None:
            return 0
        code = self._proc.wait()
        self.emitter.emit("browser_exit", code=code)
        return code

    def terminate(self, timeout: float = 5.0) -> None:
        self._stopping = True
        proc = self._proc
        if proc is None:
            return
        try:
            if sys.platform == "win32":
                deadline = time.monotonic() + timeout
                self._request_close(proc)
                proc.wait(timeout=max(deadline - time.monotonic(), 0.5))
            else:
                proc.terminate()
                proc.wait(timeout=timeout)
        except Exception:
            try:
                proc.kill()
            except Exception:
                pass

    def _request_close(self, proc: subprocess.Popen) -> None:
        # Windows 上 proc.terminate() 等于 TerminateProcess 强杀，cookie 来不及落盘；
        # 先请 Chromium 自己退出，超时再由 terminate() 兜底强杀。
        if proc.poll() is not None:
            return
        if self._cdp_port and cdp_browser_close(self._cdp_port):
            self.emitter.log("asked chromium to exit via CDP Browser.close")
        elif send_wm_close(proc.pid):
            self.emitter.log("asked chromium to exit via WM_CLOSE")
        else:
            self.emitter.log(
                "graceful close request failed, will kill after timeout",
                level="warn",
            )

    def cleanup(self) -> None:
        if self._forwarder is not None:
            try:
                self._forwarder.stop()
            except Exception:
                pass
            self._forwarder = None
        self.emitter.stage(STAGE_DONE)
