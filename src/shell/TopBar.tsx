import type { ReactNode } from "react";

export interface TopBarProps {
  title: string;
  subtitle?: ReactNode;
  actions?: ReactNode;
  back?: ReactNode;
}

export function TopBar({ title, subtitle, actions, back }: TopBarProps) {
  return (
    <header className="flex h-16 shrink-0 items-center gap-4 border-b border-line bg-surface px-6">
      {back}
      <div className="flex min-w-0 flex-1 flex-col leading-tight">
        <h1 className="truncate text-base font-semibold text-ink">{title}</h1>
        {subtitle ? (
          <div className="truncate text-xs text-ink-secondary">{subtitle}</div>
        ) : null}
      </div>
      {actions ? <div className="flex items-center gap-2">{actions}</div> : null}
    </header>
  );
}
