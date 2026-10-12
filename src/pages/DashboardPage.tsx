import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import {
  ChevronRight,
  Fingerprint,
  Globe,
  Loader2,
  Play,
  Plus,
  Shield,
  Square,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { formatDistanceToNow } from "date-fns";
import { toast } from "sonner";

import { TopBar } from "@/shell/TopBar";
import { NewBrowserDialog } from "@/components/browsers/NewBrowserDialog";
import { StatusDot } from "@/pages/BrowserDetailPage";
import { Button } from "@/components/ui/button";
import {
  isTauriRuntime,
  useBrowsers,
  useCloseBrowser,
  useOpenBrowser,
  useRunningBrowsers,
  type Browser,
  type RunningBrowser,
} from "@/lib/api";

export function DashboardPage() {
  const inTauri = isTauriRuntime();
  const [creating, setCreating] = useState(false);

  const browsers = useBrowsers({ enabled: inTauri });
  const running = useRunningBrowsers();
  const openBrowser = useOpenBrowser();
  const closeBrowser = useCloseBrowser();

  const browserById = useMemo(() => {
    const m = new Map<string, Browser>();
    for (const b of browsers.data ?? []) m.set(b.id, b);
    return m;
  }, [browsers.data]);

  const live = useMemo(
    () =>
      (running.data ?? []).filter(
        (r) => r.status === "running" || r.status === "starting",
      ),
    [running.data],
  );

  const recent = useMemo(() => {
    return [...(browsers.data ?? [])]
      .sort((a, b) => (b.last_opened_at ?? 0) - (a.last_opened_at ?? 0))
      .slice(0, 5);
  }, [browsers.data]);

  const total = browsers.data?.length ?? 0;
  const withProxy = useMemo(
    () => (browsers.data ?? []).filter((b) => b.proxy_url.trim() !== "").length,
    [browsers.data],
  );

  async function handleOpen(id: string) {
    try {
      await openBrowser.mutateAsync({ id });
    } catch (e) {
      toast.error(`Open failed: ${(e as Error).message ?? e}`);
    }
  }

  async function handleClose(id: string) {
    try {
      await closeBrowser.mutateAsync(id);
    } catch (e) {
      toast.error(`Close failed: ${(e as Error).message ?? e}`);
    }
  }

  return (
    <>
      <TopBar
        title="Dashboard"
        subtitle={`${total} profile${total === 1 ? "" : "s"} · ${live.length} running`}
        actions={
          <Button size="sm" onClick={() => setCreating(true)}>
            <Plus className="h-3.5 w-3.5" /> New browser
          </Button>
        }
      />

      <div className="flex-1 overflow-y-auto p-6">
        <div className="mx-auto flex max-w-5xl flex-col gap-6">
          {/* 统计卡片 */}
          <div className="grid grid-cols-3 gap-4">
            <StatCard
              icon={Globe}
              label="Browser profiles"
              value={total}
              hint="isolated Chromium instances"
            />
            <StatCard
              icon={Zap}
              label="Running now"
              value={live.length}
              hint="active fingerprint sessions"
              highlight={live.length > 0}
            />
            <StatCard
              icon={Shield}
              label="Behind proxy"
              value={withProxy}
              hint="profiles with a proxy configured"
            />
          </div>

          {/* 运行中的浏览器 */}
          <Section
            title="Running browsers"
            action={
              live.length > 0 ? (
                <span className="text-[11px] text-ink-muted">
                  {live.length} live
                </span>
              ) : null
            }
          >
            {live.length === 0 ? (
              <EmptyHint
                icon={Fingerprint}
                title="Nothing running"
                body="Launch a profile from the list below — each one opens as an isolated Chromium with its own fingerprint."
              />
            ) : (
              <ul className="divide-y divide-line">
                {live.map((r) => (
                  <RunningRow
                    key={r.browser_id}
                    runtime={r}
                    browser={browserById.get(r.browser_id)}
                    onClose={() => handleClose(r.browser_id)}
                    closing={
                      closeBrowser.isPending &&
                      closeBrowser.variables === r.browser_id
                    }
                  />
                ))}
              </ul>
            )}
          </Section>

          {/* 最近的 profile */}
          <Section
            title="Recent profiles"
            action={
              total > 0 ? (
                <Link
                  to="/browsers"
                  className="flex items-center gap-0.5 text-[11px] font-medium text-brand hover:underline"
                >
                  View all <ChevronRight className="h-3 w-3" />
                </Link>
              ) : null
            }
          >
            {browsers.isLoading ? (
              <div className="space-y-2 p-5">
                {Array.from({ length: 3 }).map((_, i) => (
                  <div
                    key={i}
                    className="h-10 animate-pulse rounded-md bg-subtle"
                  />
                ))}
              </div>
            ) : total === 0 ? (
              <EmptyHint
                icon={Globe}
                title="No profiles yet"
                body="Create your first browser profile with its own proxy, timezone, language and fingerprint."
                action={
                  <Button size="sm" onClick={() => setCreating(true)}>
                    <Plus className="h-3.5 w-3.5" /> New browser
                  </Button>
                }
              />
            ) : (
              <ul className="divide-y divide-line">
                {recent.map((b) => {
                  const rt = live.find((r) => r.browser_id === b.id);
                  return (
                    <RecentRow
                      key={b.id}
                      browser={b}
                      isLive={Boolean(rt)}
                      onOpen={() => handleOpen(b.id)}
                      onClose={() => handleClose(b.id)}
                      opening={
                        openBrowser.isPending &&
                        openBrowser.variables?.id === b.id
                      }
                      closing={
                        closeBrowser.isPending &&
                        closeBrowser.variables === b.id
                      }
                    />
                  );
                })}
              </ul>
            )}
          </Section>
        </div>
      </div>

      <NewBrowserDialog open={creating} onOpenChange={setCreating} />
    </>
  );
}

// ---------------------------------------------------------------------------
// 组件
// ---------------------------------------------------------------------------

function StatCard({
  icon: Icon,
  label,
  value,
  hint,
  highlight,
}: {
  icon: LucideIcon;
  label: string;
  value: number;
  hint: string;
  highlight?: boolean;
}) {
  return (
    <div className="rounded-xl border border-line bg-surface p-5">
      <div className="flex items-center gap-3">
        <div
          className={`grid h-9 w-9 place-items-center rounded-md ${
            highlight ? "bg-success/10 text-success" : "bg-brand-tint text-brand"
          }`}
        >
          <Icon className="h-4 w-4" />
        </div>
        <div className="min-w-0">
          <div className="text-2xl font-semibold leading-tight text-ink">
            {value}
          </div>
          <div className="text-[11px] font-semibold uppercase tracking-[1px] text-ink-muted">
            {label}
          </div>
        </div>
      </div>
      <p className="mt-3 text-xs text-ink-muted">{hint}</p>
    </div>
  );
}

function Section({
  title,
  action,
  children,
}: {
  title: string;
  action?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <section className="rounded-xl border border-line bg-surface">
      <header className="flex items-center justify-between border-b border-line px-5 py-3">
        <h3 className="text-[11px] font-semibold uppercase tracking-[1.2px] text-ink-muted">
          {title}
        </h3>
        {action}
      </header>
      {children}
    </section>
  );
}

function RunningRow({
  runtime,
  browser,
  onClose,
  closing,
}: {
  runtime: RunningBrowser;
  browser: Browser | undefined;
  onClose: () => void;
  closing: boolean;
}) {
  return (
    <li className="flex items-center gap-3 px-5 py-3">
      <StatusDot status={runtime.status} />
      <div className="min-w-0 flex-1">
        <Link
          to={`/browsers/${runtime.browser_id}`}
          className="text-sm font-medium text-ink hover:text-brand"
        >
          {browser?.name || "unnamed"}
        </Link>
        <div className="flex items-center gap-2 text-[11px] text-ink-muted">
          <span className="font-mono">{runtime.fingerprint_id}</span>
          <span aria-hidden>·</span>
          <span>
            started{" "}
            {formatDistanceToNow(new Date(runtime.started_at * 1000), {
              addSuffix: true,
            })}
          </span>
          {runtime.cdp_port ? (
            <>
              <span aria-hidden>·</span>
              <span className="font-mono">cdp :{runtime.cdp_port}</span>
            </>
          ) : null}
        </div>
      </div>
      <Button
        variant="outline"
        size="sm"
        className="h-7"
        onClick={onClose}
        disabled={closing}
      >
        {closing ? (
          <Loader2 className="h-3 w-3 animate-spin" />
        ) : (
          <Square className="h-3 w-3" />
        )}
        Close
      </Button>
    </li>
  );
}

function RecentRow({
  browser,
  isLive,
  onOpen,
  onClose,
  opening,
  closing,
}: {
  browser: Browser;
  isLive: boolean;
  onOpen: () => void;
  onClose: () => void;
  opening: boolean;
  closing: boolean;
}) {
  return (
    <li className="flex items-center gap-3 px-5 py-3">
      <StatusDot status={isLive ? "running" : "stopped"} />
      <div className="min-w-0 flex-1">
        <Link
          to={`/browsers/${browser.id}`}
          className="text-sm font-medium text-ink hover:text-brand"
        >
          {browser.name || "unnamed"}
        </Link>
        <div className="flex items-center gap-2 text-[11px] text-ink-muted">
          <span className="font-mono">{browser.fingerprint_id}</span>
          {browser.timezone ? (
            <>
              <span aria-hidden>·</span>
              <span>{browser.timezone}</span>
            </>
          ) : null}
          <span aria-hidden>·</span>
          <span>
            {browser.last_opened_at
              ? `opened ${formatDistanceToNow(
                  new Date(browser.last_opened_at * 1000),
                  { addSuffix: true },
                )}`
              : "never opened"}
          </span>
        </div>
      </div>
      {isLive ? (
        <Button
          variant="outline"
          size="sm"
          className="h-7"
          onClick={onClose}
          disabled={closing}
        >
          {closing ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <Square className="h-3 w-3" />
          )}
          Close
        </Button>
      ) : (
        <Button
          variant="outline"
          size="sm"
          className="h-7"
          onClick={onOpen}
          disabled={opening}
        >
          {opening ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <Play className="h-3 w-3" />
          )}
          Launch
        </Button>
      )}
    </li>
  );
}

function EmptyHint({
  icon: Icon,
  title,
  body,
  action,
}: {
  icon: LucideIcon;
  title: string;
  body: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex flex-col items-center gap-2 px-6 py-10 text-center">
      <div className="grid h-10 w-10 place-items-center rounded-full bg-brand-tint text-brand">
        <Icon className="h-5 w-5" />
      </div>
      <div className="text-sm font-medium text-ink">{title}</div>
      <p className="max-w-sm text-xs text-ink-muted">{body}</p>
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  );
}
