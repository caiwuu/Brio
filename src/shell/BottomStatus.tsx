import { isTauriRuntime, useRunningBrowsers } from "@/lib/api";

export function BottomStatus() {
  const inTauri = isTauriRuntime();
  const running = useRunningBrowsers();
  const count = inTauri
    ? (running.data?.filter((r) => r.status === "running" || r.status === "starting").length ?? 0)
    : 0;
  const active = count > 0;
  const dotClass = active ? "bg-success" : "bg-ink-muted";
  return (
    <div className="border-t border-line px-3 py-2">
      <div className="flex items-center gap-2 rounded-md px-2 py-2">
        <span className={`relative inline-flex h-2 w-2 items-center justify-center rounded-full ${dotClass}`}>
          {active ? (
            <span className="absolute inline-flex h-2 w-2 animate-ping rounded-full bg-success/60" />
          ) : null}
        </span>
        <div className="flex min-w-0 flex-col leading-tight">
          <span className="truncate text-[12px] font-medium text-ink">
            {active ? `${count} browser${count > 1 ? "s" : ""} running` : "No browsers running"}
          </span>
          <span className="truncate text-[10px] text-ink-muted">
            {active ? "fingerprint sessions active" : "idle"}
          </span>
        </div>
      </div>
    </div>
  );
}
