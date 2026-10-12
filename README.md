# Brio

Brio 是一个桌面端指纹浏览器（macOS Apple Silicon / Windows x64）：管理多个相互隔离的浏览器 profile，每个 profile 有独立的代理、时区、语言和浏览器指纹，并内嵌 MCP server 供智能体调用。

- `src/`：前端，React + Vite + TypeScript + shadcn/ui
- `src-tauri/`：Tauri 2（Rust）端，负责 profile 存储、浏览器进程托管和内嵌 MCP server（`src-tauri/src/mcp/`）
- `python/brio/`：Python sidecar，负责探测时区、转发代理并启动 Chromium（纯标准库）
- `brower/`：上游 [fingerprint-chromium](https://github.com/adryfish/fingerprint-chromium)，mac 版在 `mac_armx64/Chromium.app`，Windows 版在 `win_x64/`；不进 git，用 `python scripts/fetch_chromium.py` 下载
- `scripts/`：下载 Chromium、打包 sidecar、嵌入 Chromium、签名和生成 DMG 的脚本
- `.github/workflows/release.yml`：在 GitHub Actions 上构建两个平台的安装包

## 使用

在 **Browsers** 页点 **New browser** 新建 profile：填入上游代理（`socks5://user:pass@host:port`），时区会按代理自动探测，指纹 ID 由代理、时区和语言自动派生。点 **Launch** 打开对应的指纹 Chromium，同一指纹同一时刻只允许一个运行实例；运行中的浏览器会开放一个仅本机可达的 CDP 端口（详情页可见），供外部自动化工具和 MCP 使用。

数据保存在 `~/Library/Application Support/com.brio.desktop/`（Windows 上是 `%LOCALAPPDATA%\com.brio.desktop\`）：`data.sqlite3` 存 profile 和应用设置，`browser_data/<指纹 ID>/` 是每个指纹独立的 Chromium user-data-dir。

内置 Chromium 和 sidecar 只有 ad-hoc 签名。首次打开如果被 Gatekeeper 拦截，执行下面的命令，或在 **系统设置 → 隐私与安全性** 里点“仍要打开”：

```bash
xattr -dr com.apple.quarantine "/Applications/Brio.app"
```

Windows 安装包没有代码签名，SmartScreen 拦截时点 **更多信息 → 仍要运行**。

## MCP Server

Brio 内嵌一个应用级 MCP server，外部智能体通过这一个入口管理所有 profile，并按 `browser_id` 操作指定浏览器的页面。

- 端点：`http://127.0.0.1:<port>/mcp`（streamable HTTP，默认端口 `9333`）
- 鉴权：请求头 `Authorization: Bearer <token>`，token 首次启动时自动生成
- 开启：默认关闭，在 **Settings → MCP Server** 中开启；这里也可以改端口、复制 endpoint 和 token、重新生成 token

**Settings → MCP Server → Client config** 可以按客户端（Cursor、VS Code、Cline / Roo、Claude Code 等）复制对应格式的配置。以 Cursor 为例，把 `brio` 这一段合并进 `~/.cursor/mcp.json` 或项目里的 `.cursor/mcp.json`：

```json
{
  "mcpServers": {
    "brio": {
      "url": "http://127.0.0.1:9333/mcp",
      "headers": { "Authorization": "Bearer <your-token>" }
    }
  }
}
```

只支持 stdio 的客户端（如 Claude Desktop、Riot）需要用 `npx mcp-remote` 把 HTTP 桥接成 stdio，在 Client config 里选 **stdio / Riot** 即可得到对应配置。

工具共 20 个：

- 管理：`list_browsers`、`get_browser`、`create_browser`、`update_browser`、`delete_browser`、`open_browser`、`close_browser`、`detect_timezone`
- 页面操作（目标浏览器需已 `open_browser`）：`list_tabs`、`new_tab`、`close_tab`、`navigate`、`snapshot`、`click`、`type_text`、`press_key`、`scroll`、`wait_for`、`screenshot`、`evaluate`

安全方面，server 只监听 `127.0.0.1` 并强制校验 Bearer token；token 以明文保存在本机的 `data.sqlite3` 里，工具返回中的代理凭证会打码。

## 开发

需要 Node 20+、pnpm、Rust 工具链和 Python 3.12+。

```bash
python scripts/fetch_chromium.py   # 首次：下载当前平台的 Chromium 到 brower/
pnpm install
pnpm tauri dev
```

dev 模式下 sidecar 来自 `src-tauri/binaries/brio-py-<triple>`：如果它是调用 `python -m brio` 的 shell wrapper，改 Python 代码后即时生效；如果是 PyInstaller 二进制，需要重跑 `python scripts/build_sidecar.py`。Windows 上没有 wrapper，首次 `pnpm tauri dev` 前必须先跑一次这个脚本。Chromium 由 `src-tauri/src/paths.rs` 自动定位到 `brower/` 下对应平台的目录，无需复制。

sidecar 通过 stdout 输出 NDJSON 事件、通过 stdin 接收 JSON 命令，字段以 `python/brio/events.py`、`python/brio/browser_session.py` 和 `src-tauri/src/browser_supervisor.rs` 为准。

## 打包 .app / .dmg

依赖与开发环境相同，并且 `brower/mac_armx64/Chromium.app` 必须存在（用 `python scripts/fetch_chromium.py` 下载）。

```bash
bash scripts/release.sh
```

脚本按本机架构选择 target triple，然后依次执行：

1. `scripts/build_sidecar.sh`：用 PyInstaller 把 sidecar 打成单文件二进制
2. `pnpm tauri build --target <triple>`：构建前端和 `Brio.app`
3. `scripts/embed_chromium.sh`：把 Chromium 复制进 `Brio.app`
4. `scripts/codesign_app.sh`：ad-hoc 签名
5. `scripts/make_dmg.sh`：用 `hdiutil` 生成 DMG

各步骤的细节和设计理由见脚本注释。产物位于 `src-tauri/target/<triple>/release/bundle/`，以 Apple Silicon 为例：

```
src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Brio.app
src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/Brio_0.1.0_aarch64.dmg
```

Apple Silicon 上首次打包前先执行 `rustup target add aarch64-apple-darwin`；`release.sh` 按 `uname -m` 选择 target，避免在 Rosetta 下打出 x86_64 的包。

## Windows 安装包与 CI 发布

在 Windows 上本地打包（PowerShell 或 Git Bash，需要 Node、pnpm、Python 3.12+，以及 MSVC 版 Rust 工具链和 Visual Studio C++ 生成工具）：

```bash
python scripts/release_windows.py
```

它依次打包 sidecar、下载 Chromium、执行 `tauri build`。Chromium 通过 `src-tauri/tauri.windows-release.conf.json` 只在发布时打进安装包，避免 `tauri dev` 每次都复制。产物是 `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/Brio_<版本>_x64-setup.exe`，默认按当前用户安装，不需要管理员权限。

CI 发布：先把 `src-tauri/tauri.conf.json` 的 `version` 改成新版本，再推送同名标签，例如 `git tag v0.1.0 && git push origin v0.1.0`。GitHub Actions 会分别构建 DMG 和 Windows 安装包，并创建一个 draft Release，确认后手动发布。在 Actions 页面手动运行 **Release** 工作流则只构建，安装包在本次运行的 Artifacts 里下载。升级 Chromium 时只需改 `scripts/fetch_chromium.py` 顶部的版本号和 sha256。
