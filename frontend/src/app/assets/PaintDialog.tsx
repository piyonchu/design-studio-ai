import { useRef, useState, type PointerEvent as RPointerEvent } from 'react'
import {
  XIcon,
  SpinnerGapIcon,
  CheckIcon,
  EraserIcon,
  PaintBrushIcon,
  PaintBucketIcon,
  EyedropperIcon,
  ArrowUUpLeftIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'

type Tool = 'brush' | 'eraser' | 'bucket' | 'eyedropper'

/**
 * Manual paint editor (Pro pipeline B3). Hand-edit an asset's pixels — brush,
 * eraser, paint bucket (flood fill), eyedropper — then Save as a new version
 * (non-destructive; the original stays in history). All client-side, no model,
 * free. Pixel-crisp by default (nearest-neighbour) for game art, with a smooth
 * toggle for the illustration verticals.
 */
export function PaintDialog({
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
  const undoStack = useRef<ImageData[]>([])
  const [ready, setReady] = useState(false)
  const [tool, setTool] = useState<Tool>('brush')
  const [color, setColor] = useState('#ffffff')
  const [size, setSize] = useState(8)
  const [tolerance, setTolerance] = useState(32)
  const [pixel, setPixel] = useState(true)
  const [dirty, setDirty] = useState(false)
  const [canUndo, setCanUndo] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  function ctx() {
    return canvasRef.current!.getContext('2d', { willReadFrequently: true })!
  }

  function onImgLoad(e: React.SyntheticEvent<HTMLImageElement>) {
    const img = e.currentTarget
    const cv = canvasRef.current!
    cv.width = img.naturalWidth
    cv.height = img.naturalHeight
    const c = ctx()
    c.imageSmoothingEnabled = false
    c.drawImage(img, 0, 0)
    undoStack.current = []
    setCanUndo(false)
    setReady(true)
  }

  /** Snapshot the canvas for undo (capped). Call before a mutating action. */
  function snapshot() {
    const cv = canvasRef.current!
    undoStack.current.push(ctx().getImageData(0, 0, cv.width, cv.height))
    if (undoStack.current.length > 25) undoStack.current.shift()
    setCanUndo(true)
  }

  function undo() {
    const prev = undoStack.current.pop()
    if (!prev) return
    ctx().putImageData(prev, 0, 0)
    setCanUndo(undoStack.current.length > 0)
  }

  /** Canvas pixel coords from a pointer event. */
  function coords(e: RPointerEvent): [number, number] {
    const cv = canvasRef.current!
    const r = cv.getBoundingClientRect()
    const x = Math.floor(((e.clientX - r.left) / r.width) * cv.width)
    const y = Math.floor(((e.clientY - r.top) / r.height) * cv.height)
    return [x, y]
  }

  function hexToRgb(hex: string): [number, number, number] {
    const n = parseInt(hex.slice(1), 16)
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255]
  }

  function dab(x: number, y: number) {
    const c = ctx()
    if (tool === 'eraser') {
      c.globalCompositeOperation = 'destination-out'
      c.fillStyle = 'rgba(0,0,0,1)'
    } else {
      c.globalCompositeOperation = 'source-over'
      c.fillStyle = color
    }
    if (pixel) {
      // Crisp, integer-aligned square — paints whole pixels (no anti-alias).
      const s = Math.max(1, Math.round(size))
      c.fillRect(Math.round(x - s / 2), Math.round(y - s / 2), s, s)
    } else {
      c.beginPath()
      c.arc(x, y, size / 2, 0, Math.PI * 2)
      c.fill()
    }
    c.globalCompositeOperation = 'source-over'
    setDirty(true)
  }

  /** Scanline-ish stack flood fill with a per-channel tolerance. */
  function bucketFill(sx: number, sy: number) {
    const cv = canvasRef.current!
    const w = cv.width
    const h = cv.height
    const c = ctx()
    const img = c.getImageData(0, 0, w, h)
    const d = img.data
    const at = (x: number, y: number) => (y * w + x) * 4
    const si = at(sx, sy)
    const tgt = [d[si], d[si + 1], d[si + 2], d[si + 3]]
    const [fr, fg, fb] = hexToRgb(color)
    const tol = tolerance
    const match = (i: number) =>
      Math.abs(d[i] - tgt[0]) <= tol &&
      Math.abs(d[i + 1] - tgt[1]) <= tol &&
      Math.abs(d[i + 2] - tgt[2]) <= tol &&
      Math.abs(d[i + 3] - tgt[3]) <= tol
    if (fr === tgt[0] && fg === tgt[1] && fb === tgt[2] && tgt[3] === 255) return
    const seen = new Uint8Array(w * h)
    const stack: number[] = [sx, sy]
    while (stack.length) {
      const y = stack.pop()!
      const x = stack.pop()!
      if (x < 0 || y < 0 || x >= w || y >= h) continue
      const p = y * w + x
      if (seen[p]) continue
      const i = p * 4
      if (!match(i)) continue
      seen[p] = 1
      d[i] = fr
      d[i + 1] = fg
      d[i + 2] = fb
      d[i + 3] = 255
      stack.push(x + 1, y, x - 1, y, x, y + 1, x, y - 1)
    }
    c.putImageData(img, 0, 0)
    setDirty(true)
  }

  function pick(x: number, y: number) {
    const p = ctx().getImageData(x, y, 1, 1).data
    setColor('#' + [p[0], p[1], p[2]].map((v) => v.toString(16).padStart(2, '0')).join(''))
    setTool('brush')
  }

  function onDown(e: RPointerEvent) {
    if (!ready) return
    ;(e.target as HTMLCanvasElement).setPointerCapture(e.pointerId)
    const [x, y] = coords(e)
    if (tool === 'eyedropper') {
      pick(x, y)
      return
    }
    snapshot()
    if (tool === 'bucket') {
      bucketFill(x, y)
      return
    }
    drawing.current = true
    dab(x, y)
  }

  async function save() {
    if (!dirty || busy) return
    setBusy(true)
    setError(null)
    try {
      const blob: Blob = await new Promise((res, rej) =>
        canvasRef.current!.toBlob((b) => (b ? res(b) : rej(new Error('encode failed'))), 'image/png'),
      )
      const updated = await api.saveAssetVersion(asset.id, blob, 'Hand-painted')
      onSaved(updated)
      onClose()
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Save failed.')
    } finally {
      setBusy(false)
    }
  }

  const TOOLS: { id: Tool; label: string; icon: typeof PaintBrushIcon }[] = [
    { id: 'brush', label: 'Brush', icon: PaintBrushIcon },
    { id: 'eraser', label: 'Eraser', icon: EraserIcon },
    { id: 'bucket', label: 'Fill', icon: PaintBucketIcon },
    { id: 'eyedropper', label: 'Pick', icon: EyedropperIcon },
  ]

  return (
    <>
      <div className="fixed inset-0 z-[60] bg-black/60" onClick={onClose} aria-hidden />
      <div className="fixed inset-0 z-[70] grid place-items-center p-4" role="dialog" aria-modal>
        <div className="glass flex max-h-[92dvh] w-full max-w-3xl flex-col overflow-hidden rounded-[16px]">
          <header className="flex items-center gap-2 border-b border-white/8 px-4 py-3">
            <span className="grid size-7 place-items-center rounded-[8px] bg-accent/15 text-teal-bright">
              <PaintBrushIcon size={15} weight="fill" />
            </span>
            <p className="text-sm font-medium text-text">Paint</p>
            <span className="text-xs text-text-dim">· hand-edit, saved as a new version</span>
            <button
              onClick={onClose}
              aria-label="Close"
              className="ml-auto grid size-7 place-items-center rounded-[8px] text-text-dim transition hover:bg-white/5 hover:text-text"
            >
              <XIcon size={16} />
            </button>
          </header>

          {/* Canvas (checker bg shows transparency) */}
          <div
            className="min-h-0 flex-1 overflow-auto p-4"
            style={{
              backgroundImage:
                'repeating-conic-gradient(#2a2d35 0% 25%, #1f2229 0% 50%)',
              backgroundSize: '16px 16px',
            }}
          >
            <div className="mx-auto w-fit">
              <img src={asset.url} alt="" onLoad={onImgLoad} className="hidden" />
              <canvas
                ref={canvasRef}
                onPointerDown={onDown}
                onPointerMove={(e) => drawing.current && dab(...coords(e))}
                onPointerUp={() => (drawing.current = false)}
                onPointerLeave={() => (drawing.current = false)}
                style={{ imageRendering: pixel ? 'pixelated' : 'auto' }}
                className="max-h-[56dvh] max-w-full cursor-crosshair touch-none rounded-[10px] ring-1 ring-white/10"
              />
            </div>
            {!ready && <p className="py-6 text-center text-xs text-text-dim">Loading image…</p>}
          </div>

          {/* Controls */}
          <div className="border-t border-white/8 p-3">
            {error && <p className="mb-2 text-xs text-rose-300">{error}</p>}
            <div className="flex flex-wrap items-center gap-2">
              {/* Tools */}
              <div className="flex items-center rounded-[8px] bg-surface/60 p-0.5">
                {TOOLS.map(({ id, label, icon: Icon }) => (
                  <button
                    key={id}
                    onClick={() => setTool(id)}
                    title={label}
                    aria-label={label}
                    className={`grid size-7 place-items-center rounded-[6px] transition ${
                      tool === id ? 'bg-teal text-bg' : 'text-text-dim hover:text-text'
                    }`}
                  >
                    <Icon size={14} weight={tool === id ? 'fill' : 'regular'} />
                  </button>
                ))}
              </div>

              <input
                type="color"
                value={color}
                onChange={(e) => setColor(e.target.value)}
                aria-label="Color"
                className="size-8 shrink-0 cursor-pointer rounded-[8px] border border-white/10 bg-transparent"
              />

              {tool === 'bucket' ? (
                <label className="flex items-center gap-1.5 text-xs text-text-dim" title="Fill tolerance">
                  Tol
                  <input
                    type="range"
                    min={0}
                    max={128}
                    value={tolerance}
                    onChange={(e) => setTolerance(Number(e.target.value))}
                    className="w-20 accent-teal"
                  />
                </label>
              ) : (
                <label className="flex items-center gap-1.5 text-xs text-text-dim" title="Brush size">
                  <PaintBrushIcon size={13} />
                  <input
                    type="range"
                    min={1}
                    max={64}
                    value={size}
                    onChange={(e) => setSize(Number(e.target.value))}
                    className="w-20 accent-teal"
                  />
                </label>
              )}

              <button
                onClick={() => setPixel((v) => !v)}
                title="Pixel-crisp vs smooth brush"
                className={`rounded-[8px] border px-2.5 py-1.5 text-xs transition ${
                  pixel ? 'border-teal/40 bg-teal/10 text-teal-bright' : 'border-white/10 text-text-dim hover:text-text'
                }`}
              >
                {pixel ? 'Pixel' : 'Smooth'}
              </button>

              <button
                onClick={undo}
                disabled={!canUndo}
                className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-2.5 py-1.5 text-xs text-text-dim transition hover:text-text disabled:opacity-40"
              >
                <ArrowUUpLeftIcon size={13} />
                Undo
              </button>

              <button
                onClick={save}
                disabled={busy || !dirty}
                className="ml-auto inline-flex shrink-0 items-center gap-1.5 rounded-[8px] bg-teal px-3.5 py-2 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
              >
                {busy ? <SpinnerGapIcon size={14} className="animate-spin" /> : <CheckIcon size={14} weight="bold" />}
                Save version
              </button>
            </div>
          </div>
        </div>
      </div>
    </>
  )
}
