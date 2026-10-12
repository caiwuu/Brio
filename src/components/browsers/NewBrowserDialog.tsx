/**
 * 「New browser」弹窗。
 *
 * 字段：
 *  - name：用户可读名（必填）
 *  - proxy_url：SOCKS5 URL（必填）
 *  - timezone：可手填；输入代理后自动探测，也可点 Auto（走 sidecar）
 *  - language：固定下拉
 *  - fingerprint_id：从 (proxy, tz, lang) 派生，只读展示
 *  - notes：可选
 *  - Launch immediately after create：创建后立刻 open_browser
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
import { Checkbox } from "@/components/ui/checkbox";
import { Select } from "@/components/ui/select";
import {
  useCreateBrowser,
  useDeriveFingerprintId,
  useDetectTimezone,
  useOpenBrowser,
  type Browser,
} from "@/lib/api";
import { DEFAULT_LANGUAGE, LANGUAGE_OPTIONS, languageForTimezone } from "@/lib/languages";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated?: (browser: Browser) => void;
}

export function NewBrowserDialog({ open, onOpenChange, onCreated }: Props) {
  const [name, setName] = useState("");
  const [proxyUrl, setProxyUrl] = useState("");
  const [timezone, setTimezone] = useState(DEFAULT_LANGUAGE.defaultTimezone);
  const [language, setLanguage] = useState(DEFAULT_LANGUAGE.value);
  const [notes, setNotes] = useState("");
  const [fingerprintId, setFingerprintId] = useState("");
  const [launchNow, setLaunchNow] = useState(false);

  const detectTz = useDetectTimezone();
  const deriveFp = useDeriveFingerprintId();
  const createBrowser = useCreateBrowser();
  const openBrowser = useOpenBrowser();

  // 用户手动改过 timezone 之后，就不再自动覆盖了（避免抢用户输入）。
  const [tzTouched, setTzTouched] = useState(false);

  // 重置表单
  useEffect(() => {
    if (!open) {
      setName("");
      setProxyUrl("");
      setTimezone(DEFAULT_LANGUAGE.defaultTimezone);
      setLanguage(DEFAULT_LANGUAGE.value);
      setNotes("");
      setFingerprintId("");
      setLaunchNow(false);
      setTzTouched(false);
    }
  }, [open]);

  // 代理变化后自动探测时区（debounced），除非用户已经手动改过。
  useEffect(() => {
    if (!open) return;
    if (tzTouched) return;
    const trimmed = proxyUrl.trim();
    if (!trimmed) return;
    const handle = window.setTimeout(() => {
      detectTz
        .mutateAsync(trimmed)
        .then((res) => {
          if (res.timezone) setTimezone(res.timezone);
        })
        .catch(() => {
          /* 静默失败，用户还可以点 Auto 手动再试 */
        });
    }, 600);
    return () => window.clearTimeout(handle);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, proxyUrl, tzTouched]);

  // (proxy, tz, lang) 任一变化都派生新的 fingerprint_id
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    deriveFp
      .mutateAsync({
        proxy_url: proxyUrl,
        timezone,
        language,
      })
      .then((res) => {
        if (!cancelled) setFingerprintId(res.fingerprint_id);
      })
      .catch(() => {
        if (!cancelled) setFingerprintId("");
      });
    return () => {
      cancelled = true;
    };
    // deriveFp 是稳定 mutation，不必加进依赖
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, proxyUrl, timezone, language]);

  const canSubmit = useMemo(
    () =>
      name.trim().length > 0 &&
      proxyUrl.trim().length > 0 &&
      timezone.trim().length > 0 &&
      fingerprintId.length > 0 &&
      !createBrowser.isPending,
    [name, proxyUrl, timezone, fingerprintId, createBrowser.isPending]
  );

  async function handleAutoDetect() {
    try {
      const res = await detectTz.mutateAsync(proxyUrl || undefined);
      setTimezone(res.timezone);
      setTzTouched(false);
      if (res.detected) {
        // 显式点 Auto 时把语言也一起联动填上；自动探测（输入代理后）只动时区。
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

  function handleLanguageChange(next: string) {
    setLanguage(next);
    // 切换语言时给一个合理的默认时区（用户仍可改）
    const opt = LANGUAGE_OPTIONS.find((l) => l.value === next);
    if (opt && !timezone) setTimezone(opt.defaultTimezone);
  }

  async function handleSubmit() {
    try {
      const browser = await createBrowser.mutateAsync({
        name: name.trim(),
        proxy_url: proxyUrl.trim(),
        timezone: timezone.trim(),
        language,
        fingerprint_id: fingerprintId,
        notes: notes.trim(),
      });
      toast.success(`Browser "${browser.name}" created`);
      if (launchNow) {
        try {
          await openBrowser.mutateAsync({ id: browser.id });
        } catch (e) {
          toast.error(`Launch failed: ${(e as Error).message ?? e}`);
        }
      }
      onCreated?.(browser);
      onOpenChange(false);
    } catch (e) {
      toast.error(`Create failed: ${(e as Error).message ?? e}`);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-[640px]">
        <DialogHeader>
          <DialogTitle>New browser</DialogTitle>
          <DialogDescription>
            Attach a SOCKS5 proxy and pick a language. We&apos;ll derive a stable fingerprint id.
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-5 py-2">
          <Field label="Name" htmlFor="b-name">
            <Input
              id="b-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Enter name"
              autoFocus
            />
          </Field>

          <Field
            label="SOCKS5 proxy URL"
            htmlFor="b-proxy"
            hint="socks5://user:pass@host:port"
          >
            <Input
              id="b-proxy"
              value={proxyUrl}
              onChange={(e) => setProxyUrl(e.target.value)}
              placeholder="socks5://user:pass@host:port"
              spellCheck={false}
              autoComplete="off"
            />
          </Field>

          <div className="grid grid-cols-[1fr_1fr_auto] items-end gap-4">
            <Field label="Timezone" htmlFor="b-tz">
              <Input
                id="b-tz"
                value={timezone}
                onChange={(e) => {
                  setTimezone(e.target.value);
                  setTzTouched(true);
                }}
                placeholder="America/Chicago"
              />
            </Field>

            <Field label="Language" htmlFor="b-lang">
              <Select
                id="b-lang"
                value={language}
                onChange={(e) => handleLanguageChange(e.target.value)}
                className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              >
                {LANGUAGE_OPTIONS.map((opt) => (
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

          <Field label="Fingerprint id" hint="Derived from (proxy, timezone, language).">
            <div className="flex h-9 items-center rounded-md border border-dashed border-line bg-subtle px-3 font-mono text-xs text-ink-secondary">
              {fingerprintId ? fingerprintId : "—"}
              {deriveFp.isPending ? (
                <Loader2 className="ml-2 h-3 w-3 animate-spin text-ink-muted" />
              ) : null}
            </div>
          </Field>

          <Field label="Notes" htmlFor="b-notes" optional>
            <Textarea
              id="b-notes"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              rows={2}
              placeholder="What is this browser for?"
            />
          </Field>

          <label className="flex items-start gap-2 rounded-md border border-line bg-subtle/40 p-3">
            <Checkbox
              id="b-launch"
              checked={launchNow}
              onCheckedChange={(v) => setLaunchNow(Boolean(v))}
            />
            <div className="space-y-0.5">
              <div className="text-[13px] font-medium text-ink">
                Launch immediately after create
              </div>
              <div className="text-xs text-ink-muted">
                Spawn Chromium right away with this profile&apos;s fingerprint.
              </div>
            </div>
          </label>
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSubmit} disabled={!canSubmit}>
            {createBrowser.isPending ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : null}
            Create browser
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
