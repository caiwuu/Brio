/**
 * 编辑浏览器：name / proxy / timezone / language / notes 都可改。
 *
 * 唯一锁死的是 fingerprint_id——它是 udd 目录（browser_data/<fp>/）的键，
 * 改了会让数据与指纹脱钩。要换指纹请新建 browser。
 *
 * 正在运行的浏览器也允许编辑，改动在下次启动时生效（对话框内有提示）。
 */
import { useEffect, useMemo, useState } from "react";
import { Loader2, Wand2 } from "lucide-react";
import { toast } from "sonner";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Select } from "@/components/ui/select";
import {
  useDetectTimezone,
  useRunningBrowsers,
  useUpdateBrowser,
  type Browser,
  type UpdateBrowserInput,
} from "@/lib/api";
import { LANGUAGE_OPTIONS, languageForTimezone } from "@/lib/languages";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  browser: Browser;
}

export function EditBrowserDialog({ open, onOpenChange, browser }: Props) {
  const [name, setName] = useState(browser.name);
  const [proxyUrl, setProxyUrl] = useState(browser.proxy_url);
  const [timezone, setTimezone] = useState(browser.timezone);
  const [language, setLanguage] = useState(browser.language);
  const [notes, setNotes] = useState(browser.notes);

  const update = useUpdateBrowser();
  const detectTz = useDetectTimezone();
  const running = useRunningBrowsers();

  const isLive = useMemo(() => {
    const s = running.data?.find((r) => r.browser_id === browser.id)?.status;
    return s === "running" || s === "starting";
  }, [running.data, browser.id]);

  // 当前语言可能不在预置列表里（历史数据），补一个选项避免 Select 显示错乱。
  const languageOptions = useMemo(() => {
    if (LANGUAGE_OPTIONS.some((o) => o.value === language)) {
      return LANGUAGE_OPTIONS;
    }
    return [
      { value: language, label: language, defaultTimezone: "" },
      ...LANGUAGE_OPTIONS,
    ];
  }, [language]);

  useEffect(() => {
    if (open) {
      setName(browser.name);
      setProxyUrl(browser.proxy_url);
      setTimezone(browser.timezone);
      setLanguage(browser.language);
      setNotes(browser.notes);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, browser]);

  const dirty =
    name.trim() !== browser.name ||
    proxyUrl.trim() !== browser.proxy_url ||
    timezone.trim() !== browser.timezone ||
    language !== browser.language ||
    notes !== browser.notes;
  const canSubmit = dirty && name.trim().length > 0 && !update.isPending;

  async function handleAutoDetect() {
    try {
      const res = await detectTz.mutateAsync(proxyUrl.trim() || undefined);
      setTimezone(res.timezone);
      if (res.detected) {
        // 只有真实探测成功才联动语言；fallback 默认时区时不动用户的选择。
        const lang = languageForTimezone(res.timezone);
        setLanguage(lang);
        toast.success(`Detected ${res.timezone} · ${lang}`);
      } else {
        toast.success(`Could not detect timezone, using default ${res.timezone}`);
      }
    } catch (e) {
      toast.error(`Detect failed: ${(e as Error).message ?? e}`);
    }
  }

  async function handleSubmit() {
    try {
      const input: UpdateBrowserInput = {};
      if (name.trim() !== browser.name) input.name = name.trim();
      if (proxyUrl.trim() !== browser.proxy_url) input.proxy_url = proxyUrl.trim();
      if (timezone.trim() !== browser.timezone) input.timezone = timezone.trim();
      if (language !== browser.language) input.language = language;
      if (notes !== browser.notes) input.notes = notes;
      await update.mutateAsync({ id: browser.id, input });
      toast.success(
        isLive ? "Saved — changes apply next launch" : "Browser updated",
      );
      onOpenChange(false);
    } catch (e) {
      toast.error(`Update failed: ${(e as Error).message ?? e}`);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-[640px]">
        <DialogHeader>
          <DialogTitle>Edit browser</DialogTitle>
          <DialogDescription>
            Update the profile&apos;s proxy, timezone or language. The
            fingerprint is fixed — create a new browser if you need a
            different one.
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-5 py-2">
          <Field label="Name" htmlFor="eb-name">
            <Input
              id="eb-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              autoFocus
            />
          </Field>

          <Field
            label="Proxy URL"
            htmlFor="eb-proxy"
            hint="socks5://user:pass@host:port"
          >
            <Input
              id="eb-proxy"
              value={proxyUrl}
              onChange={(e) => setProxyUrl(e.target.value)}
              placeholder="socks5://user:pass@host:port"
              spellCheck={false}
              autoComplete="off"
            />
          </Field>

          <div className="grid grid-cols-[1fr_1fr_auto] items-end gap-4">
            <Field label="Timezone" htmlFor="eb-tz">
              <Input
                id="eb-tz"
                value={timezone}
                onChange={(e) => setTimezone(e.target.value)}
                placeholder="America/Chicago"
              />
            </Field>

            <Field label="Language" htmlFor="eb-lang">
              <Select
                id="eb-lang"
                value={language}
                onChange={(e) => setLanguage(e.target.value)}
                className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              >
                {languageOptions.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </Select>
            </Field>

            <Button
              type="button"
              variant="outline"
              onClick={handleAutoDetect}
              disabled={detectTz.isPending}
              className="h-9 shrink-0"
              title="Detect timezone & language through the proxy above"
            >
              {detectTz.isPending ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Wand2 className="h-3.5 w-3.5" />
              )}
              Auto
            </Button>
          </div>

          <Field
            label="Fingerprint id"
            hint="Fixed — bound to this profile's data directory."
          >
            <div className="flex h-9 items-center rounded-md border border-dashed border-line bg-subtle px-3 font-mono text-xs text-ink-secondary">
              {browser.fingerprint_id}
            </div>
          </Field>

          <Field label="Notes" htmlFor="eb-notes" optional>
            <Textarea
              id="eb-notes"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              rows={2}
              placeholder="What is this browser for?"
            />
          </Field>

          {isLive ? (
            <div className="rounded-md border border-warning/30 bg-warning/10 p-3 text-xs text-ink-secondary">
              This browser is currently running — proxy / timezone / language
              changes apply the next time it launches.
            </div>
          ) : null}
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSubmit} disabled={!canSubmit}>
            {update.isPending ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : null}
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Field({
  label,
  htmlFor,
  hint,
  optional,
  children,
}: {
  label: string;
  htmlFor?: string;
  hint?: string;
  optional?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-baseline justify-between">
        <Label htmlFor={htmlFor} className="text-xs font-medium uppercase tracking-wide text-ink-muted">
          {label}
          {optional ? (
            <span className="ml-1 normal-case text-[10px] text-ink-muted/80">(optional)</span>
          ) : null}
        </Label>
        {hint ? <span className="text-[11px] text-ink-muted">{hint}</span> : null}
      </div>
      {children}
    </div>
  );
}
