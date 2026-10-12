#!/usr/bin/env bash
# 用 hdiutil 给 Brio.app 自己造一个 .dmg。
# 绕开 Tauri 自带的 bundle_dmg.sh（它在 macOS 15 + Apple Silicon 上对参数处理时常挂）。
#
# 用法：bash scripts/make_dmg.sh

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TRIPLE="${TARGET_TRIPLE:-aarch64-apple-darwin}"
BUNDLE_DIR="$ROOT/src-tauri/target/$TRIPLE/release/bundle"
APP="$BUNDLE_DIR/macos/Brio.app"

if [[ ! -d "$APP" ]]; then
  echo "[!] 找不到 .app: $APP" >&2
  exit 1
fi

# 读 version
VERSION="$(grep -m1 '"version"' "$ROOT/src-tauri/tauri.conf.json" | sed -E 's/.*"version"[^"]*"([^"]+)".*/\1/')"
ARCH_LABEL="aarch64"
[[ "$TRIPLE" == "x86_64-apple-darwin" ]] && ARCH_LABEL="x64"

DMG_DIR="$BUNDLE_DIR/dmg"
mkdir -p "$DMG_DIR"
DMG_OUT="$DMG_DIR/Brio_${VERSION}_${ARCH_LABEL}.dmg"
rm -f "$DMG_OUT"

STAGING="$(mktemp -d)/dmg-staging"
mkdir -p "$STAGING"
cp -R "$APP" "$STAGING/"
ln -s /Applications "$STAGING/Applications"

echo "[*] 构建 dmg: $DMG_OUT"
# CI 的 macOS 机器上 hdiutil 偶发 "Resource busy"，重试几次
for attempt in 1 2 3; do
  if hdiutil create \
    -volname "Brio" \
    -srcfolder "$STAGING" \
    -ov \
    -format UDZO \
    -fs HFS+ \
    "$DMG_OUT"; then
    break
  fi
  [[ $attempt -eq 3 ]] && exit 1
  echo "[!] hdiutil 失败，5 秒后重试（$attempt/3）" >&2
  sleep 5
done

# ad-hoc 签 dmg
codesign --force --sign - --timestamp=none "$DMG_OUT" >/dev/null 2>&1 || true

rm -rf "$STAGING"

echo "[✓] 完成: $DMG_OUT"
ls -lh "$DMG_OUT"
