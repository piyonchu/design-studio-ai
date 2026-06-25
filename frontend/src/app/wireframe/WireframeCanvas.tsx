import { useState } from 'react'
import { MinusIcon, PlusIcon } from '@phosphor-icons/react'
import { renderElement, type Element } from './renderElement'
import { resolveFrameSize } from './devices'

const DOT_GRID =
  'radial-gradient(circle, #2a3142 1px, transparent 1px)'

export function WireframeCanvas({ root }: { root: Element }) {
  const [zoom, setZoom] = useState(0.7)
  const { w, h } = resolveFrameSize(root.props)
  const clamp = (z: number) => Math.min(1.5, Math.max(0.3, Math.round(z * 100) / 100))

  return (
    <div className="relative h-full w-full overflow-hidden">
      <div
        className="absolute inset-0"
        style={{ backgroundImage: DOT_GRID, backgroundSize: '22px 22px' }}
      />

      {/* Scaled light device frame, centered, scroll to pan when zoomed in. */}
      <div className="absolute inset-0 grid place-items-center overflow-auto p-10">
        <div
          style={{
            width: w,
            minHeight: h, // grows if content is taller than the device height
            transform: `scale(${zoom})`,
            transformOrigin: 'center',
          }}
          className="shrink-0 overflow-hidden rounded-[18px] bg-white text-zinc-900 shadow-[0_24px_80px_rgba(0,0,0,0.5)] ring-1 ring-black/10"
        >
          {renderElement(root)}
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
