import { AlertTriangle } from "lucide-react";

import { isTauriRuntime } from "@/lib/api";

/**
 * 在浏览器（非 Tauri）里跑 Vite 时显示的固定横幅，提醒走 `pnpm tauri dev`。
 * 在真正的 Tauri 窗口里返回 null，不影响布局。
 */
export function RuntimeBanner() {
  if (isTauriRuntime()) return null;
  return (
    <div className="flex items-center gap-2 border-b border-warning/40 bg-warning/15 px-4 py-2 text-xs text-warning-foreground">
      <AlertTriangle className="h-3.5 w-3.5 text-warning" />
      <span className="text-ink">
        Tauri runtime not detected. Use{" "}
        <code className="rounded bg-subtle px-1 font-mono text-[11px] text-ink">
          pnpm tauri dev
        </code>{" "}
        instead of <code className="rounded bg-subtle px-1 font-mono text-[11px] text-ink">pnpm dev</code> so
        the Rust backend (DB, sidecar, browser supervisor) is available.
      </span>
    </div>
  );
}
