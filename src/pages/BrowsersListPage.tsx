import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { formatDistanceToNow } from "date-fns";
import { Loader2, Play, Plus, Square, Trash2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { useConfirm } from "@/components/ui/confirm-dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { TopBar } from "@/shell/TopBar";
import { NewBrowserDialog } from "@/components/browsers/NewBrowserDialog";
import { StatusDot } from "@/pages/BrowserDetailPage";
import {
  useBrowsers,
  useCloseBrowser,
  useDeleteBrowser,
  useOpenBrowser,
  useRunningBrowsers,
  type Browser,
  type BrowserStatus,
} from "@/lib/api";

export function BrowsersListPage() {
  const [creating, setCreating] = useState(false);
  const browsers = useBrowsers();
  const running = useRunningBrowsers();
  const deleteBrowser = useDeleteBrowser();
  const openBrowser = useOpenBrowser();
  const closeBrowser = useCloseBrowser();
  const confirm = useConfirm();

  const runningMap = useMemo(() => {
    const map = new Map<string, BrowserStatus>();
    for (const r of running.data ?? []) map.set(r.browser_id, r.status);
    return map;
  }, [running.data]);

  const runningCount = useMemo(() => {
    return Array.from(runningMap.values()).filter(
      (s) => s === "running" || s === "starting",
    ).length;
  }, [runningMap]);

  const subtitle = useMemo(() => {
    const total = browsers.data?.length ?? 0;
    return `${total} browser profile${total === 1 ? "" : "s"} · ${runningCount} running`;
  }, [browsers.data, runningCount]);

  async function handleDelete(browser: Browser) {
    const ok = await confirm({
      title: `Delete "${browser.name}"?`,
      description: "This also removes its user-data-dir on disk.",
      confirmText: "Delete",
      destructive: true,
    });
    if (!ok) return;
    try {
      await deleteBrowser.mutateAsync(browser.id);
      toast.success(`Deleted ${browser.name}`);
    } catch (e) {
      toast.error(`Delete failed: ${(e as Error).message ?? e}`);
    }
  }

  async function handleOpen(browser: Browser) {
    try {
      await openBrowser.mutateAsync({ id: browser.id });
    } catch (e) {
      toast.error(`Open failed: ${(e as Error).message ?? e}`);
    }
  }

  async function handleClose(browser: Browser) {
    try {
      await closeBrowser.mutateAsync(browser.id);
    } catch (e) {
      toast.error(`Close failed: ${(e as Error).message ?? e}`);
    }
  }

  return (
    <>
      <TopBar
        title="Browsers"
        subtitle={subtitle}
        actions={
          <Button size="sm" onClick={() => setCreating(true)}>
            <Plus className="h-3.5 w-3.5" /> New browser
          </Button>
        }
      />
      <div className="flex-1 overflow-y-auto p-6">
        {browsers.isLoading ? (
          <SkeletonState />
        ) : browsers.error ? (
          <ErrorState message={(browsers.error as Error).message} />
        ) : (browsers.data?.length ?? 0) === 0 ? (
          <EmptyState onCreate={() => setCreating(true)} />
        ) : (
          <div className="rounded-xl border border-line bg-surface">
            <Table>
              <TableHeader>
                <TableRow className="border-line">
                  <Th className="w-8" />
                  <Th>Name</Th>
                  <Th>Region · TZ</Th>
                  <Th>Fingerprint</Th>
                  <Th>Last opened</Th>
                  <Th className="w-32 text-right">Actions</Th>
                </TableRow>
              </TableHeader>
              <TableBody>
                {browsers.data!.map((b) => {
                  const status = runningMap.get(b.id);
                  const isLive =
                    status === "running" || status === "starting";
                  return (
                    <TableRow key={b.id} className="border-line">
                      <TableCell className="px-3 py-3">
                        <StatusDot status={status ?? "stopped"} />
                      </TableCell>
                      <TableCell className="whitespace-nowrap px-4 py-3">
                        <div className="flex items-center gap-2">
                          <Link
                            to={`/browsers/${b.id}`}
                            className="font-medium text-ink hover:text-brand"
                          >
                            {b.name || (
                              <span className="text-ink-muted">unnamed</span>
                            )}
                          </Link>
                        </div>
                      </TableCell>
                      <TableCell className="whitespace-nowrap px-4 py-3 text-sm text-ink-secondary">
                        {b.timezone || <Dash />}
                      </TableCell>
                      <TableCell className="whitespace-nowrap px-4 py-3 font-mono text-xs text-ink-secondary">
                        {b.fingerprint_id}
                      </TableCell>
                      <TableCell className="whitespace-nowrap px-4 py-3 text-sm text-ink-secondary">
                        {b.last_opened_at
                          ? formatDistanceToNow(
                              new Date(b.last_opened_at * 1000),
                              { addSuffix: true },
                            )
                          : "—"}
                      </TableCell>
                      <TableCell className="px-2 py-3">
                        <div className="flex items-center justify-end gap-1">
                          {isLive ? (
                            <Button
                              variant="outline"
                              size="sm"
                              className="h-7"
                              onClick={() => handleClose(b)}
                              disabled={closeBrowser.isPending}
                            >
                              {closeBrowser.isPending &&
                              closeBrowser.variables === b.id ? (
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
                              onClick={() => handleOpen(b)}
                              disabled={openBrowser.isPending}
                            >
                              {openBrowser.isPending &&
                              openBrowser.variables?.id === b.id ? (
                                <Loader2 className="h-3 w-3 animate-spin" />
                              ) : (
                                <Play className="h-3 w-3" />
                              )}
                              Launch
                            </Button>
                          )}
                          <Button
                            variant="ghost"
                            size="icon"
                            className="h-7 w-7 text-ink-secondary hover:bg-danger/10 hover:text-danger disabled:hover:bg-transparent disabled:hover:text-ink-secondary"
                            title={isLive ? "Close browser before deleting" : "Delete"}
                            disabled={isLive || deleteBrowser.isPending}
                            onClick={() => handleDelete(b)}
                          >
                            {deleteBrowser.isPending &&
                            deleteBrowser.variables === b.id ? (
                              <Loader2 className="h-3.5 w-3.5 animate-spin" />
                            ) : (
                              <Trash2 className="h-3.5 w-3.5" />
                            )}
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        )}
      </div>

      <NewBrowserDialog open={creating} onOpenChange={setCreating} />
    </>
  );
}

function Th({ children, className }: { children?: React.ReactNode; className?: string }) {
  return (
    <TableHead
      className={`px-4 text-[10px] font-semibold uppercase tracking-[1px] text-ink-muted ${
        className ?? ""
      }`}
    >
      {children}
    </TableHead>
  );
}

function Dash() {
  return <span className="text-ink-muted">—</span>;
}

function EmptyState({ onCreate }: { onCreate: () => void }) {
  return (
    <div className="grid h-full place-items-center">
      <div className="flex max-w-md flex-col items-center gap-3 rounded-xl border border-dashed border-line bg-surface p-10 text-center">
        <div className="rounded-full bg-brand-tint px-3 py-1 text-[11px] font-semibold tracking-wide text-brand">
          No browsers yet
        </div>
        <h2 className="text-lg font-semibold text-ink">
          Create your first browser profile
        </h2>
        <p className="text-sm text-ink-secondary">
          Each browser is an isolated Chromium with its own proxy, timezone,
          language and fingerprint. Profiles keep separate cookies and storage,
          so every window looks like a different device.
        </p>
        <Button onClick={onCreate} className="mt-2">
          <Plus className="h-3.5 w-3.5" /> New browser
        </Button>
      </div>
    </div>
  );
}

function SkeletonState() {
  return (
    <div className="space-y-3">
      {Array.from({ length: 4 }).map((_, i) => (
        <div
          key={i}
          className="h-12 animate-pulse rounded-md border border-line bg-surface"
        />
      ))}
    </div>
  );
}

function ErrorState({ message }: { message: string }) {
  return (
    <div className="rounded-xl border border-danger/40 bg-danger/5 p-6 text-sm text-danger">
      Failed to load browsers: {message}
    </div>
  );
}
