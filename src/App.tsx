import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useMemo } from "react";
import { RouterProvider } from "react-router-dom";

import { ConfirmDialogProvider } from "@/components/ui/confirm-dialog";
import { Toaster } from "@/components/ui/sonner";
import { router } from "@/routes";

export default function App() {
  const queryClient = useMemo(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime: 5_000,
            refetchOnWindowFocus: false,
            retry: 1,
          },
        },
      }),
    []
  );
  return (
    <QueryClientProvider client={queryClient}>
      <ConfirmDialogProvider>
        <RouterProvider router={router} />
        <Toaster richColors closeButton position="top-center" />
      </ConfirmDialogProvider>
    </QueryClientProvider>
  );
}
