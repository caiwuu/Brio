import * as React from "react";
import { ChevronDown } from "lucide-react";

import { cn } from "@/lib/utils";

/**
 * 样式化的原生 `<select>`。
 *
 * 目的：干掉浏览器默认的灰渐变 + 双箭头，做成和 `<Input>` 视觉一致的控件。
 * 仍然是原生 select，自带键盘、屏幕阅读器和移动端原生弹层支持，
 * 不引 Radix/下拉库。
 *
 * 用法和原生 `<select>` 一样，直接传 `<option>` 作为 children。
 * 想调高度只用 `className="h-11"` 之类的覆盖。
 */
const Select = React.forwardRef<
  HTMLSelectElement,
  React.SelectHTMLAttributes<HTMLSelectElement>
>(({ className, children, ...props }, ref) => {
  return (
    <div className="relative w-full">
      <select
        ref={ref}
        className={cn(
          // 基本形：和 Input 对齐
          "flex h-9 w-full appearance-none rounded-md border border-input bg-transparent px-3 pr-8 py-1 text-sm shadow-sm transition-colors",
          // 交互
          "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring",
          "disabled:cursor-not-allowed disabled:opacity-50",
          // WebKit 上默认 option 看起来还好，但 select 自身 background/gradient 必须关
          "[&::-ms-expand]:hidden",
          className,
        )}
        {...props}
      >
        {children}
      </select>
      <ChevronDown
        className="pointer-events-none absolute right-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-ink-muted"
        aria-hidden
      />
    </div>
  );
});
Select.displayName = "Select";

export { Select };
