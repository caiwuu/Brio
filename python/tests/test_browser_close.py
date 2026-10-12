"""Windows 正常关闭 Chromium 的辅助逻辑：CDP Browser.close / WM_CLOSE / terminate 分支。"""

from __future__ import annotations

import base64
import hashlib
import io
import json
import os
import socket
import subprocess
import sys
import threading
import time
import urllib.request
from pathlib import Path

import pytest

from brio import browser_close, browser_session
from brio.browser_close import _ws_frame, _ws_read_frame, cdp_browser_close
from brio.browser_session import BrowserLauncher, BrowserParams, BrowserPaths
from brio.core import pick_free_port
from brio.events import EventEmitter

_WS_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


def _recv_until(conn: socket.socket, marker: bytes) -> bytes:
    data = b""
    while marker not in data:
        chunk = conn.recv(4096)
        if not chunk:
            break
        data += chunk
    return data


class FakeCdp:
    """只实现 /json/version 与一次 WebSocket 往返的假 DevTools 端点。"""

    def __init__(self, reply: dict | None) -> None:
        self.reply = reply
        self.received: list[dict] = []
        self.masked: list[bool] = []
        self._sock = socket.socket()
        self._sock.bind(("127.0.0.1", 0))
        self._sock.listen()
        self.port = self._sock.getsockname()[1]
        threading.Thread(target=self._serve, daemon=True).start()

    def close(self) -> None:
        self._sock.close()

    def _serve(self) -> None:
        while True:
            try:
                conn, _ = self._sock.accept()
            except OSError:
                return
            with conn:
                req = _recv_until(conn, b"\r\n\r\n")
                if req.startswith(b"GET /json/version"):
                    body = json.dumps(
                        {
                            "Browser": "Fake/1.0",
                            "webSocketDebuggerUrl": f"ws://127.0.0.1:{self.port}/devtools/browser/abc",
                        }
                    ).encode()
                    conn.sendall(
                        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n"
                        + f"Content-Length: {len(body)}\r\n\r\n".encode()
                        + body
                    )
                elif b"/devtools/browser/abc" in req:
                    self._websocket(conn, req)

    def _websocket(self, conn: socket.socket, req: bytes) -> None:
        headers = dict(
            line.split(": ", 1)
            for line in req.decode().split("\r\n")[1:]
            if ": " in line
        )
        accept = base64.b64encode(
            hashlib.sha1((headers["Sec-WebSocket-Key"] + _WS_GUID).encode()).digest()
        ).decode()
        conn.sendall(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n"
            f"Connection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n".encode()
        )
        masked, payload = _server_read_frame(conn.makefile("rb"))
        self.masked.append(masked)
        self.received.append(json.loads(payload))
        if self.reply is not None:
            data = json.dumps(self.reply).encode()
            conn.sendall(bytes([0x81, len(data)]) + data)


def _server_read_frame(reader) -> tuple[bool, bytes]:
    """服务端视角独立解析一帧，不复用被测的 _ws_read_frame。"""
    _b0, b1 = reader.read(2)
    masked = bool(b1 & 0x80)
    n = b1 & 0x7F
    if n == 126:
        n = int.from_bytes(reader.read(2), "big")
    elif n == 127:
        n = int.from_bytes(reader.read(8), "big")
    mask = reader.read(4) if masked else b""
    data = reader.read(n)
    if masked:
        data = bytes(b ^ mask[i % 4] for i, b in enumerate(data))
    return masked, data


@pytest.fixture
def fake_cdp():
    servers: list[FakeCdp] = []

    def make(reply: dict | None) -> FakeCdp:
        server = FakeCdp(reply)
        servers.append(server)
        return server

    yield make
    for s in servers:
        s.close()


# --------------------------------------------------------------------------
# WebSocket 帧编解码
# --------------------------------------------------------------------------


@pytest.mark.parametrize("size", [0, 10, 125, 126, 200, 65535, 65536, 70000])
def test_ws_frame_roundtrip(size: int) -> None:
    payload = os.urandom(size)
    frame = _ws_frame(payload)
    assert frame[0] == 0x81  # FIN + text
    assert frame[1] & 0x80  # 客户端帧必须带掩码
    opcode, data = _ws_read_frame(io.BytesIO(frame))
    assert opcode == 0x1
    assert data == payload
    assert _server_read_frame(io.BytesIO(frame)) == (True, payload)


def test_ws_read_frame_truncated_raises() -> None:
    with pytest.raises(ConnectionError):
        _ws_read_frame(io.BytesIO(b"\x81\x05ab"))


# --------------------------------------------------------------------------
# cdp_browser_close
# --------------------------------------------------------------------------


def test_cdp_browser_close_sends_masked_command(fake_cdp) -> None:
    server = fake_cdp({"id": 1, "result": {}})
    assert cdp_browser_close(server.port) is True
    assert server.received == [{"id": 1, "method": "Browser.close"}]
    assert server.masked == [True]


def test_cdp_browser_close_error_reply_is_failure(fake_cdp) -> None:
    server = fake_cdp({"id": 1, "error": {"code": -32000, "message": "nope"}})
    assert cdp_browser_close(server.port) is False


def test_cdp_browser_close_connection_dropped_counts_as_delivered(fake_cdp) -> None:
    # 真实 Chromium 可能在回包前就关掉连接
    server = fake_cdp(None)
    assert cdp_browser_close(server.port) is True
    assert server.received == [{"id": 1, "method": "Browser.close"}]


def test_cdp_browser_close_nothing_listening() -> None:
    t0 = time.monotonic()
    assert cdp_browser_close(pick_free_port()) is False
    # Windows 上连接被拒会先重试 SYN，约 1s 才报错；这里只防卡死
    assert time.monotonic() - t0 < 3.0


def test_cdp_browser_close_bad_version_endpoint() -> None:
    sock = socket.socket()
    sock.bind(("127.0.0.1", 0))
    sock.listen()
    port = sock.getsockname()[1]

    def serve() -> None:
        conn, _ = sock.accept()
        with conn:
            _recv_until(conn, b"\r\n\r\n")
            conn.sendall(b"HTTP/1.1 404 Not Found\r\nContent-Length: 2\r\n\r\nno")

    threading.Thread(target=serve, daemon=True).start()
    try:
        assert cdp_browser_close(port) is False
    finally:
        sock.close()


# --------------------------------------------------------------------------
# send_wm_close
# --------------------------------------------------------------------------


def test_send_wm_close_runs_taskkill_without_force(monkeypatch) -> None:
    calls: list[list[str]] = []

    def fake_run(cmd, **kwargs):
        calls.append(cmd)
        return subprocess.CompletedProcess(cmd, 0)

    monkeypatch.setattr(browser_close.subprocess, "run", fake_run)
    assert browser_close.send_wm_close(1234) is True
    assert calls == [["taskkill", "/PID", "1234"]]


def test_send_wm_close_failure(monkeypatch) -> None:
    def missing(cmd, **kwargs):
        raise FileNotFoundError(cmd[0])

    monkeypatch.setattr(browser_close.subprocess, "run", missing)
    assert browser_close.send_wm_close(1234) is False

    monkeypatch.setattr(
        browser_close.subprocess,
        "run",
        lambda cmd, **kw: subprocess.CompletedProcess(cmd, 128),
    )
    assert browser_close.send_wm_close(1234) is False


# --------------------------------------------------------------------------
# BrowserLauncher.terminate 的平台分支
# --------------------------------------------------------------------------


class FakeProc:
    def __init__(self, exits: bool = True) -> None:
        self.pid = 4321
        self.exits = exits
        self.calls: list[str] = []

    def poll(self):
        return None

    def terminate(self) -> None:
        self.calls.append("terminate")

    def kill(self) -> None:
        self.calls.append("kill")

    def wait(self, timeout=None):
        self.calls.append("wait")
        if not self.exits:
            raise subprocess.TimeoutExpired("chromium", timeout)
        return 0


def _launcher(proc: FakeProc, cdp_port: int = 9222) -> tuple[BrowserLauncher, io.StringIO]:
    out = io.StringIO()
    launcher = BrowserLauncher(
        BrowserParams(fingerprint_id="1"),
        BrowserPaths(user_data_dir=Path("unused")),
        EventEmitter(out),
    )
    launcher._proc = proc  # type: ignore[assignment]
    launcher._cdp_port = cdp_port
    return launcher, out


def _logs(out: io.StringIO) -> list[str]:
    return [json.loads(line)["message"] for line in out.getvalue().splitlines()]


def test_terminate_posix_keeps_sigterm(monkeypatch) -> None:
    monkeypatch.setattr(sys, "platform", "darwin")
    monkeypatch.setattr(
        browser_session, "cdp_browser_close", lambda *a, **k: pytest.fail("no CDP on posix")
    )
    proc = FakeProc()
    launcher, _ = _launcher(proc)
    launcher.terminate()
    assert proc.calls == ["terminate", "wait"]


def test_terminate_windows_prefers_cdp(monkeypatch) -> None:
    monkeypatch.setattr(sys, "platform", "win32")
    ports: list[int] = []
    monkeypatch.setattr(
        browser_session, "cdp_browser_close", lambda port, **k: ports.append(port) or True
    )
    monkeypatch.setattr(
        browser_session, "send_wm_close", lambda pid, **k: pytest.fail("CDP already worked")
    )
    proc = FakeProc()
    launcher, out = _launcher(proc, cdp_port=9333)
    launcher.terminate()
    assert ports == [9333]
    assert proc.calls == ["wait"]
    assert "CDP Browser.close" in _logs(out)[0]


def test_terminate_windows_falls_back_to_wm_close(monkeypatch) -> None:
    monkeypatch.setattr(sys, "platform", "win32")
    monkeypatch.setattr(browser_session, "cdp_browser_close", lambda port, **k: False)
    pids: list[int] = []
    monkeypatch.setattr(
        browser_session, "send_wm_close", lambda pid, **k: pids.append(pid) or True
    )
    proc = FakeProc()
    launcher, out = _launcher(proc)
    launcher.terminate()
    assert pids == [4321]
    assert proc.calls == ["wait"]
    assert "WM_CLOSE" in _logs(out)[0]


def test_terminate_windows_without_cdp_port_uses_wm_close(monkeypatch) -> None:
    monkeypatch.setattr(sys, "platform", "win32")
    monkeypatch.setattr(
        browser_session, "cdp_browser_close", lambda *a, **k: pytest.fail("CDP disabled")
    )
    monkeypatch.setattr(browser_session, "send_wm_close", lambda pid, **k: True)
    proc = FakeProc()
    launcher, _ = _launcher(proc, cdp_port=0)
    launcher.terminate()
    assert proc.calls == ["wait"]


def test_terminate_windows_kills_after_timeout(monkeypatch) -> None:
    monkeypatch.setattr(sys, "platform", "win32")
    monkeypatch.setattr(browser_session, "cdp_browser_close", lambda port, **k: False)
    monkeypatch.setattr(browser_session, "send_wm_close", lambda pid, **k: False)
    proc = FakeProc(exits=False)
    launcher, out = _launcher(proc)
    launcher.terminate(timeout=0.1)
    assert proc.calls == ["wait", "kill"]
    assert "will kill" in _logs(out)[0]


# --------------------------------------------------------------------------
# 真实 Chromium（可选）：CHROMIUM_BIN 指向本机 Chromium 时才跑
# --------------------------------------------------------------------------

_REAL_CHROMIUM = os.environ.get("CHROMIUM_BIN", "")


def _launch_real_chromium(udd: Path) -> tuple[subprocess.Popen, int]:
    port = pick_free_port()
    proc = subprocess.Popen(
        [
            _REAL_CHROMIUM,
            "--headless=new",
            "--use-mock-keychain",
            "--no-first-run",
            f"--remote-debugging-port={port}",
            f"--user-data-dir={udd}",
            "about:blank",
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/version", timeout=1):
                return proc, port
        except OSError:
            time.sleep(0.2)
    proc.kill()
    pytest.fail("chromium devtools endpoint never came up")


@pytest.mark.skipif(not _REAL_CHROMIUM, reason="设置 CHROMIUM_BIN 才跑真实 Chromium")
def test_real_chromium_exits_cleanly_on_browser_close(tmp_path: Path) -> None:
    proc, port = _launch_real_chromium(tmp_path / "udd")
    try:
        assert cdp_browser_close(port) is True
        assert proc.wait(timeout=15) == 0
    finally:
        if proc.poll() is None:
            proc.kill()


@pytest.mark.skipif(not _REAL_CHROMIUM, reason="设置 CHROMIUM_BIN 才跑真实 Chromium")
def test_real_chromium_windows_branch_end_to_end(tmp_path: Path, monkeypatch) -> None:
    proc, port = _launch_real_chromium(tmp_path / "udd")
    launcher, out = _launcher(proc, cdp_port=port)  # type: ignore[arg-type]
    monkeypatch.setattr(sys, "platform", "win32")
    try:
        launcher.terminate()
        assert proc.returncode == 0
        assert "CDP Browser.close" in _logs(out)[0]
    finally:
        if proc.poll() is None:
            proc.kill()
