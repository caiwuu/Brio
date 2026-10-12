#!/usr/bin/env bash
# 一键 release: sidecar 二进制 + tauri build + 嵌入 Chromium + ad-hoc 签名 + dmg。
#
# 产物在 src-tauri/target/<triple>/release/bundle/{macos,dmg}/。

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# 强制按真机 CPU 架构选 target，避免 rustc 在 Rosetta 下默认 x86_64。
case "$(uname -m)" in
  arm64)  export TARGET_TRIPLE="aarch64-apple-darwin" ;;
  x86_64) export TARGET_TRIPLE="x86_64-apple-darwin" ;;
  *) echo "[!] 不支持的架构 $(uname -m)" >&2; exit 1 ;;
esac
echo "[*] target triple: $TARGET_TRIPLE"

echo "============================================================"
echo "[1/5] PyInstaller 打 sidecar 二进制"
echo "============================================================"
bash scripts/build_sidecar.sh

echo
echo "============================================================"
echo "[2/5] tauri build (前端 + .app)"
echo "============================================================"
pnpm install >/dev/null
pnpm tauri build --target "$TARGET_TRIPLE"

echo
echo "============================================================"
echo "[3/5] 嵌入 Chromium.app (cp -R 保留 symlink)"
echo "============================================================"
bash scripts/embed_chromium.sh

echo
echo "============================================================"
echo "[4/5] ad-hoc 签名 .app"
echo "============================================================"
bash scripts/codesign_app.sh

echo
echo "============================================================"
echo "[5/5] hdiutil 打 dmg"
echo "============================================================"
bash scripts/make_dmg.sh

echo
echo "[✓] 完成。产物目录："
ls -1 "$ROOT/src-tauri/target/$TARGET_TRIPLE/release/bundle"/*/  2>/dev/null | sed 's/^/  /'
