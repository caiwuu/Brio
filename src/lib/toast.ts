import { toast } from "sonner";

/**
 * 统一的错误提示。sonner 在 message 为空字符串/undefined 时会渲染一个
 * "只有 icon 没有文字" 的空 toast（在 top-right 看上去就是一个圆形感叹号），
 * 体验很差。这里强制兜底，永远显示有意义的文本。
 */
export function showError(err: unknown, fallback = "Operation failed"): string {
  const msg = errorMessage(err);
  const text = msg && msg.trim() ? msg : fallback;
  toast.error(text);
  return text;
}

export function errorMessage(err: unknown): string {
  if (err == null) return "";
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message ?? "";
  if (typeof err === "object" && "message" in err) {
    const m = (err as { message?: unknown }).message;
    if (typeof m === "string") return m;
  }
  try {
    return String(err);
  } catch {
    return "";
  }
}
