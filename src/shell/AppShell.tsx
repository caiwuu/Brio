import { Outlet } from "react-router-dom";

import { isTauriRuntime, useBrowsers } from "@/lib/api";
import { useBrowserEvents } from "@/hooks/useBrowserEvents";

import { Sidebar, type SidebarCounts } from "./Sidebar";
import { RuntimeBanner } from "./RuntimeBanner";

/**
 * 主布局：左侧 240px Sidebar + 右侧主区。
 */
export function AppShell() {
  const inTauri = isTauriRuntime();
  // 全局订阅：把 browser-event 转成 react-query invalidation。
  useBrowserEvents();
  // 在浏览器里跑 vite 时不要发 invoke，避免一堆错误 toast。
  const browsers = useBrowsers({ enabled: inTauri });
  const counts: SidebarCounts = {
    browsers: browsers.data?.length ?? 0,
  };
  return (
    <div className="flex h-full w-full overflow-hidden bg-app text-ink">
      <Sidebar counts={counts} />
      <main className="flex h-full min-w-0 flex-1 flex-col overflow-hidden">
        <RuntimeBanner />
        <Outlet />
      </main>
    </div>
  );
}
