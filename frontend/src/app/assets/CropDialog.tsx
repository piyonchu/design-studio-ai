import { useRef, useState } from 'react'
import { CropIcon, XIcon, SpinnerGapIcon } from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'

type Rect = { x: number; y: number; w: number; h: number }

/**
 * Crop editor (Pro pipeline B1) — drag a rectangle over the image, apply, and
 * the backend crops the bytes into a new version (A2, non-destructive). Coords
 * are tracked in display pixels and scaled to the image's native resolution on
 * apply, so the crop is exact regardless of preview size. No canvas / pixel
 * access needed — the server does the actual crop.
 */
export function CropDialog({
  asset,
  onClose,
  onSaved,
}: {
  asset: api.Asset
  onClose: () => void
  onSaved: (a: api.Asset) => void
}) {
  const imgRef = useRef<HTMLImageElement>(null)
  const dragging = useRef(false)
  const start = useRef({ x: 0, y: 0 })
  const [sel, setSel] = useState<Rect | null>(null)
  const [nat, setNat] = useState<{ w: number; h: number } | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Pointer position clamped to the image box, in display pixels.
  function toDisplay(e: React.PointerEvent) {
    const r = imgRef.current!.getBoundingClientRect()
    return {
      x: Math.min(Math.max(e.clientX - r.left, 0), r.width),
      y: Math.min(Math.max(e.clientY - r.top, 0), r.height),
    }
  }

  function onDown(e: React.PointerEvent) {
    if (!imgRef.current) return
    e.preventDefault()
    dragging.current = true
    start.current = toDisplay(e)
    setSel({ ...start.current, w: 0, h: 0 })
  }

  function onMove(e: React.PointerEvent) {
    if (!dragging.current) return
    const cur = toDisplay(e)
    setSel({
      x: Math.min(start.current.x, cur.x),
      y: Math.min(start.current.y, cur.y),
      w: Math.abs(cur.x - start.current.x),
      h: Math.abs(cur.y - start.current.y),
    })
  }

  // The selection scaled from display pixels to the image's native pixels.
  function nativeRect(): Rect | null {
    if (!sel || !nat || !imgRef.current) return null
    const r = imgRef.current.getBoundingClientRect()
    const sx = nat.w / r.width
    const sy = nat.h / r.height
    const x = Math.round(sel.x * sx)
    const y = Math.round(sel.y * sy)
    return {
      x,
      y,
      w: Math.min(Math.round(sel.w * sx), nat.w - x),
      h: Math.min(Math.round(sel.h * sy), nat.h - y),
    }
  }

  async function apply() {
    const rect = nativeRect()
    if (!rect || rect.w < 1 || rect.h < 1) {
      setError('Drag a region on the image first.')
      return
    }
    setBusy(true)
    setError(null)
    try {
      const updated = await api.editAsset(asset.id, { op: 'crop', ...rect })
      onSaved(updated)
      onClose()
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Crop failed.')
    } finally {
      setBusy(false)
    }
  }

  const out = nativeRect()

  return (
    <>
      <div className="fixed inset-0 z-[60] bg-black/60" onClick={onClose} aria-hidden />
      <div className="fixed inset-0 z-[70] grid place-items-center p-4" role="dialog" aria-modal>
        <div className="glass flex max-h-[92dvh] w-full max-w-3xl flex-col overflow-hidden rounded-[16px]">
          <header className="flex items-center gap-2 border-b border-white/8 px-4 py-3">
            <span className="grid size-7 place-items-center rounded-[8px] bg-accent/15 text-teal-bright">
              <CropIcon size={15} weight="fill" />
            </span>
            <p className="text-sm font-medium text-text">Crop</p>
            <span className="text-xs text-text-dim">· drag a region, saved as a new version</span>
            <button
              onClick={onClose}
              aria-label="Close"
              className="ml-auto grid size-7 place-items-center rounded-[8px] text-text-dim transition hover:bg-white/5 hover:text-text"
            >
              <XIcon size={16} />
            </button>
          </header>

          <div className="min-h-0 flex-1 overflow-auto p-4 text-center">
            <div
              className="relative inline-block touch-none select-none"
              onPointerDown={onDown}
              onPointerMove={onMove}
              onPointerUp={() => (dragging.current = false)}
              onPointerLeave={() => (dragging.current = false)}
            >
              {/* An <img> with only CSS max-constraints collapses to ~0 in this
                  flex/fit-content shell (a bare img has no intrinsic layout box
                  here). Give it an explicit px width from the natural size once
                  loaded, capped so large assets still fit; getBoundingClientRect
                  drives the coord mapping, so any rendered size is exact. */}
              <img
                ref={imgRef}
                src={asset.url}
                alt=""
                draggable={false}
                onLoad={(e) =>
                  setNat({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight })
                }
                style={nat ? { width: Math.min(nat.w, 560) } : undefined}
                className="block cursor-crosshair rounded-[10px] ring-1 ring-white/10"
              />
                {sel && sel.w > 0 && sel.h > 0 && (
                  <div
                    className="pointer-events-none absolute border-2 border-teal bg-teal/15"
                    style={{ left: sel.x, top: sel.y, width: sel.w, height: sel.h }}
                  />
                )}
              </div>
            </div>

          <div className="flex items-center gap-2 border-t border-white/8 p-3">
            {error ? (
              <p className="text-xs text-rose-300">{error}</p>
            ) : (
              <p className="text-xs text-text-dim">
                {out && out.w > 0
                  ? `Selection: ${out.w}×${out.h}px`
                  : 'Drag on the image to select a region.'}
              </p>
            )}
            <div className="ml-auto flex items-center gap-2">
              <button
                onClick={() => {
                  setSel(null)
                  setError(null)
                }}
                disabled={busy || !sel}
                className="rounded-[8px] border border-white/10 px-3 py-1.5 text-xs text-text-dim transition hover:text-text disabled:opacity-40"
              >
                Reset
              </button>
              <button
                onClick={apply}
                disabled={busy || !out || out.w < 1}
                className="inline-flex items-center gap-1.5 rounded-[8px] bg-teal px-3 py-1.5 text-xs font-semibold text-bg transition active:translate-y-px disabled:opacity-40"
              >
                {busy ? <SpinnerGapIcon size={13} className="animate-spin" /> : <CropIcon size={13} weight="fill" />}
                Apply crop
              </button>
            </div>
          </div>
        </div>
      </div>
    </>
  )
}
