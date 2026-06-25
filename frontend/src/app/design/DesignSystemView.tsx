import { resolveTokens, type DesignTokens } from './tokens'

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mb-8">
      <h2 className="mb-3 text-xs font-semibold uppercase tracking-wider text-text-dim">
        {title}
      </h2>
      {children}
    </section>
  )
}

function Swatch({ name, hex }: { name: string; hex: string }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div
        className="h-16 w-full rounded-[10px] ring-1 ring-white/10"
        style={{ background: hex }}
      />
      <div className="leading-tight">
        <p className="text-xs font-medium text-text">{name}</p>
        <p className="text-[11px] tabular-nums text-text-dim">{hex}</p>
      </div>
    </div>
  )
}

export function DesignSystemView({ content }: { content: unknown }) {
  const t: DesignTokens = resolveTokens(content)
  const { colors, typography, radius } = t
  const swatches: [string, string][] = [
    ['Primary', colors.primary],
    ['Secondary', colors.secondary],
    ['Accent', colors.accent],
    ['Background', colors.bg],
    ['Surface', colors.surface],
    ['Text', colors.text],
    ['Muted', colors.muted],
  ]
  const typeScale: [string, number][] = [
    ['Heading 1', typography.h1],
    ['Heading 2', typography.h2],
    ['Heading 3', typography.h3],
    ['Body', typography.body],
    ['Caption', typography.caption],
  ]

  return (
    <div className="h-full overflow-y-auto p-8">
      <div className="mx-auto max-w-4xl">
        <h1 className="mb-1 text-2xl font-semibold tracking-tight text-text">Design System</h1>
        <p className="mb-8 text-sm text-text-dim">
          Tokens the AI generated for this project. Ask in the chat to adjust the theme.
        </p>

        <Section title="Colors">
          <div className="grid grid-cols-3 gap-4 sm:grid-cols-4 lg:grid-cols-7">
            {swatches.map(([name, hex]) => (
              <Swatch key={name} name={name} hex={hex} />
            ))}
          </div>
        </Section>

        <Section title={`Typography — ${typography.font}`}>
          <div className="glass flex flex-col gap-3 rounded-[14px] p-5">
            {typeScale.map(([label, size]) => (
              <div key={label} className="flex items-baseline justify-between gap-4">
                <span
                  className="truncate text-text"
                  style={{ fontSize: size, fontWeight: size >= typography.h3 ? 600 : 400 }}
                >
                  {label}
                </span>
                <span className="shrink-0 text-[11px] tabular-nums text-text-dim">{size}px</span>
              </div>
            ))}
          </div>
        </Section>

        <Section title="Components">
          <div className="glass flex flex-wrap items-center gap-4 rounded-[14px] p-5">
            <button
              style={{ background: colors.primary, borderRadius: radius, color: '#fff' }}
              className="px-4 py-2 text-sm font-semibold"
            >
              Primary
            </button>
            <button
              style={{ background: colors.surface, borderRadius: radius, color: colors.text }}
              className="px-4 py-2 text-sm font-medium ring-1 ring-white/10"
            >
              Secondary
            </button>
            <button
              style={{ borderRadius: radius, color: colors.text, border: `1px solid ${colors.muted}` }}
              className="px-4 py-2 text-sm font-medium"
            >
              Outline
            </button>
            {/* toggle */}
            <span
              className="inline-flex h-6 w-11 items-center rounded-full p-0.5"
              style={{ background: colors.primary }}
            >
              <span className="size-5 rounded-full bg-white" />
            </span>
            {/* input */}
            <span
              className="inline-flex h-9 min-w-44 items-center px-3 text-sm"
              style={{
                background: colors.surface,
                borderRadius: radius,
                color: colors.muted,
                border: `1px solid ${colors.muted}33`,
              }}
            >
              Input field
            </span>
          </div>
        </Section>
      </div>
    </div>
  )
}
