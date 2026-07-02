import { useRef, useState, type PointerEvent as RPointerEvent } from 'react'
import { XIcon, SpinnerGapIcon, SparkleIcon, EraserIcon, PaintBrushIcon } from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'
import { Dialog } from '../ui/Dialog'

/**
 * Masked / inpaint edit (B2). Brush over the region to change, type what it
 * should become, and only that region is regenerated → a new version (the
 * original stays in history). Backed by the `ai::edit` provider seam; free in
 * mock mode. The mask canvas overlays the image at its natural resolution, so
 * the painted pixels map 1:1 to the asset.
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
  const [prompt, setPrompt] = useState('')
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

  async function apply() {
    if (!hasMask || !prompt.trim() || busy) return
    setBusy(true)
    setError(null)
    try {
      const mask = canvasRef.current!.toDataURL('image/png')
      const updated = await api.inpaintAsset(asset.id, mask, prompt.trim(), useCanon)
      onSaved(updated)
      onClose()
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Inpaint failed.')
    } finally {
      setBusy(false)
    }
  }

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
            <span className="text-xs text-text-dim">· brush the area, describe the change</span>
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
              <input
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                placeholder="What should the region become? (e.g. red hat, remove the extra finger)"
                aria-label="Describe what the masked region should become"
                className="min-w-0 flex-1 rounded-[8px] bg-surface-2/60 px-3 py-2 text-sm text-text outline-none placeholder:text-text-dim focus:ring-1 focus:ring-teal/40"
              />
              <button
                onClick={apply}
                disabled={busy || !hasMask || !prompt.trim()}
                className="inline-flex shrink-0 items-center gap-1.5 rounded-[8px] bg-teal px-3.5 py-2 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
              >
                {busy ? <SpinnerGapIcon size={14} className="animate-spin" /> : <SparkleIcon size={14} weight="fill" />}
                Inpaint
              </button>
            </div>
            {/* Fold the project's canon style into the edit. Off for off-canon
                assets or changes the canon would fight (e.g. a recolor). */}
            <label className="mt-2 flex items-center gap-2 text-xs text-text-dim">
              <input
                type="checkbox"
                checked={useCanon}
                onChange={(e) => setUseCanon(e.target.checked)}
                className="size-3.5 accent-teal"
              />
              Apply canon style
              <span className="text-[10px] text-text-dim/70">
                · matches the project’s look — turn off for off-canon or exact-colour edits
              </span>
            </label>
          </div>
        </>
      )}
    </Dialog>
  )
}
