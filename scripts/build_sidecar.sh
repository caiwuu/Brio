#!/usr/bin/env bash
# 用 PyInstaller 把 python/brio 打包成单文件可执行，
# 输出到 src-tauri/binaries/brio-py-<target-triple>。
# 具体逻辑在 build_sidecar.py（跨平台，Windows 直接用它），这里只是保留原来的入口。
#
# 依赖：python3 (>=3.12), pip 可用，可联网装 pyinstaller。
# 用法：bash scripts/build_sidecar.sh

set -euo pipefail

exec python3 "$(dirname "$0")/build_sidecar.py" "$@"
