import { useEffect, useState } from "react";
import {
  Copy,
  Database,
  Eye,
  EyeOff,
  FolderOpen,
  Plug,
  RefreshCw,
} from "lucide-react";
import { toast } from "sonner";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { showError } from "@/lib/toast";

import { TopBar } from "@/shell/TopBar";
import {
  isTauriRuntime,
  useAppPaths,
  useMcpSettings,
  useRegenerateMcpToken,
  useSetMcpSettings,
} from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import {
  MCP_CONFIG_FORMATS,
  mcpConfigFormat,
  mcpConfigJson,
} from "@/lib/mcp-config";

async function copyToClipboard(text: string, label: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success(`${label} copied`);
  } catch (e) {
    toast.error(`Copy failed: ${(e as Error).message ?? e}`);
  }
}

export function SettingsPage() {
  return (
    <>
      <TopBar title="Settings" subtitle="Storage locations" />
      <div className="flex-1 overflow-y-auto">
        <div className="mx-auto flex max-w-4xl flex-col gap-10 px-8 py-8">
          <StorageSection />
          <McpSection />
        </div>
      </div>
    </>
  );
}

// ---------------------------------------------------------------------------
// 通用 Section 头部
// ---------------------------------------------------------------------------
function SectionHeader({
  title,
  description,
}: {
  title: string;
  description: string;
}) {
  return (
    <div className="mb-5 flex items-end justify-between gap-4">
      <div className="min-w-0">
        <h2 className="text-lg font-semibold text-ink">{title}</h2>
        <p className="mt-1 text-sm text-ink-secondary">{description}</p>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------
function StorageSection() {
  const inTauri = isTauriRuntime();
  const paths = useAppPaths({ enabled: inTauri });

  async function handleReveal(path: string) {
    try {
      await revealItemInDir(path);
    } catch (e) {
      toast.error(`Reveal failed: ${(e as Error).message ?? e}`);
    }
  }

  const rows: { label: string; description: string; path?: string }[] = [
    {
      label: "Data folder",
      description: "App root: db, sidecar state and everything below.",
      path: paths.data?.data_dir,
    },
    {
      label: "Browser profiles",
      description: "One user-data-dir per fingerprint.",
      path: paths.data?.browser_data_dir,
    },
  ];

  return (
    <section>
      <SectionHeader
        title="Storage"
        description="Where browser profiles and app data live on this machine."
      />

      <div className="overflow-hidden rounded-xl border border-line bg-surface">
        {rows.map((row, idx) => (
          <div
            key={row.label}
            className={cn(
              "flex items-center gap-3 px-5 py-4",
              idx < rows.length - 1 && "border-b border-line",
            )}
          >
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-brand-tint text-brand">
              <Database className="h-4 w-4" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="text-sm font-medium text-ink">{row.label}</div>
              <p className="mt-0.5 text-xs text-ink-secondary">
                {row.description}
              </p>
              {row.path ? (
                <code className="mt-1 block truncate font-mono text-[11px] text-ink-muted">
                  {row.path}
                </code>
              ) : null}
            </div>
            <Button
              variant="outline"
              size="sm"
              type="button"
              disabled={!row.path}
              onClick={() => row.path && handleReveal(row.path)}
              className="shrink-0"
            >
              <FolderOpen className="h-3.5 w-3.5" />
              Reveal
            </Button>
          </div>
        ))}
        {paths.error ? (
          <div className="border-t border-danger/30 bg-danger/5 px-5 py-3 text-xs text-danger">
            Failed to load paths: {(paths.error as Error).message}
          </div>
        ) : null}
      </div>
    </section>
  );
}

// ---------------------------------------------------------------------------
// MCP Server
// ---------------------------------------------------------------------------
function McpSection() {
  const inTauri = isTauriRuntime();
  const query = useMcpSettings({ enabled: inTauri });
  const setSettings = useSetMcpSettings();
  const regenerate = useRegenerateMcpToken();

  const status = query.data;
  const [portInput, setPortInput] = useState("");
  const [revealToken, setRevealToken] = useState(false);
  const [configFormat, setConfigFormat] = useState("cursor");

  useEffect(() => {
    if (status) setPortInput(String(status.port));
  }, [status?.port]);

  const busy = setSettings.isPending || regenerate.isPending;

  function handleToggle() {
    if (!status) return;
    const port = Number(portInput) || status.port;
    setSettings.mutate(
      { enabled: !status.enabled, port },
      {
        onSuccess: (d) =>
          toast.success(d.enabled ? "MCP server started" : "MCP server stopped"),
        onError: (e) => showError(e, "Failed to start MCP server"),
      },
    );
  }

  function handleApplyPort() {
    if (!status) return;
    const port = Number(portInput);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      toast.error("Port must be between 1 and 65535");
      return;
    }
    setSettings.mutate(
      { enabled: status.enabled, port },
      {
        onSuccess: () => toast.success("Port saved"),
        onError: (e) => showError(e, "Failed to save MCP port"),
      },
    );
  }

  function handleRegenerate() {
    regenerate.mutate(undefined, {
      onSuccess: () => toast.success("Token regenerated"),
      onError: (e) => showError(e, "Failed to regenerate token"),
    });
  }

  const portChanged = status ? portInput !== String(status.port) : false;
  const maskedToken = status
    ? `${status.token.slice(0, 6)}${"•".repeat(18)}${status.token.slice(-4)}`
    : "";

  return (
    <section>
      <SectionHeader
        title="MCP Server"
        description="Expose your browsers to AI agents over a local Model Context Protocol endpoint."
      />

      <div className="overflow-hidden rounded-xl border border-line bg-surface">
        {/* Enable row */}
        <div className="flex items-center gap-3 border-b border-line px-5 py-4">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-brand-tint text-brand">
            <Plug className="h-4 w-4" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2 text-sm font-medium text-ink">
              Local MCP endpoint
              {status ? (
                status.running ? (
                  <Badge variant="success">Running</Badge>
                ) : (
                  <Badge variant="secondary">Stopped</Badge>
                )
              ) : null}
            </div>
            <p className="mt-0.5 text-xs text-ink-secondary">
              Serves all browser management &amp; page-control tools on
              127.0.0.1, protected by a bearer token.
            </p>
          </div>
          <Button
            variant={status?.enabled ? "outline" : "default"}
            size="sm"
            type="button"
            disabled={!inTauri || !status || busy}
            onClick={handleToggle}
            className="shrink-0"
          >
            {status?.enabled ? "Disable" : "Enable"}
          </Button>
        </div>

        {/* Port row */}
        <div className="flex items-center gap-3 border-b border-line px-5 py-4">
          <div className="min-w-0 flex-1">
            <div className="text-sm font-medium text-ink">Port</div>
            <p className="mt-0.5 text-xs text-ink-secondary">
              Changing the port restarts the server when enabled.
            </p>
          </div>
          <Input
            value={portInput}
            onChange={(e) => setPortInput(e.target.value.replace(/[^0-9]/g, ""))}
            disabled={!inTauri || busy}
            inputMode="numeric"
            className="h-8 w-28 font-mono text-xs"
          />
          <Button
            variant="outline"
            size="sm"
            type="button"
            disabled={!inTauri || !status || busy || !portChanged}
            onClick={handleApplyPort}
            className="shrink-0"
          >
            Apply
          </Button>
        </div>

        {/* Endpoint row */}
        <div className="flex items-center gap-3 border-b border-line px-5 py-4">
          <div className="min-w-0 flex-1">
            <div className="text-sm font-medium text-ink">Endpoint</div>
            <code className="mt-1 block truncate font-mono text-[11px] text-ink-muted">
              {status?.endpoint ?? "—"}
            </code>
          </div>
          <Button
            variant="outline"
            size="sm"
            type="button"
            disabled={!status}
            onClick={() => status && copyToClipboard(status.endpoint, "Endpoint")}
            className="shrink-0"
          >
            <Copy className="h-3.5 w-3.5" />
            Copy
          </Button>
        </div>

        {/* Token row */}
        <div className="flex items-center gap-3 border-b border-line px-5 py-4">
          <div className="min-w-0 flex-1">
            <div className="text-sm font-medium text-ink">Access token</div>
            <code className="mt-1 block truncate font-mono text-[11px] text-ink-muted">
              {status ? (revealToken ? status.token : maskedToken) : "—"}
            </code>
          </div>
          <Button
            variant="ghost"
            size="icon"
            type="button"
            disabled={!status}
            onClick={() => setRevealToken((v) => !v)}
            className="shrink-0"
            title={revealToken ? "Hide token" : "Reveal token"}
          >
            {revealToken ? (
              <EyeOff className="h-3.5 w-3.5" />
            ) : (
              <Eye className="h-3.5 w-3.5" />
            )}
          </Button>
          <Button
            variant="outline"
            size="sm"
            type="button"
            disabled={!status}
            onClick={() => status && copyToClipboard(status.token, "Token")}
            className="shrink-0"
          >
            <Copy className="h-3.5 w-3.5" />
            Copy
          </Button>
          <Button
            variant="outline"
            size="sm"
            type="button"
            disabled={!inTauri || !status || busy}
            onClick={handleRegenerate}
            className="shrink-0"
          >
            <RefreshCw className="h-3.5 w-3.5" />
            Regenerate
          </Button>
        </div>

        {/* Config snippet row */}
        <div className="px-5 py-4">
          <div className="mb-3 flex items-start justify-between gap-3">
            <div className="min-w-0">
              <div className="text-sm font-medium text-ink">Client config</div>
              <p className="mt-0.5 text-xs text-ink-secondary">
                Pick the format your host understands. Riot and Claude Desktop
                need the stdio snippet — a bare <code>url</code> is rejected.
              </p>
            </div>
            <Button
              variant="outline"
              size="sm"
              type="button"
              disabled={!status}
              onClick={() =>
                status &&
                copyToClipboard(
                  mcpConfigJson(status, configFormat),
                  `${mcpConfigFormat(configFormat).label} config`,
                )
              }
              className="shrink-0"
            >
              <Copy className="h-3.5 w-3.5" />
              Copy
            </Button>
          </div>
          <div className="mb-3 flex flex-wrap gap-1.5">
            {MCP_CONFIG_FORMATS.map((fmt) => (
              <button
                key={fmt.id}
                type="button"
                onClick={() => setConfigFormat(fmt.id)}
                className={cn(
                  "h-7 rounded-md border px-2.5 text-[11px] font-medium transition-colors",
                  configFormat === fmt.id
                    ? "border-brand bg-brand-tint text-brand"
                    : "border-line bg-background text-ink-secondary hover:bg-subtle hover:text-ink",
                )}
              >
                {fmt.label}
              </button>
            ))}
          </div>
          {status ? (
            <>
              <p className="mb-1 font-mono text-[10px] text-ink-muted">
                {mcpConfigFormat(configFormat).file}
              </p>
              <p className="mb-2 text-xs text-ink-secondary">
                {mcpConfigFormat(configFormat).hint}
              </p>
              <pre className="overflow-x-auto rounded-lg border border-line bg-subtle px-4 py-3 font-mono text-[11px] leading-relaxed text-ink-muted">
                {mcpConfigJson(status, configFormat)}
              </pre>
            </>
          ) : (
            <pre className="overflow-x-auto rounded-lg border border-line bg-subtle px-4 py-3 font-mono text-[11px] leading-relaxed text-ink-muted">
              —
            </pre>
          )}
        </div>

        {query.error ? (
          <div className="border-t border-danger/30 bg-danger/5 px-5 py-3 text-xs text-danger">
            Failed to load MCP settings: {(query.error as Error).message}
          </div>
        ) : null}
      </div>
    </section>
  );
}
