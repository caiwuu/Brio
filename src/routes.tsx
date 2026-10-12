import { createBrowserRouter } from "react-router-dom";

import { AppShell } from "@/shell/AppShell";
import { DashboardPage } from "@/pages/DashboardPage";
import { BrowsersListPage } from "@/pages/BrowsersListPage";
import { BrowserDetailPage } from "@/pages/BrowserDetailPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { NotFoundPage } from "@/pages/NotFoundPage";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <DashboardPage /> },
      { path: "browsers", element: <BrowsersListPage /> },
      { path: "browsers/:id", element: <BrowserDetailPage /> },
      { path: "settings", element: <SettingsPage /> },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);
