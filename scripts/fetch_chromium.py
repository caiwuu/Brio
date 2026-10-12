#!/usr/bin/env python3
"""下载上游 fingerprint-chromium，放到开发 / 打包读取的位置：

- macOS：brower/mac_armx64/Chromium.app（挂载 dmg 后用 ditto 复制，保留 framework 的 symlink）
- Windows：brower/win_x64/chrome.exe 所在目录（解压 zip，把 chrome.exe 所在目录平铺进去）

目标已存在时跳过；要换版本先删掉对应目录。下载会校验 sha256。

用法：
  python scripts/fetch_chromium.py                    # 按当前平台下载
  python scripts/fetch_chromium.py --print-version    # 打印版本号（CI 缓存 key 用）
  python scripts/fetch_chromium.py --archive <文件>   # 用已下载好的 dmg / zip
"""

from __future__ import annotations

import argparse
import hashlib
import io
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request
import zipfile
from pathlib import Path

# 升级 Chromium 只改这里：版本号 + 两个安装包的 sha256。
CHROMIUM_VERSION = "142.0.7444.175-1.1"
SHA256 = {
    "mac": "ae3349a2d6368406543a84e43c783d019ac0743bf4fe08f6391933d207a65696",
    "win": "c99406fe2822cd6b7281b8eb54469bb59b2f1ec13e8d4d0f1f16e871fa25e7ab",
}

RELEASE_URL = "https://github.com/adryfish/fingerprint-chromium/releases/download"
ASSETS = {
    "mac": f"ungoogled-chromium_{CHROMIUM_VERSION}_macos.dmg",
    "win": f"ungoogled-chromium_{CHROMIUM_VERSION}_windows_x64.zip",
}

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DEST = {
    "mac": ROOT / "brower" / "mac_armx64",
    "win": ROOT / "brower" / "win_x64",
}
MAC_APP = "Chromium.app"
MAC_EXE = Path(MAC_APP, "Contents", "MacOS", "Chromium")
WIN_EXE = "chrome.exe"


def asset_url(plat: str) -> str:
    tag = CHROMIUM_VERSION.rsplit("-", 1)[0]
    return f"{RELEASE_URL}/{tag}/{ASSETS[plat]}"


def current_platform() -> str:
    if sys.platform == "darwin":
        return "mac"
    if sys.platform == "win32":
        return "win"
    raise SystemExit(f"[!] 不支持的平台 {sys.platform}（只有 macOS / Windows 版 Chromium）")


def sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def download(url: str, out: Path) -> None:
    print(f"[*] 下载 {url}")
    try:
        with urllib.request.urlopen(url, timeout=60) as resp, open(out, "wb") as f:
            total = int(resp.headers.get("Content-Length") or 0)
            done, next_report = 0, 0.1
            while chunk := resp.read(1 << 20):
                f.write(chunk)
                done += len(chunk)
                if total and done / total >= next_report:
                    print(f"    {done / total:4.0%}  {done >> 20} / {total >> 20} MB")
                    next_report += 0.1
    except OSError as e:
        raise SystemExit(f"[!] 下载失败：{e}\n    可以手动下载上面的地址，再用 --archive 指定") from e


def rename_with_retry(src: Path, dst: Path) -> None:
    # Windows 上杀毒软件刚扫描完新文件时会短暂占用，目录改名可能报 PermissionError
    for attempt in range(10):
        try:
            src.rename(dst)
            return
        except PermissionError:
            if attempt == 9:
                raise
            time.sleep(1)


def extract_dmg(archive: Path, dest_app: Path) -> None:
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        mnt = Path(tmp) / "mnt"
        mnt.mkdir()
        subprocess.run(
            ["hdiutil", "attach", "-nobrowse", "-readonly", "-noautoopen",
             "-mountpoint", str(mnt), str(archive)],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        try:
            src = mnt / MAC_APP
            if not src.is_dir():
                raise SystemExit(f"[!] dmg 里没有 {MAC_APP}")
            # ditto 保留 symlink / 扩展属性；Chromium Framework 靠 Versions/Current 等 symlink 组织
            subprocess.run(["ditto", str(src), str(dest_app)], check=True)
        finally:
            if subprocess.run(["hdiutil", "detach", str(mnt), "-quiet"]).returncode != 0:
                subprocess.run(["hdiutil", "detach", str(mnt), "-force", "-quiet"])


def extract_zip(archive: Path, dest: Path) -> None:
    """把 chrome.exe 所在目录平铺到 dest；跳过 *.pdb（调试符号，运行用不到）。"""
    with zipfile.ZipFile(archive) as zf:
        exes = [n for n in zf.namelist() if n.rsplit("/", 1)[-1].lower() == WIN_EXE]
        if not exes:
            raise SystemExit(f"[!] zip 里没有 {WIN_EXE}")
        exe = min(exes, key=len)
        prefix = exe[: len(exe) - len(WIN_EXE)]
        for info in zf.infolist():
            name = info.filename
            if info.is_dir() or not name.startswith(prefix):
                continue
            rel = name[len(prefix):]
            parts = rel.split("/")
            if rel.startswith("/") or ".." in parts:
                raise SystemExit(f"[!] zip 里有可疑路径：{name}")
            if rel.lower().endswith(".pdb"):
                continue
            out = dest.joinpath(*parts)
            out.parent.mkdir(parents=True, exist_ok=True)
            with zf.open(info) as src, open(out, "wb") as f:
                shutil.copyfileobj(src, f, 1 << 20)


def main(argv: list[str] | None = None) -> int:
    # Windows 上输出被重定向（如 CI 日志）时默认是 ANSI 代码页，打印中文会抛 UnicodeEncodeError
    for stream in (sys.stdout, sys.stderr):
        if isinstance(stream, io.TextIOWrapper):
            stream.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser(description="下载上游 fingerprint-chromium")
    parser.add_argument("--print-version", action="store_true", help="只打印版本号")
    parser.add_argument("--platform", choices=["mac", "win"], help="默认按当前平台")
    parser.add_argument("--dest", type=Path, help="目标目录（默认 brower/mac_armx64 或 brower/win_x64）")
    parser.add_argument("--archive", type=Path, help="使用已下载好的 dmg / zip，不再联网")
    args = parser.parse_args(argv)

    if args.print_version:
        print(CHROMIUM_VERSION)
        return 0

    plat = args.platform or current_platform()
    dest: Path = (args.dest or DEFAULT_DEST[plat]).resolve()
    if plat == "mac":
        final, exe = dest / MAC_APP, dest / MAC_EXE
    else:
        final, exe = dest, dest / WIN_EXE

    if exe.is_file():
        print(f"[=] 已存在，跳过：{exe}（要重新下载请先删掉 {final}）")
        return 0
    if final.exists():
        raise SystemExit(f"[!] {final} 已存在但不完整（缺 {exe.name}），请手动删除后重试")

    final.parent.mkdir(parents=True, exist_ok=True)
    # 先解压到同级临时目录再改名，中途失败不会留下半成品被下次误判为"已存在"
    staging = final.parent / f".{final.name}.partial"
    shutil.rmtree(staging, ignore_errors=True)

    archive: Path = args.archive.resolve() if args.archive else final.parent / f".download-{ASSETS[plat]}"
    try:
        if not args.archive:
            download(asset_url(plat), archive)
        actual = sha256_of(archive)
        if actual != SHA256[plat]:
            raise SystemExit(
                f"[!] sha256 不匹配：{archive}\n    期望 {SHA256[plat]}\n    实际 {actual}"
            )
        print(f"[*] 解压到 {final}")
        if plat == "mac":
            extract_dmg(archive, staging)
        else:
            staging.mkdir()
            extract_zip(archive, staging)
        rename_with_retry(staging, final)
    finally:
        shutil.rmtree(staging, ignore_errors=True)
        if not args.archive:
            archive.unlink(missing_ok=True)

    print(f"[✓] Chromium {CHROMIUM_VERSION} 就绪：{exe}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
