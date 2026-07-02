import { useRef, useState, type PointerEvent as RPointerEvent } from 'react'
import {
  XIcon,
  SpinnerGapIcon,
  SparkleIcon,
  EraserIcon,
  PaintBrushIcon,
  SwatchesIcon,
  BackspaceIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'
import { Dialog } from '../ui/Dialog'

// Per-intent routing: each edit takes the pipeline that's actually good at it.
// Recolor is deterministic math (diffusion fights colour changes); remove and
// replace are diffusion inpaint with intent-tuned prompts.
const MODES: { id: api.EditIntent; label: string; hint: string }[] = [
  { id: 'replace', label: 'Replace', hint: 'brush the area, describe what it becomes' },
  { id: 'remove', label: 'Remove', hint: 'brush the object to erase — background fills in' },
  { id: 'recolor', label: 'Recolor', hint: 'brush the area, pick its new colour — exact & instant' },
]

/**
 * Masked region edit (B2). Brush over the region, pick an intent (replace /
 * remove / recolor) → a new version (the original stays in history). The mask
 * canvas overlays the image at its natural resolution, so painted pixels map
 * 1:1 to the asset. Replace/remove run diffusion inpaint (free in mock mode);
 * recolor is deterministic and always free.
 */
export function InpaintDialog({
  asset,
  onClose,
  onSaved,
}: {
  asset: api.Asset
  onClose: () => void
  onSaved: (a: api.Asset) => void
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const drawing = useRef(false)
  const [ready, setReady] = useState(false)
  const [mode, setMode] = useState<api.EditIntent>('replace')
  const [prompt, setPrompt] = useState('')
  const [color, setColor] = useState('#22aa55')
  const [brush, setBrush] = useState(28)
  const [hasMask, setHasMask] = useState(false)
  const [useCanon, setUseCanon] = useState(true)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  function onImgLoad(e: React.SyntheticEvent<HTMLImageElement>) {
    const img = e.currentTarget
    const cv = canvasRef.current!
    cv.width = img.naturalWidth
    cv.height = img.naturalHeight
    cv.getContext('2d')!.clearRect(0, 0, cv.width, cv.height)
    setReady(true)
  }

  function paintAt(e: RPointerEvent) {
    const cv = canvasRef.current!
    const r = cv.getBoundingClientRect()
    const x = ((e.clientX - r.left) / r.width) * cv.width
    const y = ((e.clientY - r.top) / r.height) * cv.height
    const ctx = cv.getContext('2d')!
    // Opaque teal at full alpha (the canvas is shown at reduced CSS opacity, so
    // it reads as a translucent highlight). Exported it's a bright, opaque
    // selection the backend mask test picks up.
    ctx.fillStyle = 'rgb(45, 212, 191)'
    ctx.beginPath()
    ctx.arc(x, y, brush, 0, Math.PI * 2)
    ctx.fill()
    setHasMask(true)
  }

  function clear() {
    const cv = canvasRef.current!
    cv.getContext('2d')!.clearRect(0, 0, cv.width, cv.height)
    setHasMask(false)
  }

  const canApply = hasMask && (mode === 'replace' ? !!prompt.trim() : mode === 'recolor' ? !!color : true)

  async function apply() {
    if (!canApply || busy) return
    setBusy(true)
    setError(null)
    try {
      const mask = canvasRef.current!.toDataURL('image/png')
      const updated = await api.inpaintAsset(asset.id, mask, {
        mode,
        prompt: mode === 'replace' ? prompt.trim() : undefined,
        color: mode === 'recolor' ? color : undefined,
        useCanon,
      })
      onSaved(updated)
      onClose()
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Edit failed.')
    } finally {
      setBusy(false)
    }
  }

  const modeHint = MODES.find((m) => m.id === mode)!.hint

  return (
    <Dialog
      onClose={onClose}
      z="z-[70]"
      panelClassName="glass flex max-h-[92dvh] w-full max-w-3xl flex-col overflow-hidden rounded-[16px]"
    >
      {({ titleId }) => (
        <>
          <header className="flex items-center gap-2 border-b border-white/8 px-4 py-3">
            <span className="grid size-7 place-items-center rounded-[8px] bg-accent/15 text-teal-bright">
              <PaintBrushIcon size={15} weight="fill" />
            </span>
            <h2 id={titleId} className="text-sm font-medium text-text">Edit a region</h2>
            <span className="text-xs text-text-dim">· {modeHint}</span>
            <button
              onClick={onClose}
              aria-label="Close"
              className="ml-auto grid size-7 place-items-center rounded-[8px] text-text-dim transition hover:bg-white/5 hover:text-text"
            >
              <XIcon size={16} />
            </button>
          </header>

          {/* Canvas */}
          <div className="min-h-0 flex-1 overflow-auto bg-surface/40 p-4">
            <div className="relative mx-auto w-fit">
              <img
                src={asset.url}
                alt=""
                onLoad={onImgLoad}
                draggable={false}
                className="max-h-[58dvh] max-w-full rounded-[10px] ring-1 ring-white/10 select-none"
              />
              <canvas
                ref={canvasRef}
                onPointerDown={(e) => {
                  ;(e.target as HTMLCanvasElement).setPointerCapture(e.pointerId)
                  drawing.current = true
                  paintAt(e)
                }}
                onPointerMove={(e) => drawing.current && paintAt(e)}
                onPointerUp={() => (drawing.current = false)}
                onPointerLeave={() => (drawing.current = false)}
                className="absolute inset-0 size-full cursor-crosshair touch-none rounded-[10px] opacity-50"
              />
            </div>
            {!ready && <p className="py-6 text-center text-xs text-text-dim">Loading image…</p>}
          </div>

          {/* Controls */}
          <div className="border-t border-white/8 p-3">
            {error && <p className="mb-2 text-xs text-rose-300">{error}</p>}

            {/* Intent switch — each mode routes to the pipeline that's good at it */}
            <div className="mb-2 flex items-center gap-2">
              <div className="flex items-center rounded-[8px] bg-surface/60 p-0.5">
                {MODES.map((m) => (
                  <button
                    key={m.id}
                    type="button"
                    onClick={() => setMode(m.id)}
                    title={m.hint}
                    className={`rounded-[6px] px-2.5 py-1 text-xs font-medium transition ${
                      mode === m.id ? 'bg-teal text-bg' : 'text-text-dim hover:text-text'
                    }`}
                  >
                    {m.label}
                  </button>
                ))}
              </div>
              {mode === 'recolor' && (
                <span className="text-[10px] text-text-dim">free · exact · keeps shading</span>
              )}
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <label className="flex items-center gap-1.5 text-xs text-text-dim">
                <PaintBrushIcon size={13} />
                <input
                  type="range"
                  min={6}
                  max={80}
                  value={brush}
                  onChange={(e) => setBrush(Number(e.target.value))}
                  className="w-24 accent-teal"
                  aria-label="Brush size"
                />
              </label>
              <button
                onClick={clear}
                disabled={!hasMask}
                className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-2.5 py-1.5 text-xs text-text-dim transition hover:text-text disabled:opacity-40"
              >
                <EraserIcon size={13} />
                Clear
              </button>

              {mode === 'replace' && (
                <input
                  value={prompt}
                  onChange={(e) => setPrompt(e.target.value)}
                  placeholder="What should the region become? (e.g. red hat, wooden shield)"
                  aria-label="Describe what the masked region should become"
                  className="min-w-0 flex-1 rounded-[8px] bg-surface-2/60 px-3 py-2 text-sm text-text outline-none placeholder:text-text-dim focus:ring-1 focus:ring-teal/40"
                />
              )}
              {mode === 'remove' && (
                <span className="min-w-0 flex-1 truncate text-xs text-text-dim">
                  The brushed content is erased and the background continues through it.
                </span>
              )}
              {mode === 'recolor' && (
                <label className="flex min-w-0 flex-1 items-center gap-2 text-xs text-text-dim">
                  <input
                    type="color"
                    value={color}
                    onChange={(e) => setColor(e.target.value)}
                    aria-label="Target colour"
                    className="size-8 shrink-0 cursor-pointer rounded-[6px] border border-white/10 bg-surface-2/60 p-0.5"
                  />
                  <span className="font-mono text-text">{color}</span>
                  <span className="truncate text-[10px]">brushed pixels shift to this colour, shading kept</span>
                </label>
              )}

              <button
                onClick={apply}
                disabled={busy || !canApply}
                className="inline-flex shrink-0 items-center gap-1.5 rounded-[8px] bg-teal px-3.5 py-2 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
              >
                {busy ? (
                  <SpinnerGapIcon size={14} className="animate-spin" />
                ) : mode === 'recolor' ? (
                  <SwatchesIcon size={14} weight="fill" />
                ) : mode === 'remove' ? (
                  <BackspaceIcon size={14} weight="fill" />
                ) : (
                  <SparkleIcon size={14} weight="fill" />
                )}
                {mode === 'recolor' ? 'Recolor' : mode === 'remove' ? 'Remove' : 'Inpaint'}
              </button>
            </div>

            {/* Canon style only applies to diffusion replace — recolor is exact
                by definition and remove has no style to match. */}
            {mode === 'replace' && (
              <label className="mt-2 flex items-center gap-2 text-xs text-text-dim">
                <input
                  type="checkbox"
                  checked={useCanon}
                  onChange={(e) => setUseCanon(e.target.checked)}
                  className="size-3.5 accent-teal"
                />
                Apply canon style
                <span className="text-[10px] text-text-dim/70">
                  · matches the project’s look — turn off for off-canon edits
                </span>
              </label>
            )}
          </div>
        </>
      )}
    </Dialog>
  )
}
