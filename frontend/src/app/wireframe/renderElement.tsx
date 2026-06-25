// Recursive low-fi renderer: maps an Element-tree DSL node to a grayscale
// wireframe visual. Tolerant of unknown element types. Rendered inside a light
// frame (low-fi look) regardless of the dark app theme.

import type { JSX } from 'react'

export interface Element {
  id: string
  type: string
  props?: Record<string, unknown>
  children?: Element[]
}

const str = (v: unknown): string | undefined => (typeof v === 'string' ? v : undefined)

function GrayLines({ count = 2 }: { count?: number }) {
  return (
    <div className="flex w-full flex-col gap-1.5">
      {Array.from({ length: count }).map((_, i) => (
        <div
          key={i}
          className="h-2 rounded-sm bg-zinc-300"
          style={{ width: `${90 - i * 18}%` }}
        />
      ))}
    </div>
  )
}

export function renderElement(el: Element, depth = 0): JSX.Element {
  const p = el.props ?? {}
  const kids = el.children ?? []
  const renderKids = () => kids.map((c) => renderElement(c, depth + 1))

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
      if (!text) return <GrayLines key={el.id} count={lg ? 1 : 2} />
      return (
        <p
          key={el.id}
          className={lg ? 'text-xl font-semibold text-zinc-800' : 'text-sm text-zinc-500'}
        >
          {text}
        </p>
      )
    }
    case 'button':
      return (
        <div
          key={el.id}
          className="inline-flex w-fit items-center justify-center rounded-md bg-zinc-800 px-5 py-2 text-sm font-medium text-zinc-50"
        >
          {str(p.label) ?? 'Button'}
        </div>
      )
    case 'input':
      return (
        <div
          key={el.id}
          className="flex h-9 w-full items-center rounded-md border border-zinc-300 px-3 text-sm text-zinc-400"
        >
          {str(p.placeholder) ?? ''}
        </div>
      )
    case 'image': {
      const height = typeof p.height === 'number' ? p.height : 180
      return (
        <div
          key={el.id}
          className="relative w-full overflow-hidden rounded-md border border-zinc-300 bg-zinc-100"
          style={{ height }}
        >
          <svg className="absolute inset-0 h-full w-full text-zinc-300" preserveAspectRatio="none">
            <line x1="0" y1="0" x2="100%" y2="100%" stroke="currentColor" strokeWidth="1" />
            <line x1="100%" y1="0" x2="0" y2="100%" stroke="currentColor" strokeWidth="1" />
          </svg>
        </div>
      )
    }
    case 'nav':
      return (
        <div key={el.id} className="flex w-full items-center justify-between">
          <div className="h-5 w-20 rounded bg-zinc-300" />
          <div className="flex gap-3">
            {[0, 1, 2].map((i) => (
              <div key={i} className="h-2.5 w-10 rounded-sm bg-zinc-200" />
            ))}
          </div>
        </div>
      )
    case 'list':
      return (
        <div key={el.id} className="flex w-full flex-col gap-2.5">
          {(kids.length ? kids : [0, 1, 2]).map((_, i) => (
            <div key={i} className="flex items-center gap-3">
              <div className="size-7 shrink-0 rounded-full bg-zinc-200" />
              <GrayLines count={1} />
            </div>
          ))}
        </div>
      )
    default:
      // Unknown type: generic container, still render any children.
      return (
        <div key={el.id} className="w-full rounded-md border border-dashed border-zinc-300 p-3">
          {kids.length ? renderKids() : <GrayLines count={1} />}
        </div>
      )
  }
}
