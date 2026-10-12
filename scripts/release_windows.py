#!/usr/bin/env python3
"""Windows 发布：sidecar → Chromium → tauri build，产出 NSIS 安装包（setup.exe）。

对应 macOS 的 scripts/release.sh。Chromium 通过 src-tauri/tauri.windows-release.conf.json
的 bundle.resources 打进安装包；这份配置只在这里用 --config 叠加，`pnpm tauri dev` 不会读到，
dev 态直接用工程里的 brower/win_x64。

用法（Windows，需要 Python 3.12+、Node 20+、pnpm、Rust MSVC 工具链）：
  python scripts\\release_windows.py
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
SCRIPTS = ROOT / "scripts"
TAURI_DIR = ROOT / "src-tauri"
# 上游只发布 x64 版 Chromium
TRIPLE = "x86_64-pc-windows-msvc"


def run(cmd: list[str | Path], **kwargs) -> None:
    subprocess.run([str(c) for c in cmd], check=True, cwd=ROOT, **kwargs)


def main() -> int:
    # Windows 上输出被重定向（如 CI 日志）时默认是 ANSI 代码页，打印中文会抛 UnicodeEncodeError
    for stream in (sys.stdout, sys.stderr):
        if isinstance(stream, io.TextIOWrapper):
            stream.reconfigure(encoding="utf-8")

    if sys.platform != "win32":
        raise SystemExit("[!] 只能在 Windows 上运行（macOS 用 bash scripts/release.sh）")
    # pnpm 在 Windows 上是 pnpm.cmd，必须用 which 解析出完整路径才能直接 spawn
    pnpm = shutil.which("pnpm")
    if not pnpm:
        raise SystemExit("[!] 找不到 pnpm")

    version = json.loads((TAURI_DIR / "tauri.conf.json").read_text(encoding="utf-8"))["version"]
    print(f"[*] Brio {version} / {TRIPLE}")

    print("[1/4] 打包 Python sidecar")
    run([sys.executable, SCRIPTS / "build_sidecar.py"], env={**os.environ, "TARGET_TRIPLE": TRIPLE})

    print("[2/4] 准备 Chromium")
    run([sys.executable, SCRIPTS / "fetch_chromium.py"])

    print("[3/4] 安装前端依赖")
    run([pnpm, "install", "--frozen-lockfile"])

    print("[4/4] tauri build（NSIS）")
    run([
        pnpm, "tauri", "build",
        "--target", TRIPLE,
        "--config", TAURI_DIR / "tauri.windows-release.conf.json",
    ])

    release = TAURI_DIR / "target" / TRIPLE / "release"
    # tauri-build 会把 resources 拷到可执行旁边，借此确认 Chromium 确实进了安装包
    if not (release / "chromium" / "chrome.exe").is_file():
        raise SystemExit(f"[!] {release / 'chromium'} 下没有 chrome.exe，Chromium 没打进安装包")
    installers = sorted((release / "bundle" / "nsis").glob("*-setup.exe"))
    if not installers:
        raise SystemExit("[!] 没找到 NSIS 安装包")

    print("[✓] 完成")
    for p in installers:
        print(f"  {p}  ({p.stat().st_size >> 20} MB)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
