import {
  LayoutDashboard,
  Globe,
  Settings,
  type LucideIcon,
} from "lucide-react";

export type NavGroup = {
  id: string;
  label: string;
  items: NavItem[];
};

export type NavItem = {
  id: string;
  label: string;
  to: string;
  icon: LucideIcon;
  badgeKey?: "browsers";
};

export const NAV_GROUPS: NavGroup[] = [
  {
    id: "workspace",
    label: "WORKSPACE",
    items: [
      { id: "dashboard", label: "Dashboard", to: "/", icon: LayoutDashboard },
      { id: "browsers", label: "Browsers", to: "/browsers", icon: Globe, badgeKey: "browsers" },
    ],
  },
];

export const FOOTER_NAV: NavItem = {
  id: "settings",
  label: "Settings",
  to: "/settings",
  icon: Settings,
};
