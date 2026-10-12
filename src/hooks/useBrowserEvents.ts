/**
 * 全局监听 Rust 端 emit 的 `browser-event`，把状态变化广播到 react-query。
 *
 * - kind === "stdout" 且 status 变化 → invalidate runningKeys + 当前 browser detail
 * - kind === "exit"                  → invalidate runningKeys
 * - 也额外把最近一条 event 暴露成 React state，让 BrowserDetailPage 可以直接拿去渲染日志
 */
import { useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";

import {
  browsersKeys,
  runningKeys,
  type BrowserStatus,
} from "@/lib/api";

export interface BrowserEventEnvelope {
  browser_id: string;
  kind: "stdout" | "stderr" | "exit" | "status";
  status?: BrowserStatus;
  payload?: Record<string, unknown>;
  message?: string;
  code?: number | null;
  pid?: number | null;
}

export function useBrowserEvents(
  onEvent?: (e: BrowserEventEnvelope) => void
) {
  const qc = useQueryClient();
  const cb = useRef(onEvent);
  cb.current = onEvent;

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    listen<BrowserEventEnvelope>("browser-event", (e) => {
      const env = e.payload;
      qc.invalidateQueries({ queryKey: runningKeys.all });
      qc.invalidateQueries({ queryKey: browsersKeys.detail(env.browser_id) });
      if (env.kind === "exit") {
        qc.invalidateQueries({ queryKey: browsersKeys.all });
      }
      cb.current?.(env);
    }).then((u) => {
      unlisten = u;
    });
    return () => {
      unlisten?.();
    };
  }, [qc]);
}

/** 给 BrowserDetailPage 用：拿当前 browser 的最新一条事件 + 一个滚动日志缓冲。 */
export function useBrowserEventLog(
  browserId: string | undefined,
  capacity = 100,
) {
  const [log, setLog] = useState<BrowserEventEnvelope[]>([]);
  useBrowserEvents((env) => {
    if (!browserId || env.browser_id !== browserId) return;
    setLog((prev) => {
      const next = [...prev, env];
      return next.length > capacity ? next.slice(-capacity) : next;
    });
  });
  return log;
}
