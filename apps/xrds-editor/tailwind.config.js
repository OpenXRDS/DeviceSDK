/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  // Preflight's reset would fight editor.css's existing global reset
  // during this incremental migration. Re-enable once editor.css (and
  // every component still depending on its classes) is fully retired.
  corePlugins: {
    preflight: false,
  },
  theme: {
    extend: {
      // Resolve to the same CSS custom properties editor.css's `:root`
      // already defines, rather than duplicating hex values in two
      // places — that block stays the single source of truth for the
      // palette. See docs/xrds-trigger-action-editor-plan.md's frontend
      // follow-up notes if this ever needs to change.
      colors: {
        base: "var(--base)",
        mantle: "var(--mantle)",
        crust: "var(--crust)",
        elevated: "var(--elevated)",
        well: "var(--well)",
        sel: "var(--sel)",
        surface0: "var(--surface0)",
        surface1: "var(--surface1)",
        overlay0: "var(--overlay0)",
        text: "var(--text)",
        bright: "var(--bright)",
        subtext0: "var(--subtext0)",
        subtext1: "var(--subtext1)",
        blue: "var(--blue)",
        "blue-l": "var(--blue-l)",
        green: "var(--green)",
        red: "var(--red)",
        peach: "var(--peach)",
        mauve: "var(--mauve)",
        teal: "var(--teal)",
        yellow: "var(--yellow)",
        flamingo: "var(--flamingo)",
      },
      // Same tokens as editor.css's `:root` (see the redesign pitch in
      // docs/done/xrds-trigger-action-editor-plan.md's frontend follow-up
      // notes) — sans for UI text, mono demoted to numbers/paths/code, and
      // the same 6/8px radius scale so Tailwind-based components match
      // CSS-class-based ones exactly.
      fontFamily: {
        sans: ["var(--font-sans)"],
        mono: ["var(--font-mono)"],
      },
      borderRadius: {
        sm: "var(--radius-sm)",
        DEFAULT: "var(--radius)",
        md: "var(--radius)",
        lg: "var(--radius)",
      },
    },
  },
  plugins: [],
};
