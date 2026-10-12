"""brio sidecar 入口。

被 Tauri Rust 端通过 `brio-py launch-browser ...` 拉起，
通过 stdout NDJSON 上报事件，通过 stdin NDJSON 接收控制命令：

    {"cmd": "stop"}      # 主动终止浏览器

也提供 detect-tz / derive-fp 两个一次性子命令供 UI 使用。
"""

from __future__ import annotations

import argparse
import io
import json
import signal
import sys
import threading
from pathlib import Path

from brio.browser_session import (
    BrowserLauncher,
    BrowserParams,
    BrowserPaths,
)
from brio.core import (
    DEFAULT_TIMEZONE,
    derive_fingerprint_id,
    detect_timezone,
    resolve_chromium_bin,
)
from brio.events import EventEmitter


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="brio-py")
    sub = parser.add_subparsers(dest="cmd", required=True)

    launch = sub.add_parser(
        "launch-browser",
        help="拉起一个指纹浏览器实例",
    )
    launch.add_argument("--proxy-url", default="")
    launch.add_argument("--timezone", default=None)
    launch.add_argument("--language", default="en-US")
    launch.add_argument("--fingerprint-id", required=True)
    launch.add_argument("--chromium-bin", required=True)
    launch.add_argument("--user-data-dir", required=True)
    launch.add_argument("--fingerprint-platform", default="windows")
    launch.add_argument("--fingerprint-brand", default="Edge")
    launch.add_argument(
        "--open-url",
        action="append",
        default=[],
        help="启动后打开的 URL，可多次指定",
    )
    launch.add_argument(
        "--cdp-port",
        type=int,
        default=0,
        help="远程调试端口；0 表示让 OS 自动分配（默认开启）",
    )
    launch.add_argument(
        "--no-cdp",
        action="store_true",
        help="禁用远程调试端口（默认开启，供外部自动化接入）",
    )

    detect = sub.add_parser("detect-tz", help="仅探测时区（用于 UI 自动检测按钮）")
    detect.add_argument("--proxy-url", default="")

    derive = sub.add_parser(
        "derive-fp",
        help="根据 (proxy, tz, lang) 派生稳定的 fingerprint_id",
    )
    derive.add_argument("--proxy-url", default="")
    derive.add_argument("--timezone", default="")
    derive.add_argument("--language", default="")

    sub.add_parser("ping", help="健康检查，立刻退出")

    return parser


def _stdin_reader(emitter: EventEmitter, on_command) -> threading.Thread:
    """后台线程：每行一条 JSON 命令，转发给 on_command(dict)。"""

    def _run() -> None:
        for raw in sys.stdin:
            raw = raw.strip()
            if not raw:
                continue
            try:
                msg = json.loads(raw)
            except json.JSONDecodeError as e:
                emitter.log(f"invalid stdin line: {e}", level="warn")
                continue
            try:
                on_command(msg)
            except Exception as e:  # 防止线程挂掉
                emitter.log(f"on_command error: {e}", level="error")

    t = threading.Thread(target=_run, daemon=True)
    t.start()
    return t


def cmd_launch_browser(args: argparse.Namespace) -> int:
    emitter = EventEmitter()

    chromium_bin = resolve_chromium_bin(args.chromium_bin)
    if not Path(chromium_bin).exists():
        emitter.error(f"chromium not found: {chromium_bin}")
        return 2

    enable_cdp = not bool(getattr(args, "no_cdp", False))
    cdp_port = int(getattr(args, "cdp_port", 0) or 0)
    params = BrowserParams(
        proxy_url=(args.proxy_url or "").strip(),
        timezone=(args.timezone.strip() if args.timezone else None) or None,
        language=(args.language or "en-US").strip(),
        fingerprint_id=args.fingerprint_id,
        chromium_bin=chromium_bin,
        fingerprint_platform=args.fingerprint_platform,
        fingerprint_brand=args.fingerprint_brand,
        open_urls=list(args.open_url or []),
        enable_cdp=enable_cdp,
        cdp_port=cdp_port,
    )
    paths = BrowserPaths(user_data_dir=Path(args.user_data_dir))

    emitter.emit(
        "session_init",
        fingerprint_id=params.fingerprint_id,
        proxy_url=params.proxy_url,
        timezone_input=params.timezone,
        language=params.language,
        chromium_bin=chromium_bin,
        user_data_dir=str(paths.user_data_dir),
    )

    launcher = BrowserLauncher(params, paths, emitter)

    def handle_command(msg: dict) -> None:
        cmd = msg.get("cmd")
        if cmd == "stop":
            emitter.log("received stop command")
            launcher.terminate()
        else:
            emitter.log(f"unknown cmd: {cmd}", level="warn")

    _stdin_reader(emitter, handle_command)

    def on_signal(sig: int, frame: object) -> None:
        emitter.log(f"received signal {sig}, terminating browser")
        launcher.terminate()

    signal.signal(signal.SIGINT, on_signal)
    signal.signal(signal.SIGTERM, on_signal)

    try:
        launcher.prepare()
        launcher.start()
        launcher.wait()
    except FileNotFoundError as e:
        emitter.error(str(e))
        launcher.cleanup()
        return 2
    except Exception as e:  # noqa: BLE001
        emitter.error(f"{type(e).__name__}: {e}")
        try:
            launcher.terminate()
        finally:
            launcher.cleanup()
        return 1

    launcher.cleanup()
    return 0


def cmd_detect_tz(args: argparse.Namespace) -> int:
    emitter = EventEmitter()
    proxy = (args.proxy_url or "").strip() or None
    tz = detect_timezone(proxy)
    if tz:
        emitter.emit("tz", timezone=tz, detected=True)
    else:
        emitter.emit("tz", timezone=DEFAULT_TIMEZONE, detected=False)
    return 0


def cmd_derive_fp(args: argparse.Namespace) -> int:
    emitter = EventEmitter()
    fp = derive_fingerprint_id(
        proxy_url=args.proxy_url or "",
        timezone=args.timezone or "",
        language=args.language or "",
    )
    emitter.emit("fp", fingerprint_id=fp)
    return 0


def cmd_ping(_: argparse.Namespace) -> int:
    EventEmitter().emit("pong", ok=True)
    return 0


def _force_utf8_stdio() -> None:
    """Rust 端按 UTF-8 解码 stdout。中文 Windows 上管道默认是 cp936，不改的话
    中文路径 / 系统报错会乱码，遇到 GBK 编不了的字符还会直接抛 UnicodeEncodeError。"""
    for stream in (sys.stdin, sys.stdout, sys.stderr):
        if isinstance(stream, io.TextIOWrapper):
            stream.reconfigure(encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    _force_utf8_stdio()
    parser = _build_parser()
    args = parser.parse_args(argv)
    if args.cmd == "launch-browser":
        return cmd_launch_browser(args)
    if args.cmd == "detect-tz":
        return cmd_detect_tz(args)
    if args.cmd == "derive-fp":
        return cmd_derive_fp(args)
    if args.cmd == "ping":
        return cmd_ping(args)
    parser.print_help(sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
