import { useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  ArrowLeft,
  ChevronRight,
  Loader2,
  Pencil,
  Play,
  Square,
  Trash2,
} from "lucide-react";
import { formatDistanceToNow } from "date-fns";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { useConfirm } from "@/components/ui/confirm-dialog";
import { TopBar } from "@/shell/TopBar";
import { EditBrowserDialog } from "@/components/browsers/EditBrowserDialog";
import {
  useBrowser,
  useCloseBrowser,
  useDeleteBrowser,
  useOpenBrowser,
  useRunningBrowsers,
  type Browser,
  type BrowserStatus,
  type RunningBrowser,
} from "@/lib/api";
import { useBrowserEventLog } from "@/hooks/useBrowserEvents";

export function BrowserDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const browser = useBrowser(id);
  const running = useRunningBrowsers();
  const open = useOpenBrowser();
  const close = useCloseBrowser();
  const del = useDeleteBrowser();
  const confirm = useConfirm();
  const eventLog = useBrowserEventLog(id);
  const [editOpen, setEditOpen] = useState(false);

  const runtime = useMemo<RunningBrowser | null>(() => {
    if (!id) return null;
    return running.data?.find((r) => r.browser_id === id) ?? null;
  }, [id, running.data]);

  if (browser.isLoading) {
    return (
      <>
        <TopBar
          title="Loading…"
          back={
            <Link
              to="/browsers"
              className="grid h-8 w-8 place-items-center rounded-md text-ink-secondary hover:bg-subtle"
            >
              <ArrowLeft className="h-4 w-4" />
            </Link>
          }
        />
        <div className="flex flex-1 items-center justify-center text-sm text-ink-muted">
          <Loader2 className="mr-2 h-4 w-4 animate-spin" /> loading browser…
        </div>
      </>
    );
  }
  if (browser.error || !browser.data) {
    return (
      <>
        <TopBar title="Browser not found" />
        <div className="flex flex-1 items-center justify-center text-sm text-danger">
          {(browser.error as Error | undefined)?.message ?? "not found"}
        </div>
      </>
    );
  }

  const b = browser.data;
  const status: BrowserStatus = runtime?.status ?? "stopped";
  const isBusy = status === "starting" || status === "stopping";
  const isLive = status === "running";

  async function handleOpen() {
    if (!id) return;
    try {
      await open.mutateAsync({ id });
    } catch (e) {
      toast.error(`Open failed: ${(e as Error).message ?? e}`);
    }
  }
  async function handleClose() {
    if (!id) return;
    try {
      await close.mutateAsync(id);
    } catch (e) {
      toast.error(`Close failed: ${(e as Error).message ?? e}`);
    }
  }
  async function handleDelete() {
    if (!id) return;
    const ok = await confirm({
      title: `Delete "${b.name}"?`,
      description: "This also removes its user-data-dir.",
      confirmText: "Delete",
      destructive: true,
    });
    if (!ok) return;
    try {
      await del.mutateAsync(id);
      toast.success(`Deleted ${b.name}`);
      navigate("/browsers");
    } catch (e) {
      toast.error(`Delete failed: ${(e as Error).message ?? e}`);
    }
  }

  return (
    <>
      <TopBar
        title={b.name || "(unnamed)"}
        subtitle={
          <Crumbs status={status} pid={runtime?.pid ?? null} udd={b.user_data_dir} />
        }
        back={
          <Link
            to="/browsers"
            className="grid h-8 w-8 place-items-center rounded-md text-ink-secondary hover:bg-subtle"
          >
            <ArrowLeft className="h-4 w-4" />
          </Link>
        }
        actions={
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setEditOpen(true)}
              title="Rename or update notes"
            >
              <Pencil className="h-3.5 w-3.5" /> Edit
            </Button>
            {isLive || status === "stopping" ? (
              <Button
                variant="outline"
                size="sm"
                onClick={handleClose}
                disabled={close.isPending || isBusy}
              >
                {close.isPending ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Square className="h-3.5 w-3.5" />
                )}
                Close
              </Button>
            ) : (
              <Button size="sm" onClick={handleOpen} disabled={open.isPending || isBusy}>
                {open.isPending ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Play className="h-3.5 w-3.5" />
                )}
                Launch
              </Button>
            )}
          </div>
        }
      />

      <div className="flex flex-1 flex-col gap-6 overflow-auto p-6">
        <ConnectionCard browser={b} />
        <StatusCard
          status={status}
          runtime={runtime}
          openedAt={b.last_opened_at}
        />
        {b.notes ? <NotesCard notes={b.notes} /> : null}
        <ActivityCard events={eventLog} />

        <div className="mt-2 flex justify-end">
          <Button
            variant="ghost"
            size="sm"
            className="text-danger hover:text-danger"
            onClick={handleDelete}
            disabled={isLive || isBusy || del.isPending}
          >
            <Trash2 className="h-3.5 w-3.5" />
            Delete browser
          </Button>
        </div>
      </div>

      <EditBrowserDialog
        open={editOpen}
        onOpenChange={setEditOpen}
        browser={b}
      />
    </>
  );
}

function Crumbs({
  status,
  pid,
  udd,
}: {
  status: BrowserStatus;
  pid: number | null;
  udd: string;
}) {
  return (
    <span className="flex items-center gap-2 text-xs text-ink-secondary">
      <StatusDot status={status} />
      <span className="capitalize">{status}</span>
      {pid ? <span className="text-ink-muted">· pid {pid}</span> : null}
      <ChevronRight className="h-3 w-3 text-ink-muted" />
      <span className="font-mono text-[11px] text-ink-muted truncate max-w-[420px]">
        {udd}
      </span>
    </span>
  );
}

export function StatusDot({ status }: { status: BrowserStatus }) {
  const klass: Record<BrowserStatus, string> = {
    starting: "bg-warning animate-pulse",
    running: "bg-success",
    stopping: "bg-warning",
    stopped: "bg-ink-muted/50",
    error: "bg-danger",
  };
  return <span className={`inline-block h-2 w-2 rounded-full ${klass[status]}`} />;
}

function Card({
  title,
  children,
  action,
}: {
  title: string;
  children: React.ReactNode;
  action?: React.ReactNode;
}) {
  return (
    <section className="rounded-xl border border-line bg-surface">
      <header className="flex items-center justify-between border-b border-line px-5 py-3">
        <h3 className="text-[11px] font-semibold uppercase tracking-[1.2px] text-ink-muted">
          {title}
        </h3>
        {action}
      </header>
      <div className="p-5">{children}</div>
    </section>
  );
}

function ConnectionCard({ browser }: { browser: Browser }) {
  const items: { label: string; value: React.ReactNode; mono?: boolean }[] = [
    {
      label: "SOCKS5 PROXY",
      value: maskProxy(browser.proxy_url) || <Empty />,
      mono: true,
    },
    {
      label: "TIMEZONE",
      value: browser.timezone || <Empty />,
    },
    {
      label: "LANGUAGE",
      value: browser.language || <Empty />,
    },
    {
      label: "FINGERPRINT ID",
      value: browser.fingerprint_id,
      mono: true,
    },
    {
      label: "CDP ENDPOINT",
      value: browser.cdp_port
        ? `http://127.0.0.1:${browser.cdp_port}`
        : <span className="text-ink-muted">offline</span>,
      mono: true,
    },
  ];
  return (
    <Card title="Connection">
      <div className="grid grid-cols-2 gap-x-8 gap-y-5">
        {items.map((it) => (
          <div key={it.label} className="space-y-1">
            <div className="text-[10px] font-semibold uppercase tracking-[1.2px] text-ink-muted">
              {it.label}
            </div>
            <div
              className={`text-sm ${
                it.mono ? "font-mono text-ink-secondary" : "text-ink"
              }`}
            >
              {it.value}
            </div>
          </div>
        ))}
      </div>
    </Card>
  );
}

function StatusCard({
  status,
  runtime,
  openedAt,
}: {
  status: BrowserStatus;
  runtime: RunningBrowser | null;
  openedAt: number | null;
}) {
  return (
    <Card
      title="Runtime"
      action={
        <Badge variant="secondary" className="gap-1.5 capitalize">
          <StatusDot status={status} />
          {status}
        </Badge>
      }
    >
      <dl className="grid grid-cols-3 gap-x-6 gap-y-3 text-sm">
        <Item label="Process ID" value={runtime?.pid ? `${runtime.pid}` : "—"} />
        <Item
          label="Started"
          value={
            runtime
              ? formatDistanceToNow(new Date(runtime.started_at * 1000), {
                  addSuffix: true,
                })
              : "—"
          }
        />
        <Item
          label="Last opened"
          value={
            openedAt
              ? formatDistanceToNow(new Date(openedAt * 1000), {
                  addSuffix: true,
                })
              : "Never"
          }
        />
      </dl>
    </Card>
  );
}

function NotesCard({ notes }: { notes: string }) {
  return (
    <Card title="Notes">
      <p className="whitespace-pre-wrap text-sm text-ink-secondary">{notes}</p>
    </Card>
  );
}

import { type BrowserEventEnvelope } from "@/hooks/useBrowserEvents";

function ActivityCard({ events }: { events: BrowserEventEnvelope[] }) {
  return (
    <Card
      title="Activity"
      action={
        <span className="text-[11px] text-ink-muted">
          {events.length === 0 ? "no events yet" : `${events.length} events`}
        </span>
      }
    >
      {events.length === 0 ? (
        <p className="text-xs text-ink-muted">
          Logs and stage events from the sidecar will appear here while the
          browser is running.
        </p>
      ) : (
        <ul className="max-h-64 space-y-1.5 overflow-auto pr-1 text-xs font-mono text-ink-secondary">
          {events
            .slice()
            .reverse()
            .map((e, i) => (
              <li key={i} className="flex gap-2">
                <span className="w-12 shrink-0 text-ink-muted">{e.kind}</span>
                <span className="truncate">{summarize(e)}</span>
              </li>
            ))}
        </ul>
      )}
    </Card>
  );
}

function summarize(e: BrowserEventEnvelope): string {
  if (e.kind === "exit") return `exit code=${e.code ?? "?"}`;
  if (e.payload) {
    const ev = e.payload.event as string | undefined;
    if (ev === "stage") {
      const stage = e.payload.stage as string | undefined;
      const extra = e.payload.pid ? ` pid=${e.payload.pid}` : "";
      return `stage ${stage}${extra}`;
    }
    if (ev === "tz")
      return `tz ${e.payload.timezone} detected=${e.payload.detected}`;
    if (ev === "proxy_check")
      return `proxy_check ${e.payload.host}:${e.payload.port} ok=${e.payload.ok}`;
    if (ev === "log") return `log ${e.payload.level ?? "info"} ${e.payload.message ?? ""}`;
    if (ev === "error") return `error ${e.payload.message ?? ""}`;
    return JSON.stringify(e.payload);
  }
  return e.message ?? "";
}

function Item({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="space-y-0.5">
      <dt className="text-[10px] font-semibold uppercase tracking-[1.2px] text-ink-muted">
        {label}
      </dt>
      <dd className="text-sm text-ink">{value}</dd>
    </div>
  );
}

function Empty() {
  return <span className="text-ink-muted">—</span>;
}

function maskProxy(url: string): string {
  if (!url) return "";
  return url.replace(/\/\/[^@/]+@/, "//••••@");
}
