# UI 约定（当前实现）

创建浏览器时：

- 代理：界面按 **SOCKS5 URL** 收集，且为必填（形如 `socks5://user:pass@host:port`）
- 时区：输入代理后自动探测；也可点 **Auto** 手动探测；探测失败用默认时区
- 语言：下拉选择；显式点 Auto 时会按探测到的时区联动推荐语言
- 指纹 id：只读，由 (proxy, timezone, language) 自动派生
- 可选 **Launch immediately after create**

不再提供「导出 identity / 选择 provider」流程；需要自动化时走 Settings 里的 MCP Server 或详情页 CDP endpoint。
