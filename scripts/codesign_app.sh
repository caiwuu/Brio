#!/usr/bin/env bash
# 给 tauri build 输出的 .app 做 ad-hoc 签名，确保本机能直接打开。
# 不做 notarize；要分发请改用真实 Developer ID 证书。
#
# 用法：bash scripts/codesign_app.sh [path/to/Brio.app]

set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)/.."
ROOT="$(cd "$ROOT" && pwd)"
APP_PATH="${1:-}"

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
  echo "[!] 找不到 .app，请先 pnpm tauri build，或手动传入路径" >&2
  exit 1
fi

echo "[*] 签名 $APP_PATH"

sign() {
  local target="$1"
  if [[ -e "$target" ]]; then
    codesign --force --sign - --timestamp=none "$target" >/dev/null 2>&1 || \
      echo "[!] 签名失败 (忽略): $target" >&2
  fi
}

# Chromium 必须按 内 -> 外 顺序签：Helper apps -> Framework -> Chromium.app
INNER_CHROMIUM="$APP_PATH/Contents/Resources/resources/chromium/Chromium.app"
if [[ -d "$INNER_CHROMIUM" ]]; then
  echo "[*] 签内嵌 Chromium 各 Helper / Framework"
  FW_HELPERS_DIR="$INNER_CHROMIUM/Contents/Frameworks/Chromium Framework.framework/Helpers"
  if [[ -d "$FW_HELPERS_DIR" ]]; then
    while IFS= read -r helper; do
      sign "$helper"
    done < <(find "$FW_HELPERS_DIR" -maxdepth 1 -name '*.app')
  fi

  FW_VERSIONS="$INNER_CHROMIUM/Contents/Frameworks/Chromium Framework.framework/Versions"
  if [[ -d "$FW_VERSIONS" ]]; then
    while IFS= read -r ver; do
      [[ "$(basename "$ver")" == "Current" ]] && continue
      sign "$ver"
    done < <(find "$FW_VERSIONS" -mindepth 1 -maxdepth 1 -type d)
  fi

  for fw in "$INNER_CHROMIUM/Contents/Frameworks/"*.framework; do
    [[ -d "$fw" ]] || continue
    [[ "$(basename "$fw")" == "Chromium Framework.framework" ]] && continue
    sign "$fw"
  done

  echo "[*] 签 Chromium.app 顶层"
  sign "$INNER_CHROMIUM"
fi

INNER_SIDECAR="$APP_PATH/Contents/MacOS/brio-py"
if [[ -f "$INNER_SIDECAR" ]]; then
  echo "[*] 签 sidecar"
  sign "$INNER_SIDECAR"
fi

echo "[*] 签外层 Brio.app"
sign "$APP_PATH"

# 校验外层（不强制深度校验，Chromium 内部允许 ad-hoc）
codesign --verify --verbose=2 "$APP_PATH" || {
  echo "[!] 外层校验失败" >&2
  exit 1
}

xattr -dr com.apple.quarantine "$APP_PATH" 2>/dev/null || true

echo "[✓] 签名完成"
