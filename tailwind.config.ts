import type { Config } from "tailwindcss";
import animate from "tailwindcss-animate";

export default {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    container: {
      center: true,
      padding: "2rem",
      screens: { "2xl": "1400px" },
    },
    extend: {
      colors: {
        // shadcn 兼容（仍用 HSL 变量）
        border: "hsl(var(--border-hsl))",
        input: "hsl(var(--input-hsl))",
        ring: "hsl(var(--ring-hsl))",
        background: "hsl(var(--background-hsl))",
        foreground: "hsl(var(--foreground-hsl))",
        primary: {
          DEFAULT: "hsl(var(--primary-hsl))",
          foreground: "hsl(var(--primary-foreground-hsl))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary-hsl))",
          foreground: "hsl(var(--secondary-foreground-hsl))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive-hsl))",
          foreground: "hsl(var(--destructive-foreground-hsl))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted-hsl))",
          foreground: "hsl(var(--muted-foreground-hsl))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent-hsl))",
          foreground: "hsl(var(--accent-foreground-hsl))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover-hsl))",
          foreground: "hsl(var(--popover-foreground-hsl))",
        },
        card: {
          DEFAULT: "hsl(var(--card-hsl))",
          foreground: "hsl(var(--card-foreground-hsl))",
        },

        // ui.pen design tokens (走 CSS 变量；同时支持 light/dark)
        app: "var(--bg-app)",
        surface: "var(--bg-card)",
        subtle: "var(--bg-subtle)",
        line: "var(--border)",
        "line-strong": "var(--border-strong)",
        ink: {
          DEFAULT: "var(--text-primary)",
          muted: "var(--text-muted)",
          secondary: "var(--text-secondary)",
          inverse: "var(--text-inverse)",
        },
        brand: {
          DEFAULT: "var(--brand-primary)",
          secondary: "var(--brand-secondary)",
          accent: "var(--brand-accent)",
          tint: "var(--brand-tint)",
        },
        success: {
          DEFAULT: "var(--success)",
          bg: "var(--success-bg)",
        },
        warning: {
          DEFAULT: "var(--warning)",
          bg: "var(--warning-bg)",
        },
        danger: {
          DEFAULT: "var(--danger)",
          bg: "var(--danger-bg)",
        },
      },
      borderRadius: {
        sm: "var(--radius-sm)",
        md: "var(--radius-md)",
        lg: "var(--radius-lg)",
        xl: "var(--radius-xl)",
      },
      fontFamily: {
        sans: [
          "Inter",
          "-apple-system",
          "BlinkMacSystemFont",
          "Segoe UI",
          "PingFang SC",
          "Hiragino Sans GB",
          "Microsoft Yahei",
          "sans-serif",
        ],
        mono: ["JetBrains Mono", "ui-monospace", "SFMono-Regular", "monospace"],
      },
      keyframes: {
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
      },
    },
  },
  plugins: [animate],
} satisfies Config;
