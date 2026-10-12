/**
 * 各家 MCP 客户端的配置模板。
 *
 * Brio 本机只提供 streamable HTTP（Bearer token）。
 * 只认 stdio 的客户端（Claude Desktop、Riot 等）通过 `npx mcp-remote` 桥过去。
 */

import type { McpStatus } from "@/lib/api";

export interface McpConfigFormat {
  id: string;
  /** 短标签，用在切换按钮上 */
  label: string;
  /** 一行说明：给谁用、贴到哪 */
  hint: string;
  /** 配置文件路径提示 */
  file: string;
}

function pretty(value: unknown): string {
  return JSON.stringify(value, null, 2);
}

function bearer(status: McpStatus): string {
  return `Bearer ${status.token}`;
}

/** 无空格写法，避免部分客户端拆 args 时把 `Bearer xxx` 切成两段。 */
function authHeaderArg(status: McpStatus): string {
  return `Authorization:${bearer(status)}`;
}

export const MCP_CONFIG_FORMATS: McpConfigFormat[] = [
  {
    id: "cursor",
    label: "Cursor",
    hint: "Streamable HTTP. Merge into mcpServers — do not replace the whole file.",
    file: "~/.cursor/mcp.json  ·  .cursor/mcp.json",
  },
  {
    id: "stdio",
    label: "stdio / Riot",
    hint: "For clients that only accept command + args (Riot, Claude Desktop, older hosts). Bridges HTTP via npx mcp-remote.",
    file: "claude_desktop_config.json  ·  any stdio-only host",
  },
  {
    id: "vscode",
    label: "VS Code",
    hint: "Root key is servers, not mcpServers. Type http tries streamable HTTP then SSE.",
    file: ".vscode/mcp.json",
  },
  {
    id: "cline",
    label: "Cline / Roo",
    hint: "Uses type streamableHttp. Same shape works in many OpenClaw-style hosts.",
    file: "cline_mcp_settings.json  ·  roo mcp settings",
  },
  {
    id: "claude-code",
    label: "Claude Code",
    hint: "HTTP transport with an explicit type field.",
    file: "~/.claude.json  ·  .claude/settings.json",
  },
];

export function mcpConfigJson(status: McpStatus, formatId: string): string {
  const auth = { Authorization: bearer(status) };

  switch (formatId) {
    case "vscode":
      return pretty({
        servers: {
          brio: {
            type: "http",
            url: status.endpoint,
            headers: auth,
          },
        },
      });

    case "stdio":
      return pretty({
        mcpServers: {
          brio: {
            command: "npx",
            args: [
              "-y",
              "mcp-remote",
              status.endpoint,
              "--header",
              authHeaderArg(status),
            ],
          },
        },
      });

    case "cline":
      return pretty({
        mcpServers: {
          brio: {
            type: "streamableHttp",
            url: status.endpoint,
            headers: auth,
          },
        },
      });

    case "claude-code":
      return pretty({
        mcpServers: {
          brio: {
            type: "http",
            url: status.endpoint,
            headers: auth,
          },
        },
      });

    case "cursor":
    default:
      return pretty({
        mcpServers: {
          brio: {
            url: status.endpoint,
            headers: auth,
          },
        },
      });
  }
}

export function mcpConfigFormat(id: string): McpConfigFormat {
  return MCP_CONFIG_FORMATS.find((f) => f.id === id) ?? MCP_CONFIG_FORMATS[0];
}
