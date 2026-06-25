import { useState } from 'react'
import { MinusIcon, PlusIcon } from '@phosphor-icons/react'
import { renderElement, type Element } from './renderElement'
import { resolveFrameSize } from './devices'
import type { DesignTokens } from '../design/tokens'

const DOT_GRID = 'radial-gradient(circle, #2a3142 1px, transparent 1px)'

export function WireframeCanvas({
  root,
  tokens,
  hasDesignSystem,
}: {
  root: Element
  tokens: DesignTokens
  hasDesignSystem: boolean
}) {
  const [zoom, setZoom] = useState(0.7)
  const [hifi, setHifi] = useState(false)
  const { w, h } = resolveFrameSize(root.props)
  const clamp = (z: number) => Math.min(1.5, Math.max(0.3, Math.round(z * 100) / 100))

  return (
    <div className="relative h-full w-full overflow-hidden">
      <div className="absolute inset-0" style={{ backgroundImage: DOT_GRID, backgroundSize: '22px 22px' }} />

      {/* Fidelity toggle */}
      <div className="glass absolute right-4 top-4 z-10 flex items-center gap-0.5 rounded-full p-1 text-xs">
        {(['low', 'hi'] as const).map((mode) => {
          const on = (mode === 'hi') === hifi
          return (
            <button
              key={mode}
              onClick={() => setHifi(mode === 'hi')}
              className={`rounded-full px-3 py-1 font-medium transition ${
                on ? 'bg-white/10 text-text' : 'text-text-dim hover:text-text'
              }`}
            >
              {mode === 'hi' ? 'Hi-fi' : 'Low-fi'}
            </button>
          )
        })}
      </div>
      {hifi && !hasDesignSystem && (
        <p className="glass absolute left-1/2 top-4 z-10 -translate-x-1/2 rounded-full px-3 py-1.5 text-xs text-text-dim">
          Using default theme. Generate a Design System to theme this screen.
        </p>
      )}

      {/* Scaled device frame */}
      <div className="absolute inset-0 grid place-items-center overflow-auto p-10">
        <div
          style={{
            width: w,
            minHeight: h,
            transform: `scale(${zoom})`,
            transformOrigin: 'center',
            background: hifi ? tokens.colors.bg : '#ffffff',
            color: hifi ? tokens.colors.text : '#18181b',
          }}
          className="shrink-0 overflow-hidden rounded-[18px] shadow-[0_24px_80px_rgba(0,0,0,0.5)] ring-1 ring-black/10"
        >
          {renderElement(root, hifi ? tokens : undefined)}
        </div>
      </div>

      {/* Zoom controls */}
      <div className="glass absolute bottom-4 left-1/2 z-10 flex -translate-x-1/2 items-center gap-1 rounded-full px-2 py-1.5">
        <button
          onClick={() => setZoom((z) => clamp(z - 0.1))}
          aria-label="Zoom out"
          className="grid size-7 place-items-center rounded-full text-text-dim transition hover:bg-white/10 hover:text-text"
        >
          <MinusIcon size={14} />
        </button>
        <span className="w-12 text-center text-xs tabular-nums text-text-muted">
          {Math.round(zoom * 100)}%
        </span>
        <button
          onClick={() => setZoom((z) => clamp(z + 0.1))}
          aria-label="Zoom in"
          className="grid size-7 place-items-center rounded-full text-text-dim transition hover:bg-white/10 hover:text-text"
        >
          <PlusIcon size={14} />
        </button>
      </div>
    </div>
  )
}
