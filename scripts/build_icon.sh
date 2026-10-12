#!/usr/bin/env bash
# 生成 macOS 风格的应用图标：渐变圆角底 + 白色 fingerprint。
# 输出（与 tauri.conf.json bundle.icon 对齐）：
#   src-tauri/icons/icon.png            (1024)
#   src-tauri/icons/icon.icns           (多尺寸 macOS 图标)
#   src-tauri/icons/32x32.png
#   src-tauri/icons/128x128.png
#   src-tauri/icons/128x128@2x.png      (256)
#   src-tauri/icons/icon.ico            (Windows，16~256 多尺寸)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICON_DIR="$ROOT/src-tauri/icons"
TMP_DIR="$(mktemp -d)"
SVG="$TMP_DIR/icon.svg"
PNG_BASE="$TMP_DIR/icon-1024.png"
ICONSET="$TMP_DIR/icon.iconset"

mkdir -p "$ICON_DIR" "$ICONSET"

# 紫色圆角整体缩到画布的 82%，四周透明，程序坞里不再铺满。
# 组内仍按 1024 设计：圆角约 230，指纹 24x24 放大 30 倍。
cat > "$SVG" <<'SVG'
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1024" y2="1024" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#6366F1"/>
      <stop offset="0.55" stop-color="#8B5CF6"/>
      <stop offset="1" stop-color="#EC4899"/>
    </linearGradient>
    <linearGradient id="hi" x1="0" y1="0" x2="0" y2="1024" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.28"/>
      <stop offset="0.5" stop-color="#FFFFFF" stop-opacity="0"/>
    </linearGradient>
    <radialGradient id="glow" cx="512" cy="512" r="430" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.18"/>
      <stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
    </radialGradient>
    <filter id="shadow" x="-20%" y="-20%" width="140%" height="140%">
      <feGaussianBlur in="SourceAlpha" stdDeviation="14"/>
      <feOffset dy="10" result="off"/>
      <feComponentTransfer><feFuncA type="linear" slope="0.32"/></feComponentTransfer>
      <feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge>
    </filter>
  </defs>

  <g transform="translate(92.16 92.16) scale(0.82)">
    <!-- 圆角底 + 渐变 -->
    <rect x="0" y="0" width="1024" height="1024" rx="230" ry="230" fill="url(#bg)"/>
    <!-- 顶部高光 -->
    <rect x="0" y="0" width="1024" height="1024" rx="230" ry="230" fill="url(#hi)"/>
    <!-- 中央柔光 -->
    <rect x="0" y="0" width="1024" height="1024" rx="230" ry="230" fill="url(#glow)"/>

    <!-- 内圆托底，弱化指纹与背景的对比，更耐看 -->
    <circle cx="512" cy="512" r="320" fill="#FFFFFF" fill-opacity="0.10"/>
    <circle cx="512" cy="512" r="320" fill="none" stroke="#FFFFFF" stroke-opacity="0.18" stroke-width="2"/>

    <!-- fingerprint：tabler:fingerprint 等比放大并居中。
         原始 24x24，放大 30 倍 = 720x720，居中偏移 (152, 152)。 -->
    <g transform="translate(152, 152) scale(30)" fill="none" stroke="#FFFFFF"
       stroke-linecap="round" stroke-linejoin="round" stroke-width="1.45" filter="url(#shadow)">
      <path d="M18.9 7a8 8 0 0 1 1.1 5v1a6 6 0 0 0 .8 3M8 11a4 4 0 0 1 8 0v1a10 10 0 0 0 2 6"/>
      <path d="M12 11v2a14 14 0 0 0 2.5 8M8 15a18 18 0 0 0 1.8 6m-4.9-2a22 22 0 0 1-.9-7v-1a8 8 0 0 1 12-6.95"/>
    </g>
  </g>
</svg>
SVG

echo "[*] 渲染 1024 PNG"
sips -s format png "$SVG" --out "$PNG_BASE" >/dev/null

# 拷给 iconset 用
cp "$PNG_BASE" "$ICONSET/icon_512x512@2x.png"
sips -z 512 512   "$PNG_BASE" --out "$ICONSET/icon_512x512.png"     >/dev/null
sips -z 512 512   "$PNG_BASE" --out "$ICONSET/icon_256x256@2x.png"  >/dev/null
sips -z 256 256   "$PNG_BASE" --out "$ICONSET/icon_256x256.png"     >/dev/null
sips -z 256 256   "$PNG_BASE" --out "$ICONSET/icon_128x128@2x.png"  >/dev/null
sips -z 128 128   "$PNG_BASE" --out "$ICONSET/icon_128x128.png"     >/dev/null
sips -z 64 64     "$PNG_BASE" --out "$ICONSET/icon_32x32@2x.png"    >/dev/null
sips -z 32 32     "$PNG_BASE" --out "$ICONSET/icon_32x32.png"       >/dev/null
sips -z 32 32     "$PNG_BASE" --out "$ICONSET/icon_16x16@2x.png"    >/dev/null
sips -z 16 16     "$PNG_BASE" --out "$ICONSET/icon_16x16.png"       >/dev/null

echo "[*] iconutil 合成 .icns"
iconutil -c icns "$ICONSET" -o "$ICON_DIR/icon.icns"

cp "$PNG_BASE" "$ICON_DIR/icon.png"
sips -z 32   32   "$PNG_BASE" --out "$ICON_DIR/32x32.png"     >/dev/null
sips -z 128  128  "$PNG_BASE" --out "$ICON_DIR/128x128.png"   >/dev/null
sips -z 256  256  "$PNG_BASE" --out "$ICON_DIR/128x128@2x.png" >/dev/null

echo "[*] 生成 Windows .ico"
# tauri icon 会生成一整套图标，输出到临时目录，只取 icon.ico，不覆盖上面的产物
pnpm --dir "$ROOT" tauri icon "$PNG_BASE" -o "$TMP_DIR/tauri-icons" >/dev/null
cp "$TMP_DIR/tauri-icons/icon.ico" "$ICON_DIR/icon.ico"

rm -rf "$TMP_DIR"

echo "[✓] 完成"
ls -lh "$ICON_DIR"
