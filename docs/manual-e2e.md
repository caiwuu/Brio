# 手动 E2E checklist

> 纯指纹浏览器的人工冒烟。每次合并前过一遍，2 分钟搞定。

## 0. 准备

- 若 `src-tauri/binaries/brio-py-*` 已是 PyInstaller 二进制且改过 Python，跑一次
  `bash scripts/build_sidecar.sh`；若是指向 `python -m brio` 的 shell wrapper 可跳过
- `pnpm tauri dev`
- 准备一个 socks5 代理（可以是本机 mitmproxy、shadowsocks 等）；下面以
  `socks5://127.0.0.1:1080` 示例（New browser 弹窗代理为必填）

## 1. Profile 生命周期

1. **Browsers** 页 → `New browser`
   - Proxy URL: `socks5://127.0.0.1:1080`
   - 时区：输入代理后应自动探测；也可点 **Auto**（代理不可用时会 fallback 默认时区）
   - Fingerprint ID 应随 (proxy, tz, lang) 自动派生
   - 可选勾选 **Launch immediately after create**
2. 创建后点 **Launch**（若未勾选立即启动）→ 等浏览器窗口出来
   - 列表里状态点变绿，Dashboard 的 Running browsers 出现该行
3. 进 Browser detail，确认 `Connection` 卡片：
   - Proxy / Timezone / Language / Fingerprint ID 都有值
   - 多了一行 `CDP ENDPOINT: http://127.0.0.1:<port>`
4. 点 **Close** → 状态回到 stopped
5. **Edit** 改 name / proxy / timezone / language / notes（fingerprint 不可改）→ 保存后详情页刷新
6. **Delete browser** → 列表消失，`browser_data/<fp>/` 目录被回收

## 2. Dashboard 概览

- 统计卡片数字与实际 profile / running 数量一致
- Recent profiles 里可直接 Launch / Close
- 左下角 BottomStatus 显示 "N browsers running"

## 3. Settings

- Storage 两行路径 **Reveal** 都能打开 Finder（Windows 上是资源管理器）
- **MCP Server**：默认关闭；Enable 后 endpoint / token 可见，可复制 Client config；
  Disable 后再测工具应连不上

通过条件：以上全绿，`pnpm tauri dev` 控制台无 Rust panic / 前端红错。
