#!/usr/bin/env python3
"""用 PyInstaller 把 python/brio 打包成单文件可执行，
输出到 src-tauri/binaries/brio-py-<target-triple>（Windows 带 .exe）。

依赖：python3 (>=3.12)，可联网装 pyinstaller。
用法：python scripts/build_sidecar.py（macOS 也可以继续用 bash scripts/build_sidecar.sh）
可以用环境变量 TARGET_TRIPLE 显式指定 triple，但必须和打包用的 Python 架构一致。
"""

from __future__ import annotations

import io
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PY_DIR = ROOT / "python"
BIN_DIR = ROOT / "src-tauri" / "binaries"
VENV = ROOT / ".venv-sidecar"
DIST = ROOT / ".pyinstaller-dist"
WORK = ROOT / ".pyinstaller-build"

PYINSTALLER = "pyinstaller==6.22.3"
HIDDEN_IMPORTS = [
    "brio",
    "brio.core",
    "brio.browser_close",
    "brio.browser_session",
    "brio.events",
    "brio.local_proxy_forwarder",
]

# 在打包用的解释器里求 triple：PyInstaller 只能打出这个解释器自己的架构。
# macOS 上 platform.machine() 反映进程实际架构（Rosetta 下是 x86_64）；
# Windows 上它返回的是系统架构，32 位 Python 也是 AMD64，所以改看 sysconfig。
_TRIPLE_PROBE = """
import platform, sys, sysconfig
if sys.platform == "darwin":
    arch = {"arm64": "aarch64", "x86_64": "x86_64"}.get(platform.machine())
    print(f"{arch}-apple-darwin" if arch else "")
elif sys.platform == "win32":
    arch = {"win-amd64": "x86_64", "win-arm64": "aarch64"}.get(sysconfig.get_platform())
    print(f"{arch}-pc-windows-msvc" if arch else "")
else:
    print(f"{platform.machine()}-unknown-linux-gnu")
"""


def run(cmd: list[str | Path], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run([str(c) for c in cmd], check=True, **kwargs)


def venv_python() -> Path:
    if sys.platform == "win32":
        return VENV / "Scripts" / "python.exe"
    return VENV / "bin" / "python"


def interpreter_triple(python: Path) -> str:
    out = run([python, "-c", _TRIPLE_PROBE], capture_output=True, text=True).stdout.strip()
    if not out:
        raise SystemExit(f"[!] 不支持的架构：{python}")
    return out


def smoke_test(binary: Path) -> None:
    """跑一次 `ping`，确认打出来的可执行能启动、事件协议正常。"""
    # Windows 首次启动要把 onefile 解压到临时目录，杀毒软件扫描时会慢一些
    proc = subprocess.run([str(binary), "ping"], capture_output=True, timeout=120)
    lines = proc.stdout.decode("utf-8", "replace").strip().splitlines()
    try:
        event = json.loads(lines[-1])
    except (IndexError, ValueError):
        event = None
    if proc.returncode != 0 or not isinstance(event, dict) or event.get("event") != "pong":
        sys.stderr.write(proc.stderr.decode("utf-8", "replace"))
        raise SystemExit(f"[!] 自检失败：exit={proc.returncode} stdout={lines[-1:]}")
    print(f"[*] 自检通过：{lines[-1]}")


def main() -> int:
    # Windows 上输出被重定向（如 CI 日志）时默认是 ANSI 代码页，打印中文会抛 UnicodeEncodeError
    for stream in (sys.stdout, sys.stderr):
        if isinstance(stream, io.TextIOWrapper):
            stream.reconfigure(encoding="utf-8")

    # 1. 准备一个独立 venv，避免污染用户环境。
    py = venv_python()
    if not py.exists():
        print(f"[*] 创建虚拟环境 {VENV}")
        run([sys.executable, "-m", "venv", VENV])
    run([py, "-m", "pip", "install", "--quiet", "--upgrade", "pip"])
    run([py, "-m", "pip", "install", "--quiet", PYINSTALLER])

    # 2. 解析目标 triple，必须和 venv 解释器的架构一致。
    host = interpreter_triple(py)
    triple = os.environ.get("TARGET_TRIPLE") or host
    if triple != host:
        raise SystemExit(
            f"[!] TARGET_TRIPLE={triple}，但 {py} 是 {host}；"
            "PyInstaller 不能交叉打包，请换对应架构的 Python 重建 .venv-sidecar"
        )
    windows = "windows" in triple
    out_name = f"brio-py-{triple}"
    out_file = BIN_DIR / (out_name + (".exe" if windows else ""))
    print(f"[*] target triple: {triple}")
    print(f"[*] 输出文件:     {out_file}")

    # 3. 调 PyInstaller。--onefile 体积友好；--noconfirm 避免交互。
    #    --noupx：UPX 压缩过的 exe 更容易被杀毒软件误报。
    shutil.rmtree(DIST, ignore_errors=True)
    shutil.rmtree(WORK, ignore_errors=True)
    cmd: list[str | Path] = [
        py, "-m", "PyInstaller",
        "--name", out_name,
        "--onefile",
        "--noconfirm",
        "--clean",
        "--console",
        "--noupx",
        "--paths", PY_DIR,
    ]
    for module in HIDDEN_IMPORTS:
        cmd += ["--hidden-import", module]
    cmd += ["--distpath", DIST, "--workpath", WORK, "--specpath", WORK, "brio/__main__.py"]
    run(cmd, cwd=PY_DIR)

    # 4. 移到 src-tauri/binaries。
    BIN_DIR.mkdir(parents=True, exist_ok=True)
    os.replace(DIST / out_file.name, out_file)
    if not windows:
        out_file.chmod(0o755)

    # 5. ad-hoc 签名（macOS 启动需要）。
    if sys.platform == "darwin":
        subprocess.run(
            ["codesign", "--force", "--sign", "-", str(out_file)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

    # 清理中间产物
    shutil.rmtree(DIST, ignore_errors=True)
    shutil.rmtree(WORK, ignore_errors=True)

    smoke_test(out_file)
    print(f"[✓] sidecar 打包完成: {out_file}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
