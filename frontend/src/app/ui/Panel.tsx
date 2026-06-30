import type { ComponentProps, ReactNode } from 'react'

type PanelProps = ComponentProps<'div'> & {
  overflow?: 'hidden' | 'visible'
  layout?: 'stack' | 'split'
}

/** Glass workspace panel — the standard shell for project tabs. */
export function Panel({
  className = '',
  overflow = 'hidden',
  layout = 'stack',
  children,
  ...rest
}: PanelProps) {
  const flexDir = layout === 'split' ? 'flex-row' : 'flex-col'
  return (
    <div
      className={`glass flex min-h-0 min-w-0 w-full flex-1 ${flexDir} rounded-[var(--radius-panel)] ${
        overflow === 'hidden' ? 'overflow-hidden' : ''
      } ${className}`}
      {...rest}
    >
      {children}
    </div>
  )
}

type PanelHeaderProps = ComponentProps<'div'> & {
  size?: 'default' | 'compact'
}

/** Panel title bar — px-5 py-4 by default; compact for nested rails. */
export function PanelHeader({ className = '', size = 'default', children, ...rest }: PanelHeaderProps) {
  const pad = size === 'compact' ? 'px-3 py-3' : 'px-5 py-4'
  return (
    <div
      className={`flex shrink-0 items-center gap-2 border-b border-white/8 ${pad} ${className}`}
      {...rest}
    >
      {children}
    </div>
  )
}

/** Secondary action row below the header (batch bars, filters, forms). */
export function PanelToolbar({ className = '', children, ...rest }: ComponentProps<'div'>) {
  return (
    <div
      className={`flex shrink-0 flex-wrap items-center gap-2 border-b border-white/8 px-5 py-3 ${className}`}
      {...rest}
    >
      {children}
    </div>
  )
}

type PanelBodyProps = ComponentProps<'div'> & {
  density?: 'default' | 'dense' | 'flush'
  scroll?: boolean
}

/** Scrollable panel content — default p-5; dense p-3 for rails and lists. */
export function PanelBody({
  className = '',
  density = 'default',
  scroll = true,
  children,
  ...rest
}: PanelBodyProps) {
  const pad = { default: 'p-5', dense: 'p-3', flush: '' }[density]
  return (
    <div
      className={`min-h-0 flex-1 ${scroll ? 'overflow-y-auto' : ''} ${pad} ${className}`}
      {...rest}
    >
      {children}
    </div>
  )
}

/** Header icon badge — teal tint by default; pass className to override. */
export function PanelIcon({ children, className = '' }: { children: ReactNode; className?: string }) {
  return (
    <span
      className={`grid size-7 shrink-0 place-items-center rounded-[8px] bg-accent/15 text-teal-bright ${className}`}
    >
      {children}
    </span>
  )
}

/** Filter-rail section — tight label + chip stack with generous separation between groups. */
export function RailSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mb-5 last:mb-0">
      <p className="mb-1.5 px-2.5 text-[10px] font-semibold uppercase tracking-wider text-text-dim">{title}</p>
      <div className="flex flex-col gap-0.5">{children}</div>
    </section>
  )
}

/** Centered form column used in toolbars and editors. */
export function ContentWell({ children, className = '' }: { children: ReactNode; className?: string }) {
  return <div className={`mx-auto w-full max-w-2xl ${className}`}>{children}</div>
}

/** Footer action row (save bars, confirmations). */
export function PanelFooter({ className = '', children, ...rest }: ComponentProps<'div'>) {
  return (
    <div
      className={`flex shrink-0 flex-wrap items-center gap-3 border-t border-white/8 px-5 py-3 ${className}`}
      {...rest}
    >
      {children}
    </div>
  )
}

/** In-panel alert slot — aligns banners with panel padding rhythm. */
export function PanelInset({ className = '', children, ...rest }: ComponentProps<'div'>) {
  return (
    <div className={`shrink-0 px-5 pt-3 empty:hidden ${className}`} {...rest}>
      {children}
    </div>
  )
}
