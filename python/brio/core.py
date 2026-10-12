"""核心纯函数：fingerprint / 时区检测 / 代理探测 / 启动参数构建。

本模块不做任何 print；所有路径以入参形式传入，便于被 sidecar 复用。
"""

from __future__ import annotations

import hashlib
import json
import os
import socket
import urllib.request
from pathlib import Path
from typing import Iterable

from .local_proxy_forwarder import (
    LocalProxyForwarder,
    UpstreamProxy,
    parse_proxy_server,
    parse_proxy_url,
)

# ---------------------------------------------------------------------------
# 常量
# ---------------------------------------------------------------------------

DEFAULT_TIMEZONE = "America/Chicago"


# ---------------------------------------------------------------------------
# 工具函数
# ---------------------------------------------------------------------------


FP_DIGITS = 16
"""指纹 ID 的十进制位数。

16 位 ≈ 5.3×10^15 空间，碰撞概率对个人桌面用够低；同时仍在 sqlite INTEGER
(64-bit signed, max ≈ 9.2×10^18) 范围内，可读、可排序、可转 int。
"""


def _hash_to_decimal(raw: str, digits: int = FP_DIGITS) -> str:
    """sha256(raw) → int → mod 10^digits → zero-padded decimal string。"""
    if digits <= 0:
        raise ValueError("digits must be > 0")
    h = hashlib.sha256(raw.strip().lower().encode("utf-8")).hexdigest()
    return str(int(h, 16) % (10**digits)).zfill(digits)


def derive_fingerprint_id(
    proxy_url: str,
    timezone: str,
    language: str,
    *,
    digits: int = FP_DIGITS,
) -> str:
    """根据 (proxy, tz, lang) 派生稳定的指纹 ID。

    - 同一组三元组永远得到同一个 ID（便于幂等创建浏览器）
    - 输出：``digits`` 位十进制字符串（默认 16 位）。整数序列，便于做 INT 主键 /
      hash bucket / 显示对齐。

    算法::

        h = sha256(f"{proxy}|{tz}|{lang}".strip().lower()).hexdigest()
        return str(int(h, 16) % 10**digits).zfill(digits)
    """
    raw = f"{proxy_url.strip()}|{timezone.strip()}|{language.strip()}"
    return _hash_to_decimal(raw, digits)


def resolve_chromium_bin(
    cli_value: str | None,
    *,
    env_value: str | None = None,
    default: str | None = None,
) -> str:
    """按优先级解析 Chromium 路径：CLI > 环境变量 > default。"""
    if cli_value:
        return cli_value
    env = (
        env_value if env_value is not None else os.environ.get("CHROMIUM_BIN", "")
    ).strip()
    if env:
        return env
    return default or ""


def check_proxy(host: str, port: int, timeout: float = 5.0) -> bool:
    """TCP 连通性检测。"""
    try:
        with socket.create_connection((host, port), timeout=timeout):
            return True
    except (OSError, TimeoutError):
        return False


def detect_timezone(proxy_url: str | None = None) -> str | None:
    """通过 IP 地理位置 API 自动检测时区。

    若代理 URL 以 socks5:// 开头，会临时启动 LocalProxyForwarder，把上游转成
    本机 HTTP 代理再请求（urllib 不支持 SOCKS5 握手；转发侧对上游仍走 HTTP CONNECT）。
    """
    temp_forwarder: LocalProxyForwarder | None = None
    try:
        if proxy_url and proxy_url.startswith("socks5://"):
            proxy_host, proxy_user, proxy_pass = parse_proxy_url(proxy_url)
            upstream_host, upstream_port = parse_proxy_server(proxy_host)
            upstream = UpstreamProxy(
                host=upstream_host,
                port=upstream_port,
                username=proxy_user,
                password=proxy_pass,
            )
            temp_forwarder = LocalProxyForwarder(
                upstream,
                listen_host="127.0.0.1",
                listen_port=0,
            )
            temp_forwarder.start()
            http_proxy = temp_forwarder.proxy_url
            proxy_handler = urllib.request.ProxyHandler(
                {"http": http_proxy, "https": http_proxy}
            )
            opener = urllib.request.build_opener(proxy_handler)
        elif proxy_url:
            proxy_handler = urllib.request.ProxyHandler(
                {"http": proxy_url, "https": proxy_url}
            )
            opener = urllib.request.build_opener(proxy_handler)
        else:
            opener = urllib.request.build_opener()

        apis = [
            ("http://ip-api.com/json/?fields=timezone", "timezone"),
            ("http://worldtimeapi.org/api/ip", "timezone"),
        ]
        for url, key in apis:
            try:
                req = urllib.request.Request(
                    url, headers={"Accept": "application/json"}
                )
                with opener.open(req, timeout=5) as resp:
                    data = json.loads(resp.read().decode())
                    tz = str(data.get(key, "")).strip()
                    if tz:
                        return tz
            except Exception:
                continue
        return None
    except Exception:
        return None
    finally:
        if temp_forwarder is not None:
            try:
                temp_forwarder.stop()
            except Exception:
                pass


def build_browser_args(
    *,
    chromium_bin: str,
    fingerprint_id: str,
    user_data_dir: Path,
    timezone: str,
    proxy_arg: str | None,
    open_urls: Iterable[str] = (),
    fingerprint_platform: str = "windows",
    fingerprint_brand: str = "Edge",
    remote_debugging_port: int | None = None,
) -> list[str]:
    """构造 Chromium 启动参数。"""
    args = [
        chromium_bin,
        f"--fingerprint={fingerprint_id}",
        f"--fingerprint-platform={fingerprint_platform}",
        f"--fingerprint-brand={fingerprint_brand}",
        f"--user-data-dir={user_data_dir}",
        f"--timezone={timezone}",
        "--force-webrtc-ip-handling-policy",
        "--webrtc-ip-handling-policy=disable_non_proxied_udp",
        "--disable-features=AsyncDNS",
        "--no-first-run",
        "--no-default-browser-check",
        # 限制每个 profile 的磁盘缓存，否则 udd 会随使用无限膨胀
        # （Cookie / storage 等登录态不受影响，只限 HTTP/media cache）。
        "--disk-cache-size=268435456",   # 256 MB
        "--media-cache-size=134217728",  # 128 MB
    ]
    if remote_debugging_port:
        # 监听在 127.0.0.1，避免外部网络可达。
        args.append(f"--remote-debugging-port={int(remote_debugging_port)}")
        args.append("--remote-debugging-address=127.0.0.1")
    if proxy_arg:
        args.append(proxy_arg)
    args.extend(open_urls)
    return args


def pick_free_port(host: str = "127.0.0.1") -> int:
    """让 OS 分配一个空闲 TCP 端口。返回端口号。"""
    import socket

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind((host, 0))
        return int(s.getsockname()[1])
