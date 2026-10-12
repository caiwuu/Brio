"""请 Chromium 自己正常退出（Windows 用）。

Windows 上 `Popen.terminate()` 就是 TerminateProcess 强杀：最近的 cookie / 登录态
来不及落盘，下次启动还会提示"未正常关闭"。这里提供两种温和的关闭方式，
由 `BrowserLauncher.terminate()` 依次尝试，都不行再强杀：

- CDP `Browser.close`：launcher 知道调试端口；sidecar 必须保持纯标准库，
  所以自带一个只够发一条命令的 WebSocket 客户端
- `taskkill /PID`（不带 /F）：给浏览器窗口发 WM_CLOSE，相当于用户点了关闭
"""

from __future__ import annotations

import base64
import http.client
import json
import os
import socket
import subprocess
from typing import BinaryIO
from urllib.parse import urlparse

_OP_TEXT = 0x1
_OP_CLOSE = 0x8


def cdp_browser_close(port: int, timeout: float = 1.0) -> bool:
    """发送 CDP `Browser.close`。命令送达返回 True；连不上 / 被拒绝返回 False。"""
    try:
        # 用 http.client 直连：urllib 会读系统代理（Windows 注册表），本机端口可能被转发走。
        conn = http.client.HTTPConnection("127.0.0.1", port, timeout=timeout)
        try:
            conn.request("GET", "/json/version")
            info = json.loads(conn.getresponse().read().decode("utf-8"))
        finally:
            conn.close()
        path = urlparse(info["webSocketDebuggerUrl"]).path

        with socket.create_connection(("127.0.0.1", port), timeout=timeout) as sock:
            key = base64.b64encode(os.urandom(16)).decode("ascii")
            # 不带 Origin 头：Chromium 111+ 只校验带 Origin 的连接（--remote-allow-origins）。
            sock.sendall(
                (
                    f"GET {path} HTTP/1.1\r\n"
                    f"Host: 127.0.0.1:{port}\r\n"
                    "Upgrade: websocket\r\n"
                    "Connection: Upgrade\r\n"
                    f"Sec-WebSocket-Key: {key}\r\n"
                    "Sec-WebSocket-Version: 13\r\n"
                    "\r\n"
                ).encode("ascii")
            )
            reader = sock.makefile("rb")
            if b" 101 " not in reader.readline():
                return False
            while reader.readline() not in (b"\r\n", b""):
                pass

            command = {"id": 1, "method": "Browser.close"}
            sock.sendall(_ws_frame(json.dumps(command).encode("utf-8")))
            # Chromium 先回 {"id":1,"result":{}} 再退出；回包前连接就断了也算送达。
            try:
                while True:
                    opcode, data = _ws_read_frame(reader)
                    if opcode == _OP_CLOSE:
                        return True
                    if opcode == _OP_TEXT:
                        msg = json.loads(data)
                        if isinstance(msg, dict) and msg.get("id") == 1:
                            return "error" not in msg
            except (OSError, ValueError):
                return True
    except (OSError, ValueError, KeyError, TypeError, http.client.HTTPException):
        return False


def send_wm_close(pid: int, timeout: float = 3.0) -> bool:
    """`taskkill /PID`（不带 /F）给进程的顶层窗口发 WM_CLOSE。成功发出返回 True。"""
    try:
        result = subprocess.run(
            ["taskkill", "/PID", str(pid)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=timeout,
            # sidecar 本身没有控制台窗口，不加这个 taskkill 会闪一下黑框。
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
    except (OSError, subprocess.SubprocessError):
        return False
    return result.returncode == 0


def _ws_frame(payload: bytes, opcode: int = _OP_TEXT) -> bytes:
    """单帧（FIN=1）。客户端发出的帧必须带掩码（RFC 6455 §5.3）。"""
    header = bytearray([0x80 | opcode])
    n = len(payload)
    if n < 126:
        header.append(0x80 | n)
    elif n < 1 << 16:
        header.append(0x80 | 126)
        header += n.to_bytes(2, "big")
    else:
        header.append(0x80 | 127)
        header += n.to_bytes(8, "big")
    mask = os.urandom(4)
    header += mask
    return bytes(header) + bytes(b ^ mask[i % 4] for i, b in enumerate(payload))


def _ws_read_frame(reader: BinaryIO) -> tuple[int, bytes]:
    """读一帧，返回 (opcode, payload)。不处理分片：Chromium 的短回包不会分片。"""
    b0, b1 = _read_exact(reader, 2)
    n = b1 & 0x7F
    if n == 126:
        n = int.from_bytes(_read_exact(reader, 2), "big")
    elif n == 127:
        n = int.from_bytes(_read_exact(reader, 8), "big")
    mask = _read_exact(reader, 4) if b1 & 0x80 else b""
    data = _read_exact(reader, n)
    if mask:
        data = bytes(b ^ mask[i % 4] for i, b in enumerate(data))
    return b0 & 0x0F, data


def _read_exact(reader: BinaryIO, n: int) -> bytes:
    data = reader.read(n)
    if len(data) < n:
        raise ConnectionError("websocket closed")
    return data
