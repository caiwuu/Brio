import { NavLink } from "react-router-dom";
import { Fingerprint } from "lucide-react";

import { cn } from "@/lib/utils";
import { FOOTER_NAV, NAV_GROUPS, type NavItem } from "@/lib/nav";
import { BottomStatus } from "./BottomStatus";

export interface SidebarCounts {
  browsers?: number;
}

export function Sidebar({ counts }: { counts?: SidebarCounts }) {
  return (
    <aside className="flex h-full w-60 shrink-0 flex-col border-r border-line bg-surface">
      <Brand />
      <nav className="flex-1 overflow-y-auto px-3 py-2 scroll-area">
        {NAV_GROUPS.map((group) => (
          <div key={group.id} className="mb-4">
            <div className="px-2 py-1.5 text-[10px] font-semibold tracking-[1.5px] text-ink-muted">
              {group.label}
            </div>
            <ul className="flex flex-col gap-0.5">
              {group.items.map((item) => (
                <SidebarItem
                  key={item.id}
                  item={item}
                  badge={item.badgeKey ? counts?.[item.badgeKey] : undefined}
                />
              ))}
            </ul>
          </div>
        ))}
      </nav>
      <div className="border-t border-line px-3 py-2">
        <SidebarItem item={FOOTER_NAV} />
      </div>
      <BottomStatus />
    </aside>
  );
}

function Brand() {
  return (
    <div className="flex items-center gap-2.5 px-4 py-4">
      <div className="grid h-8 w-8 place-items-center rounded-md bg-brand text-ink-inverse">
        <Fingerprint className="h-4 w-4" />
      </div>
      <div className="flex flex-col leading-tight">
        <span className="text-sm font-semibold text-ink">Brio</span>
        <span className="text-[10px] uppercase tracking-wider text-ink-muted">
        Fingerprint Browser
        </span>
      </div>
    </div>
  );
}

function SidebarItem({ item, badge }: { item: NavItem; badge?: number }) {
  const Icon = item.icon;
  return (
    <li>
      <NavLink
        to={item.to}
        end={item.to === "/"}
        className={({ isActive }) =>
          cn(
            "group flex items-center gap-2.5 rounded-md px-2.5 py-1.5 text-[13px] font-medium transition-colors",
            isActive
              ? "bg-brand-tint text-brand"
              : "text-ink-secondary hover:bg-subtle hover:text-ink"
          )
        }
      >
        <Icon className="h-4 w-4" />
        <span className="flex-1">{item.label}</span>
        {typeof badge === "number" && badge > 0 ? (
          <span className="rounded-full bg-subtle px-1.5 text-[10px] font-semibold text-ink-secondary group-data-[active=true]:bg-brand group-data-[active=true]:text-ink-inverse">
            {badge}
          </span>
        ) : null}
      </NavLink>
    </li>
  );
}
