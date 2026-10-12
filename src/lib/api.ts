/**
 * Tauri command 调用层。
 *
 * 命名约定：
 * - 类型用 PascalCase，与 Rust 端的 serde 字段保持一致（snake_case）。
 * - 每个 command 都暴露一个 thin wrapper（`apiX`）+ 一个 react-query hook（`useX`）。
 */

import {
  useMutation,
  useQuery,
  useQueryClient,
  type UseMutationOptions,
  type UseQueryOptions,
} from "@tanstack/react-query";
import { invoke as tauriInvoke } from "@tauri-apps/api/core";

/**
 * 检查是否在 Tauri runtime 内。
 * `pnpm dev` 跑出来的纯 Vite 页面不会有 `__TAURI_INTERNALS__`，所有 invoke 都会失败。
 * 这种情况下走 `pnpm tauri dev` 才行。
 */
export function isTauriRuntime(): boolean {
  if (typeof window === "undefined") return false;
  return Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}

function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    return Promise.reject(
      new Error(
        "Tauri runtime not available. Run `pnpm tauri dev` (not `pnpm dev`) so the desktop shell loads.",
      ),
    );
  }
  return tauriInvoke<T>(cmd, args);
}

// ---------------------------------------------------------------------------
// Browser
// ---------------------------------------------------------------------------

export interface Browser {
  id: string;
  name: string;
  proxy_url: string;
  timezone: string;
  language: string;
  fingerprint_id: string;
  notes: string;
  user_data_dir: string;
  last_opened_at: number | null;
  created_at: number;
  updated_at: number;
  /** 当前活跃 CDP 端口；0 表示未在跑或未启用 CDP。 */
  cdp_port: number;
}

export interface CreateBrowserInput {
  name: string;
  proxy_url: string;
  timezone: string;
  language: string;
  fingerprint_id: string;
  notes?: string;
}

/** fingerprint_id 不可更新：udd 目录按 fingerprint 共享，要换指纹请新建 browser。 */
export interface UpdateBrowserInput {
  name?: string;
  proxy_url?: string;
  timezone?: string;
  language?: string;
  notes?: string;
}

export const browsersApi = {
  list: () => invoke<Browser[]>("list_browsers"),
  get: (id: string) => invoke<Browser>("get_browser", { id }),
  create: (input: CreateBrowserInput) =>
    invoke<Browser>("create_browser", { input }),
  update: (id: string, input: UpdateBrowserInput) =>
    invoke<Browser>("update_browser", { id, input }),
  delete: (id: string) => invoke<void>("delete_browser", { id }),
};

export const browsersKeys = {
  all: ["browsers"] as const,
  detail: (id: string) => ["browsers", id] as const,
};

export function useBrowsers(
  options?: Omit<UseQueryOptions<Browser[]>, "queryKey" | "queryFn">
) {
  return useQuery({
    queryKey: browsersKeys.all,
    queryFn: browsersApi.list,
    ...options,
  });
}

export function useBrowser(
  id: string | undefined,
  options?: Omit<UseQueryOptions<Browser>, "queryKey" | "queryFn" | "enabled">
) {
  return useQuery({
    queryKey: id ? browsersKeys.detail(id) : ["browsers", "__none__"],
    queryFn: () => browsersApi.get(id as string),
    enabled: Boolean(id),
    ...options,
  });
}

export function useCreateBrowser(
  options?: UseMutationOptions<Browser, Error, CreateBrowserInput>
) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: browsersApi.create,
    onSuccess: (data, vars, ctx, mut) => {
      qc.invalidateQueries({ queryKey: browsersKeys.all });
      options?.onSuccess?.(data, vars, ctx, mut);
    },
    ...options,
  });
}

export function useUpdateBrowser(
  options?: UseMutationOptions<
    Browser,
    Error,
    { id: string; input: UpdateBrowserInput }
  >
) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }) => browsersApi.update(id, input),
    onSuccess: (data, vars, ctx, mut) => {
      qc.invalidateQueries({ queryKey: browsersKeys.all });
      qc.invalidateQueries({ queryKey: browsersKeys.detail(vars.id) });
      options?.onSuccess?.(data, vars, ctx, mut);
    },
    ...options,
  });
}

export function useDeleteBrowser(
  options?: UseMutationOptions<void, Error, string>
) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: browsersApi.delete,
    onSuccess: (data, id, ctx, mut) => {
      qc.invalidateQueries({ queryKey: browsersKeys.all });
      qc.removeQueries({ queryKey: browsersKeys.detail(id) });
      options?.onSuccess?.(data, id, ctx, mut);
    },
    ...options,
  });
}

// ---------------------------------------------------------------------------
// Browser runtime: open / close / list_running
// ---------------------------------------------------------------------------

export type BrowserStatus =
  | "starting"
  | "running"
  | "stopping"
  | "stopped"
  | "error";

export interface RunningBrowser {
  browser_id: string;
  status: BrowserStatus;
  started_at: number;
  pid: number | null;
  /** 当前 CDP 端口；0 表示未启用或未就绪 */
  cdp_port: number;
  /** http://127.0.0.1:<cdp_port>，未启用时为空串 */
  cdp_endpoint: string;
  /** 浏览器指纹 ID。同一时刻同一指纹只允许一个 running 实例。 */
  fingerprint_id: string;
}

export interface OpenBrowserInput {
  open_urls?: string[];
}

export const browserRuntimeApi = {
  open: (id: string, input?: OpenBrowserInput) =>
    invoke<RunningBrowser>("open_browser", { id, input: input ?? null }),
  close: (id: string) => invoke<void>("close_browser", { id }),
  listRunning: () => invoke<RunningBrowser[]>("list_running_browsers"),
};

export const runningKeys = {
  all: ["browsers", "__running__"] as const,
};

export function useRunningBrowsers() {
  return useQuery({
    queryKey: runningKeys.all,
    queryFn: browserRuntimeApi.listRunning,
    refetchInterval: 5_000,
  });
}

export function useOpenBrowser(
  options?: UseMutationOptions<
    RunningBrowser,
    Error,
    { id: string; input?: OpenBrowserInput }
  >
) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }) => browserRuntimeApi.open(id, input),
    onSuccess: (data, vars, ctx, mut) => {
      qc.invalidateQueries({ queryKey: runningKeys.all });
      qc.invalidateQueries({ queryKey: browsersKeys.all });
      qc.invalidateQueries({ queryKey: browsersKeys.detail(vars.id) });
      options?.onSuccess?.(data, vars, ctx, mut);
    },
    ...options,
  });
}

export function useCloseBrowser(
  options?: UseMutationOptions<void, Error, string>
) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: browserRuntimeApi.close,
    onSuccess: (data, id, ctx, mut) => {
      qc.invalidateQueries({ queryKey: runningKeys.all });
      qc.invalidateQueries({ queryKey: browsersKeys.all });
      qc.invalidateQueries({ queryKey: browsersKeys.detail(id) });
      options?.onSuccess?.(data, id, ctx, mut);
    },
    ...options,
  });
}

// ---------------------------------------------------------------------------
// Sidecar 一次性命令
// ---------------------------------------------------------------------------

export interface DetectTzResult {
  timezone: string;
  detected: boolean;
}

export interface DeriveFpResult {
  fingerprint_id: string;
}

export const sidecarApi = {
  detectTimezone: (proxyUrl?: string) =>
    invoke<DetectTzResult>("detect_timezone", {
      args: { proxy_url: proxyUrl ?? "" },
    }),
  deriveFingerprintId: (input: {
    proxy_url: string;
    timezone: string;
    language: string;
  }) => invoke<DeriveFpResult>("derive_fingerprint_id", { args: input }),
  defaultChromiumPath: () => invoke<string>("default_chromium_path"),
};

export interface AppPaths {
  data_dir: string;
  browser_data_dir: string;
}

export const pathsApi = {
  appPaths: () => invoke<AppPaths>("get_app_paths"),
};

// ---------------------------------------------------------------------------
// MCP server 设置
// ---------------------------------------------------------------------------

export interface McpStatus {
  enabled: boolean;
  port: number;
  token: string;
  /** http://127.0.0.1:<port>/mcp */
  endpoint: string;
  /** server 当前是否在监听 */
  running: boolean;
}

export interface SetMcpSettingsInput {
  enabled: boolean;
  port?: number;
}

export const mcpApi = {
  get: () => invoke<McpStatus>("get_mcp_settings"),
  set: (input: SetMcpSettingsInput) =>
    invoke<McpStatus>("set_mcp_settings", { input }),
  regenerateToken: () => invoke<McpStatus>("regenerate_mcp_token"),
};

export const mcpKeys = {
  all: ["mcp-settings"] as const,
};

export function useMcpSettings(
  options?: Omit<UseQueryOptions<McpStatus>, "queryKey" | "queryFn">,
) {
  return useQuery({
    queryKey: mcpKeys.all,
    queryFn: mcpApi.get,
    ...options,
  });
}

export function useSetMcpSettings(
  options?: UseMutationOptions<McpStatus, Error, SetMcpSettingsInput>,
) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: mcpApi.set,
    onSuccess: (data, vars, ctx, mut) => {
      qc.setQueryData(mcpKeys.all, data);
      options?.onSuccess?.(data, vars, ctx, mut);
    },
    ...options,
  });
}

export function useRegenerateMcpToken(
  options?: UseMutationOptions<McpStatus, Error, void>,
) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => mcpApi.regenerateToken(),
    onSuccess: (data, vars, ctx, mut) => {
      qc.setQueryData(mcpKeys.all, data);
      options?.onSuccess?.(data, vars, ctx, mut);
    },
    ...options,
  });
}

export function useAppPaths(
  options?: Omit<UseQueryOptions<AppPaths>, "queryKey" | "queryFn">,
) {
  return useQuery({
    queryKey: ["app-paths"] as const,
    queryFn: pathsApi.appPaths,
    staleTime: Infinity,
    ...options,
  });
}

export function useDetectTimezone() {
  return useMutation({
    mutationFn: (proxyUrl?: string) => sidecarApi.detectTimezone(proxyUrl),
  });
}

export function useDeriveFingerprintId() {
  return useMutation({
    mutationFn: (input: {
      proxy_url: string;
      timezone: string;
      language: string;
    }) => sidecarApi.deriveFingerprintId(input),
  });
}
