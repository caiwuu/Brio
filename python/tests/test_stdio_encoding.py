"""sidecar 的 stdin/stdout/stderr 必须是 UTF-8（中文 Windows 管道默认 cp936）。"""

from __future__ import annotations

import io
import json
import os
import subprocess
import sys
from pathlib import Path

from brio import __main__ as sidecar_main
from brio.events import EventEmitter

PY_DIR = Path(__file__).resolve().parents[1]


def test_force_utf8_stdio_reconfigures_streams(monkeypatch) -> None:
    streams = {
        name: io.TextIOWrapper(io.BytesIO(), encoding="gbk")
        for name in ("stdin", "stdout", "stderr")
    }
    for name, stream in streams.items():
        monkeypatch.setattr(sys, name, stream)

    sidecar_main._force_utf8_stdio()

    for stream in streams.values():
        assert stream.encoding == "utf-8"
    sys.stdout.write("中文😀")
    sys.stdout.flush()
    raw = streams["stdout"].buffer
    assert isinstance(raw, io.BytesIO)
    assert raw.getvalue() == "中文😀".encode("utf-8")


def test_force_utf8_stdio_tolerates_missing_streams(monkeypatch) -> None:
    # pythonw / 无控制台时 sys.stdout 可能是 None
    monkeypatch.setattr(sys, "stdout", None)
    sidecar_main._force_utf8_stdio()


def test_sidecar_emits_utf8_under_gbk_locale(tmp_path: Path) -> None:
    """模拟中文 Windows：PYTHONIOENCODING=gbk 下中文 + GBK 编不了的 emoji 都要原样输出。"""
    chromium = "C:/不存在/😀/chrome.exe"
    env = {**os.environ, "PYTHONIOENCODING": "gbk", "PYTHONPATH": str(PY_DIR)}
    proc = subprocess.run(
        [
            sys.executable,
            "-m",
            "brio",
            "launch-browser",
            "--fingerprint-id",
            "1",
            "--chromium-bin",
            chromium,
            "--user-data-dir",
            str(tmp_path / "udd"),
        ],
        capture_output=True,
        env=env,
        timeout=30,
    )
    assert proc.returncode == 2, proc.stderr.decode("utf-8", "replace")
    event = json.loads(proc.stdout.decode("utf-8").strip().splitlines()[-1])
    assert event == {"event": "error", "message": f"chromium not found: {chromium}"}


def test_emitter_survives_broken_stdout() -> None:
    class Broken(io.StringIO):
        def write(self, s: str) -> int:
            raise BrokenPipeError(32, "Broken pipe")

    EventEmitter(Broken()).emit("log", message="still alive")
