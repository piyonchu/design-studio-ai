// Recursive renderer: maps an Element-tree DSL node to a visual. Two modes:
//   - no theme  -> grayscale low-fi wireframe (light frame)
//   - theme set -> hi-fi, painted with the project's design-system tokens
// Tolerant of unknown element types.

import type { JSX } from 'react'
import type { DesignTokens } from '../design/tokens'

export interface Element {
  id: string
  type: string
  props?: Record<string, unknown>
  children?: Element[]
}

const str = (v: unknown): string | undefined => (typeof v === 'string' ? v : undefined)

function GrayLines({ count = 2, color = '#d4d4d8' }: { count?: number; color?: string }) {
  return (
    <div className="flex w-full flex-col gap-1.5">
      {Array.from({ length: count }).map((_, i) => (
        <div
          key={i}
          className="h-2 rounded-sm"
          style={{ width: `${90 - i * 18}%`, background: color }}
        />
      ))}
    </div>
  )
}

export function renderElement(el: Element, theme?: DesignTokens, depth = 0): JSX.Element {
  const p = el.props ?? {}
  const kids = el.children ?? []
  const hifi = !!theme
  const renderKids = () => kids.map((c) => renderElement(c, theme, depth + 1))

  switch (el.type) {
    case 'frame': {
      const dir = str(p.direction) === 'row' ? 'row' : 'column'
      const gap = typeof p.gap === 'number' ? p.gap : 14
      const pad = typeof p.padding === 'number' ? p.padding : depth === 0 ? 28 : 0
      return (
        <div
          key={el.id}
          style={{ display: 'flex', flexDirection: dir, gap, padding: pad }}
          className={dir === 'row' ? 'items-center' : ''}
        >
          {renderKids()}
        </div>
      )
    }
    case 'text': {
      const text = str(p.text)
      const lg = str(p.size) === 'lg'
      if (!text) return <GrayLines key={el.id} count={lg ? 1 : 2} color={hifi ? theme!.colors.muted + '55' : '#d4d4d8'} />
      if (hifi) {
        const t = theme!
        return (
          <p
            key={el.id}
            style={{
              fontFamily: t.typography.font,
              fontSize: lg ? t.typography.h2 : t.typography.body,
              fontWeight: lg ? 700 : 400,
              color: lg ? t.colors.text : t.colors.muted,
            }}
          >
            {text}
          </p>
        )
      }
      return (
        <p key={el.id} className={lg ? 'text-xl font-semibold text-zinc-800' : 'text-sm text-zinc-500'}>
          {text}
        </p>
      )
    }
    case 'button':
      return (
        <div
          key={el.id}
          className="inline-flex w-fit items-center justify-center px-5 py-2 text-sm font-semibold"
          style={
            hifi
              ? { background: theme!.colors.primary, color: '#fff', borderRadius: theme!.radius }
              : { background: '#27272a', color: '#fafafa', borderRadius: 6 }
          }
        >
          {str(p.label) ?? 'Button'}
        </div>
      )
    case 'input':
      return (
        <div
          key={el.id}
          className="flex h-9 w-full items-center px-3 text-sm"
          style={
            hifi
              ? {
                  background: theme!.colors.surface,
                  color: theme!.colors.muted,
                  borderRadius: theme!.radius,
                  border: `1px solid ${theme!.colors.muted}33`,
                }
              : { color: '#a1a1aa', borderRadius: 6, border: '1px solid #d4d4d8' }
          }
        >
          {str(p.placeholder) ?? ''}
        </div>
      )
    case 'image': {
      const src = str(p.src)
      return (
        <div
          key={el.id}
          className="relative w-full overflow-hidden"
          style={{
            height: typeof p.height === 'number' ? p.height : 180,
            borderRadius: hifi ? theme!.radius : 6,
            background: hifi
              ? `linear-gradient(135deg, ${theme!.colors.primary}55, ${theme!.colors.accent}40)`
              : '#f4f4f5',
            border: hifi ? 'none' : '1px solid #d4d4d8',
          }}
        >
          {src && (
            <img src={src} alt="" className="absolute inset-0 h-full w-full object-cover" />
          )}
          {!hifi && !src && (
            <svg className="absolute inset-0 h-full w-full text-zinc-300" preserveAspectRatio="none">
              <line x1="0" y1="0" x2="100%" y2="100%" stroke="currentColor" strokeWidth="1" />
              <line x1="100%" y1="0" x2="0" y2="100%" stroke="currentColor" strokeWidth="1" />
            </svg>
          )}
        </div>
      )
    }
    case 'nav':
      return (
        <div key={el.id} className="flex w-full items-center justify-between">
          <div
            className="h-5 w-20 rounded"
            style={{ background: hifi ? theme!.colors.primary : '#d4d4d8' }}
          />
          <div className="flex gap-3">
            {[0, 1, 2].map((i) => (
              <div
                key={i}
                className="h-2.5 w-10 rounded-sm"
                style={{ background: hifi ? theme!.colors.muted + '66' : '#e4e4e7' }}
              />
            ))}
          </div>
        </div>
      )
    case 'list':
      return (
        <div key={el.id} className="flex w-full flex-col gap-2.5">
          {(kids.length ? kids : [0, 1, 2]).map((_, i) => (
            <div key={i} className="flex items-center gap-3">
              <div
                className="size-7 shrink-0 rounded-full"
                style={{ background: hifi ? theme!.colors.accent + '55' : '#e4e4e7' }}
              />
              <GrayLines count={1} color={hifi ? theme!.colors.muted + '55' : '#d4d4d8'} />
            </div>
          ))}
        </div>
      )
    default:
      return (
        <div
          key={el.id}
          className="w-full rounded-md p-3"
          style={{ border: `1px dashed ${hifi ? theme!.colors.muted + '55' : '#d4d4d8'}` }}
        >
          {kids.length ? renderKids() : <GrayLines count={1} color={hifi ? theme!.colors.muted + '55' : '#d4d4d8'} />}
        </div>
      )
  }
}
