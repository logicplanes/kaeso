import brand from "./brand/tokens.json" with { type: "json" };

type Mode = { light: string; dark: string };
const mode = (light: string, dark: string): Mode => ({ light, dark });

export const neutral = {
  "bg-panel": mode(brand.color.light.canvas, brand.color.dark.canvas),
  "bg-chrome": mode("#f5f5f5", brand.color.dark.surface),
  "bg-raised": mode("#ffffff", "#111111"),
  "fill-1": mode(brand.color.light.muted, brand.color.dark.muted),
  "fill-2": mode("#ebebeb", "#1c1c1c"),
  "fill-3": mode("#e0e0e0", "#262626"),
  "line-subtle": mode("#f0f0f0", "#171717"),
  line: mode(brand.color.light.border, brand.color.dark.border),
  "line-strong": mode(brand.color.light.borderEmphasized, brand.color.dark.borderEmphasized),
  "line-field": mode("#858585", "#757575"),
  "line-field-hover": mode("#707070", "#858585"),
  "switch-thumb": mode("#ffffff", "#c7c7c7"),
  text: mode(brand.color.light.text, brand.color.dark.text),
  "text-secondary": mode(brand.color.light.textSecondary, brand.color.dark.textSecondary),
  "text-tertiary": mode("#696969", "#999999"),
  "text-disabled": mode(brand.color.light.textDisabled, brand.color.dark.textDisabled),
  ink: mode(brand.color.light.ink, brand.color.dark.ink),
  "ink-hover": mode("#262626", "#e0e0e0"),
  "ink-pressed": mode("#1a1a1a", "#d4d4d4"),
  "on-ink": mode(brand.color.light.onStrong, brand.color.dark.onStrong),
  focus: mode(brand.color.light.focus, brand.color.dark.focus),
  overlay: mode("rgb(0 0 0 / 0.24)", "rgb(0 0 0 / 0.72)"),
  "panel-ring": mode(brand.color.light.border, "#1a1a1a"),
  "toast-bg": mode("#0a0a0a", "#1c1c1c"),
  scan: mode("#ffffff", "#ffffff"),
  "on-scan": mode("#0a0a0a", "#0a0a0a"),
  "toast-fg": mode("#fafafa", "#fafafa"),
  "toast-fg-secondary": mode("#a3a3a3", "#a3a3a3"),
  "toast-ring": mode("rgb(255 255 255 / 0.06)", "rgb(255 255 255 / 0.09)"),
  "toast-action": mode("rgb(255 255 255 / 0.12)", "rgb(255 255 255 / 0.1)"),
  "toast-action-hover": mode("rgb(255 255 255 / 0.2)", "rgb(255 255 255 / 0.16)"),
  "toast-danger": mode("#f87171", "#f87171"),
  "item-hover": mode("#f5f5f5", "#222222"),
  "item-pressed": mode("#ebebeb", "#2b2b2b"),
  "raised-hover": mode("#f5f5f5", "#1c1c1c"),
  "raised-pressed": mode("#ebebeb", "#262626"),
  "control-on-fill": mode("#ffffff", "#222222"),
  "control-on-fill-hover": mode("#fafafa", "#2b2b2b"),
  "bg-tooltip": mode("#ffffff", "#1c1c1c"),
} as const;

export const semantic = {
  success: { solid: mode("#22c55e", "#22c55e"), text: mode("#166534", "#4ade80"), tint: mode("#f0fdf4", "#18231a") },
  warning: { solid: mode("#f59e0b", "#f59e0b"), text: mode("#92400e", "#fbbf24"), tint: mode("#fffbeb", "#272017") },
  danger: { solid: mode("#ef4444", "#ef4444"), text: mode("#b91c1c", "#f87171"), tint: mode("#fef2f2", "#281918") },
  info: { solid: mode("#3b82f6", "#3b82f6"), text: mode("#1d4ed8", "#60a5fa"), tint: mode("#eff6ff", "#171d28") },
} as const;

export const diff = {
  added: { line: mode("#ebf9ef", "#0a2211"), word: mode("#aeeabf", "#133e26") },
  removed: { line: mode("#fef2f2", "#2a0d0e"), word: mode("#fbcbcb", "#5a1a1c") },
} as const;

export const dangerFill = mode("#dc2626", "#dc2626");
export const onDanger = mode("#ffffff", "#ffffff");

export const viz = [
  { name: "blue", light: "#3b82f6", dark: "#60a5fa", thin: "#3b82f6" },
  { name: "amber", light: "#cc6d05", dark: "#fbbf24", thin: "#cc6d05" },
  { name: "emerald", light: "#059669", dark: "#34d399", thin: "#059669" },
  { name: "rose", light: "#f43f5e", dark: "#fb7185", thin: "#f43f5e" },
  { name: "violet", light: "#8b5cf6", dark: "#a78bfa", thin: "#8b5cf6" },
  { name: "teal", light: "#0d9488", dark: "#2dd4bf", thin: "#0d9488" },
  { name: "orange", light: "#ea580c", dark: "#fb923c", thin: "#ea580c" },
  { name: "fuchsia", light: "#d946ef", dark: "#e879f9", thin: "#d946ef" },
] as const;

export const art = {
  grounds: {
    paper: { fill: "#f1ece2", text: "#141414" },
    ink: { fill: "#1c1c22", text: "#f7f5f0" },
  },
  palettes: [
    { name: "bauhaus", ground: "paper", colors: ["#e4572e", "#f3a712", "#29335c", "#669bbc"] },
    { name: "harbor", ground: "paper", colors: ["#d1495b", "#edae49", "#00798c", "#30638e"] },
    { name: "terrace", ground: "paper", colors: ["#264653", "#2a9d8f", "#e9c46a", "#e76f51"] },
    { name: "orchard", ground: "paper", colors: ["#bc4b51", "#5b8e7d", "#f4a259", "#8cb369"] },
    { name: "kiln", ground: "paper", colors: ["#0f4c5c", "#e36414", "#fb8b24", "#9a031e"] },
    { name: "riso", ground: "paper", colors: ["#ff48b0", "#0078bf", "#ffe800", "#00a95c"] },
    { name: "signal", ground: "ink", colors: ["#ff4f3f", "#ffd23f", "#3bceac", "#f7f5f0"] },
    { name: "arcade", ground: "ink", colors: ["#f72585", "#7209b7", "#4cc9f0", "#f7f5f0"] },
    { name: "sunset", ground: "ink", colors: ["#ffbe0b", "#fb5607", "#ff006e", "#3a86ff"] },
    { name: "sorbet", ground: "ink", colors: ["#e9ff70", "#ffd670", "#ff9770", "#ff70a6"] },
    { name: "lagoon", ground: "ink", colors: ["#06d6a0", "#ffd166", "#ef476f", "#118ab2"] },
    { name: "graphite", ground: "ink", colors: ["#f7f5f0", "#a3a3a3", "#525252", "#e5484d"] },
  ],
} as const;

export const tokenKindSeries = { input: 1, output: 3, cacheRead: 1, cacheWrite: 2, reasoning: 5 } as const;
export const tokenKindTint: Partial<Record<keyof typeof tokenKindSeries, true>> = { cacheRead: true };
export const vizSoft = mode("#a7c7fb", "#3d72b8");

export const providerMark = {
  openai: mode("#000000", "#ffffff"),
  xai: mode("#000000", "#ffffff"),
  "togetherai-magenta": mode("#ef2cc1", "#ef2cc1"),
  "togetherai-lavender": mode("#a488cd", "#caaef5"),
  "togetherai-orange": mode("#fc4c02", "#fc4c02"),
  openrouter: mode("#03080a", "#fcfcfe"),
  "mistral-yellow": mode("#ffaf01", "#ffaf01"),
  "mistral-amber": mode("#ff8204", "#ff8204"),
  "mistral-orange": mode("#fa500f", "#fa500f"),
  "mistral-red": mode("#e61300", "#e61300"),
  "mistral-crimson": mode("#c4001d", "#c4001d"),
  vercel: mode("#000000", "#ffffff"),
  "cohere-green": mode("#355146", "#4b685c"),
  "cohere-lilac": mode("#be7ccf", "#d18ee2"),
  "cohere-coral": mode("#f26b4e", "#ff7759"),
  deepseek: mode("#4d6bfe", "#6799fe"),
  groq: mode("#f43e01", "#f43e01"),
  moonshotai: mode("#1783ff", "#1783ff"),
  "alibaba-violet": mode("#4f21ff", "#543aff"),
  "alibaba-pink": mode("#d75bfe", "#d75bfe"),
} as const;

export const type = {
  micro: { size: 11, lineHeight: 14, weight: 500, tracking: 0.01 },
  caption: { size: 12, lineHeight: 16, weight: 500, tracking: 0 },
  small: { size: 13, lineHeight: 18, weight: 400, tracking: 0 },
  body: { size: 14, lineHeight: 20, weight: 400, tracking: 0 },
  "body-lg": { size: 15, lineHeight: 22, weight: 500, tracking: -0.005 },
  "title-sm": { size: 17, lineHeight: 24, weight: 600, tracking: -0.01 },
  title: { size: 20, lineHeight: 28, weight: 600, tracking: -0.015 },
  "title-lg": { size: 28, lineHeight: 34, weight: 600, tracking: -0.022 },
  display: { size: 40, lineHeight: 44, weight: 600, tracking: -0.03 },
  metric: { size: 28, lineHeight: 32, weight: 600, tracking: -0.02 },
} as const;

export const weight = {
  regular: mode("400", "400"),
  medium: mode("500", "480"),
  semibold: mode("600", "580"),
} as const;

export const font = {
  sans: '"Inter Variable", Inter, ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif',
  mono: '"Geist Mono Variable", "Geist Mono", ui-monospace, SFMono-Regular, Menlo, monospace',
  features: '"cv11", "ss01", "ss03"',
} as const;

export const control = {
  xs: { height: 24, padding: 10, icon: 14, gap: 4, text: "caption" },
  sm: { height: 32, padding: 12, icon: 16, gap: 6, text: "body" },
  md: { height: 36, padding: 16, icon: 16, gap: 6, text: "body" },
  lg: { height: 40, padding: 18, icon: 18, gap: 8, text: "body" },
  xl: { height: 48, padding: 22, icon: 20, gap: 8, text: "body-lg" },
} as const;

export const row = {
  list: 44,
  table: 40,
  tableCompact: 36,
  tableComfortable: 48,
  menu: 34,
  menuCoarse: 44,
  sidebar: 32,
  option: 34,
  settings: 64,
  settingsSingle: 56,
  groupHeader: 36,
  palette: 40,
} as const;

export const chrome = { header: 52, toolbar: 48, sidebar: 248, sidebarMin: 232, sidebarMax: 280, rail: 60, inset: 8 } as const;

export const space = [2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48, 64, 80, 96] as const;

export const lengths = [
  0, 1, 2, 3, 4, 5, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 28, 32, 34, 36, 40, 44, 48, 52, 56, 60, 64, 72, 80, 96, 112, 120, 140, 160, 180, 200, 220, 240, 248,
  260, 280, 320, 360, 400, 440, 480, 560, 640, 720, 760, 800,
] as const;

export const radius = {
  pill: "9999px",
  panel: "12px",
  menu: "16px",
  item: "10px",
  popover: "16px",
  dialog: "20px",
  sheet: "16px",
  group: "14px",
  well: "14px",
  tooltip: "12px",
  palette: "20px",
  "palette-item": "12px",
  checkbox: "5px",
  org: "8px",
  kbd: "6px",
  code: "6px",
  bar: "3px",
  cell: "3px",
  avatar: "6px",
  field: "18px",
  textarea: "14px",
  "cell-lg": "14px",
  "tooltip-lg": "20px",
  frame: "24px",
  toast: "22px",
} as const;

export const shadow = {
  ring: mode("inset 0 0 0 1px var(--k-line)", "inset 0 0 0 1px var(--k-line)"),
  control: mode("inset 0 0 0 1px var(--k-line), 0 1px 2px rgb(0 0 0 / 0.05)", "inset 0 0 0 1px var(--k-line)"),
  "control-hover": mode("inset 0 0 0 1px var(--k-line-strong), 0 1px 2px rgb(0 0 0 / 0.05)", "inset 0 0 0 1px var(--k-line-strong)"),
  "control-on-fill": mode("inset 0 0 0 1px var(--k-line-strong)", "inset 0 0 0 1px var(--k-line-strong)"),
  field: mode("inset 0 0 0 1px var(--k-line-field), 0 1px 2px rgb(0 0 0 / 0.05)", "inset 0 0 0 1px var(--k-line-field)"),
  "field-hover": mode("inset 0 0 0 1px var(--k-line-field-hover), 0 1px 2px rgb(0 0 0 / 0.05)", "inset 0 0 0 1px var(--k-line-field-hover)"),
  selector: mode("inset 0 0 0 1px var(--k-line-field), 0 1px 2px rgb(0 0 0 / 0.05)", "inset 0 0 0 1px var(--k-line-field)"),
  "selector-hover": mode("inset 0 0 0 1px var(--k-line-field-hover), 0 1px 2px rgb(0 0 0 / 0.05)", "inset 0 0 0 1px var(--k-line-field-hover)"),
  frame: mode("0 0 0 1px var(--k-line)", "0 0 0 1px var(--k-line)"),
  xs: mode("0 0 0 1px var(--k-line), 0 1px 2px rgb(0 0 0 / 0.03)", "0 0 0 1px var(--k-line)"),
  raised: mode(
    "0 0 0 1px var(--k-line), 0 4px 8px -2px rgb(0 0 0 / 0.06), 0 16px 32px -8px rgb(0 0 0 / 0.14)",
    "0 0 0 1px #262626, 0 16px 40px rgb(0 0 0 / 0.8)",
  ),
  dialog: mode("0 0 0 1px var(--k-line), 0 24px 64px -16px rgb(0 0 0 / 0.24)", "0 0 0 1px #2a2a2a, 0 32px 80px rgb(0 0 0 / 0.9)"),
  panel: mode("0 0 0 1px var(--k-panel-ring), 0 1px 2px rgb(0 0 0 / 0.03)", "0 0 0 1px var(--k-panel-ring)"),
  segment: mode("inset 0 0 0 1px var(--k-line), 0 1px 2px rgb(0 0 0 / 0.06)", "inset 0 0 0 1px var(--k-line-strong)"),
  toast: mode(
    "inset 0 0 0 1px var(--k-toast-ring), 0 2px 6px rgb(0 0 0 / 0.12), 0 12px 32px -8px rgb(0 0 0 / 0.35)",
    "inset 0 0 0 1px var(--k-toast-ring), 0 2px 6px rgb(0 0 0 / 0.6), 0 16px 40px rgb(0 0 0 / 0.8)",
  ),
  thumb: mode("0 1px 2px rgb(0 0 0 / 0.16), 0 0 0 1px rgb(0 0 0 / 0.04)", "0 1px 2px rgb(0 0 0 / 0.5)"),
} as const;

export const themeDisplay = {
  "show-light": mode("block", "none"),
  "show-dark": mode("none", "block"),
} as const;

export const material = {
  "primary-gradient": mode(
    "linear-gradient(180deg, rgb(255 255 255 / 0.16), rgb(255 255 255 / 0) 50%)",
    "linear-gradient(180deg, rgb(255 255 255 / 0), rgb(0 0 0 / 0.06))",
  ),
  "primary-shadow": mode(
    "inset 0 1px 0 rgb(255 255 255 / 0.14), 0 1px 2px rgb(0 0 0 / 0.24)",
    "inset 0 1px 0 rgb(255 255 255 / 0.5), 0 1px 2px rgb(0 0 0 / 0.5)",
  ),
  "danger-gradient": mode(
    "linear-gradient(180deg, rgb(255 255 255 / 0.16), rgb(255 255 255 / 0) 50%)",
    "linear-gradient(180deg, rgb(255 255 255 / 0.12), rgb(255 255 255 / 0) 50%)",
  ),
  "danger-shadow": mode(
    "inset 0 1px 0 rgb(255 255 255 / 0.14), 0 1px 2px rgb(0 0 0 / 0.24)",
    "inset 0 1px 0 rgb(255 255 255 / 0.24), 0 1px 2px rgb(0 0 0 / 0.5)",
  ),
} as const;

export const motion = {
  duration: {
    fast: 120,
    "close-overlay": 110,
    overlay: 160,
    "close-dialog": 140,
    dialog: 200,
    "close-sheet": 180,
    sheet: 240,
    "toast-in": 220,
    "toast-out": 160,
    indicator: 180,
    disclosure: 200,
    chart: 240,
    check: 140,
    radio: 160,
    copied: 1200,
    fresh: 1000,
  },
  ease: {
    standard: "cubic-bezier(0.2, 0, 0, 1)",
    overlay: "cubic-bezier(0.16, 1, 0.3, 1)",
    exit: "cubic-bezier(0.4, 0, 1, 1)",
    pulse: "cubic-bezier(0.4, 0, 0.6, 1)",
  },
  skeleton: { duration: 1600, low: 0.55 },
  spinner: { turn: 2400, delay: 300, minimum: 400 },
} as const;

export const z = { sticky: 10, sheet: 40, overlay: 50, toast: 60, tooltip: 70 } as const;

export const optical = {
  "leading-icon": 2,
  "trailing-chevron": 2,
} as const;

export const width = {
  prose: "68ch",
  settings: 760,
  auth: 400,
  menuMin: 220,
  menuMax: 360,
  tooltipMax: 280,
  palette: 640,
  fieldLabel: 240,
  aside: 320,
  banner: 480,
  kv: 480,
  settingsRow: 480,
  chartLegend: 480,
  cellSm: 208,
  cellMd: 320,
  cellLg: 512,
} as const;

export const breakpoint = { sidebar: 1024 } as const;
