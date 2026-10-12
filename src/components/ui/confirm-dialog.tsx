import * as React from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export type ConfirmOptions = {
  title: string;
  description?: React.ReactNode;
  confirmText?: string;
  cancelText?: string;
  /** 删除/破坏性操作时，确认按钮使用 destructive 红色样式。 */
  destructive?: boolean;
};

type ConfirmContextValue = (opts: ConfirmOptions) => Promise<boolean>;

const ConfirmContext = React.createContext<ConfirmContextValue | null>(null);

type State =
  | { open: false }
  | {
      open: true;
      opts: ConfirmOptions;
      resolve: (ok: boolean) => void;
    };

export function ConfirmDialogProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [state, setState] = React.useState<State>({ open: false });

  const confirm = React.useCallback<ConfirmContextValue>(
    (opts) =>
      new Promise<boolean>((resolve) => {
        setState((prev) => {
          // 同一时间只允许一个确认弹窗：上一个直接当作 cancel。
          if (prev.open) prev.resolve(false);
          return { open: true, opts, resolve };
        });
      }),
    [],
  );

  const settle = React.useCallback((ok: boolean) => {
    setState((prev) => {
      if (prev.open) prev.resolve(ok);
      return { open: false };
    });
  }, []);

  const opts = state.open ? state.opts : null;

  return (
    <ConfirmContext.Provider value={confirm}>
      {children}
      <Dialog
        open={state.open}
        onOpenChange={(next) => {
          if (!next) settle(false);
        }}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{opts?.title ?? ""}</DialogTitle>
            {opts?.description ? (
              <DialogDescription asChild>
                <div className="text-sm text-muted-foreground">
                  {opts.description}
                </div>
              </DialogDescription>
            ) : null}
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => settle(false)}>
              {opts?.cancelText ?? "Cancel"}
            </Button>
            <Button
              variant={opts?.destructive ? "destructive" : "default"}
              onClick={() => settle(true)}
              autoFocus
            >
              {opts?.confirmText ?? "Confirm"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </ConfirmContext.Provider>
  );
}

/**
 * 应用内确认弹窗，替代 window.confirm。
 *
 * 用法：
 *   const confirm = useConfirm();
 *   if (!(await confirm({ title: "Delete browser?", destructive: true }))) return;
 */
export function useConfirm(): ConfirmContextValue {
  const ctx = React.useContext(ConfirmContext);
  if (!ctx) {
    throw new Error("useConfirm must be used inside <ConfirmDialogProvider>");
  }
  return ctx;
}
