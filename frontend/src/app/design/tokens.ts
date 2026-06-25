// Design-system token resolution. Shared by the Design System view and the
// hi-fi screen renderer. Tolerant of partial AI output via defaults.

export interface DesignTokens {
  colors: {
    primary: string
    secondary: string
    accent: string
    bg: string
    surface: string
    text: string
    muted: string
  }
  typography: {
    font: string
    h1: number
    h2: number
    h3: number
    body: number
    caption: number
  }
  radius: number
  spacing: { sm: number; md: number; lg: number }
}

export const DEFAULT_TOKENS: DesignTokens = {
  colors: {
    primary: '#6366f1',
    secondary: '#0ea5e9',
    accent: '#2dd4bf',
    bg: '#0b1020',
    surface: '#161b2e',
    text: '#e8ecf6',
    muted: '#9aa3bd',
  },
  typography: { font: 'Inter', h1: 40, h2: 30, h3: 22, body: 16, caption: 13 },
  radius: 12,
  spacing: { sm: 8, md: 16, lg: 24 },
}

type Dict = Record<string, unknown>
const obj = (v: unknown): Dict => (v && typeof v === 'object' ? (v as Dict) : {})
const s = (v: unknown, d: string): string => (typeof v === 'string' ? v : d)
const n = (v: unknown, d: number): number => (typeof v === 'number' ? v : d)

/** Read a design_system artifact's `content.tokens` into typed tokens + defaults. */
export function resolveTokens(content: unknown): DesignTokens {
  const t = obj(obj(content).tokens)
  const c = obj(t.colors)
  const ty = obj(t.typography)
  const sp = obj(t.spacing)
  const d = DEFAULT_TOKENS
  return {
    colors: {
      primary: s(c.primary, d.colors.primary),
      secondary: s(c.secondary, d.colors.secondary),
      accent: s(c.accent, d.colors.accent),
      bg: s(c.bg, d.colors.bg),
      surface: s(c.surface, d.colors.surface),
      text: s(c.text, d.colors.text),
      muted: s(c.muted, d.colors.muted),
    },
    typography: {
      font: s(ty.font, d.typography.font),
      h1: n(ty.h1, d.typography.h1),
      h2: n(ty.h2, d.typography.h2),
      h3: n(ty.h3, d.typography.h3),
      body: n(ty.body, d.typography.body),
      caption: n(ty.caption, d.typography.caption),
    },
    radius: n(t.radius, d.radius),
    spacing: {
      sm: n(sp.sm, d.spacing.sm),
      md: n(sp.md, d.spacing.md),
      lg: n(sp.lg, d.spacing.lg),
    },
  }
}
