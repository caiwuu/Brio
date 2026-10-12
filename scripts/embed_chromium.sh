#!/usr/bin/env bash
# 把 brower/mac_armx64/Chromium.app 嵌入到 tauri build 出来的 .app 里。
#
# 必须用 `cp -R` (而不是让 tauri 的 bundle.resources 处理)，否则 macOS framework
# 的 symlink 结构 (Versions/Current, 顶层 Chromium Framework / Helpers / Libraries
# / Resources) 会被全部展开成实体副本，导致 Chromium.app 从 273MB 膨胀到 800+MB。
#
# 用法：bash scripts/embed_chromium.sh [path/to/Brio.app]

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/brower/mac_armx64/Chromium.app"
APP_PATH="${1:-}"

if [[ ! -d "$SRC" ]]; then
  echo "[!] 找不到源 Chromium: $SRC" >&2
  exit 1
fi

if [[ -z "$APP_PATH" ]]; then
  CANDIDATES=(
    "$ROOT/src-tauri/target/${TARGET_TRIPLE:-aarch64-apple-darwin}/release/bundle/macos"
    "$ROOT/src-tauri/target/release/bundle/macos"
  )
  for d in "${CANDIDATES[@]}"; do
    if [[ -d "$d" ]]; then
      APP_PATH="$(find "$d" -maxdepth 1 -name '*.app' -print -quit 2>/dev/null || true)"
      [[ -n "$APP_PATH" ]] && break
    fi
  done
fi

if [[ -z "$APP_PATH" || ! -d "$APP_PATH" ]]; then
  echo "[!] 找不到目标 .app，请先 pnpm tauri build，或手动传入路径" >&2
  exit 1
fi

DST_DIR="$APP_PATH/Contents/Resources/resources/chromium"

echo "[*] 目标: $DST_DIR"
mkdir -p "$DST_DIR"
rm -rf "$DST_DIR/Chromium.app"

echo "[*] cp -R Chromium.app (保留 symlink)"
cp -R "$SRC" "$DST_DIR/"

SIZE="$(du -sh "$DST_DIR/Chromium.app" | awk '{print $1}')"
echo "[✓] 嵌入完成，大小 $SIZE (源 $(du -sh "$SRC" | awk '{print $1}'))"
