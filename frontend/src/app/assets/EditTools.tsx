import { useState } from 'react'
import {
  PencilRulerIcon,
  FlipHorizontalIcon,
  FlipVerticalIcon,
  ArrowClockwiseIcon,
  CircleHalfIcon,
  SwatchesIcon,
  ScissorsIcon,
  PaintBrushIcon,
  SpinnerGapIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'
import { InpaintDialog } from './InpaintDialog'
import { PaintDialog } from './PaintDialog'

/**
 * Deterministic edits (Pro pipeline B1) — free, instant, model-free transforms.
 * Each one applies to the head bytes and lands as a NEW version (history keeps
 * the original), so "change one little thing" never re-rolls the whole image.
 */
export function EditTools({
  asset,
  onChanged,
}: {
  asset: api.Asset
  onChanged: (a: api.Asset) => void
}) {
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [w, setW] = useState('')
  const [h, setH] = useState('')
  const [inpainting, setInpainting] = useState(false)
  const [painting, setPainting] = useState(false)

  async function run(label: string, op: api.EditOp) {
    if (busy) return
    setBusy(label)
    setError(null)
    try {
      const updated = await api.editAsset(asset.id, op)
      onChanged(updated)
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Edit failed.')
    } finally {
      setBusy(null)
    }
  }

  const Tool = ({
    id,
    icon: Icon,
    label,
    op,
  }: {
    id: string
    icon: typeof FlipHorizontalIcon
    label: string
    op: api.EditOp
  }) => (
    <button
      onClick={() => run(id, op)}
      disabled={!!busy}
      className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-2.5 py-1.5 text-xs text-text-dim transition hover:bg-white/5 hover:text-text disabled:opacity-40"
    >
      {busy === id ? <SpinnerGapIcon size={13} className="animate-spin" /> : <Icon size={13} />}
      {label}
    </button>
  )

  return (
    <div>
      <div className="flex items-center gap-1.5 text-xs text-text-dim">
        <PencilRulerIcon size={14} />
        Edit
        <span className="text-[10px]">· free · saves a new version</span>
        <button
          onClick={() => setPainting(true)}
          className="ml-auto inline-flex items-center gap-1 rounded-[7px] border border-teal/30 bg-teal/10 px-2 py-1 text-[11px] text-teal-bright transition hover:bg-teal/15"
        >
          <PaintBrushIcon size={12} weight="fill" />
          Paint
        </button>
        <button
          onClick={() => setInpainting(true)}
          className="inline-flex items-center gap-1 rounded-[7px] border border-teal/30 bg-teal/10 px-2 py-1 text-[11px] text-teal-bright transition hover:bg-teal/15"
        >
          <PaintBrushIcon size={12} weight="fill" />
          Edit a region
        </button>
      </div>

      {painting && (
        <PaintDialog
          asset={asset}
          onClose={() => setPainting(false)}
          onSaved={(updated) => onChanged(updated)}
        />
      )}
      {inpainting && (
        <InpaintDialog
          asset={asset}
          onClose={() => setInpainting(false)}
          onSaved={(updated) => onChanged(updated)}
        />
      )}

      <div className="mt-2 flex flex-wrap gap-1.5">
        <Tool id="flh" icon={FlipHorizontalIcon} label="Flip H" op={{ op: 'flip', axis: 'horizontal' }} />
        <Tool id="flv" icon={FlipVerticalIcon} label="Flip V" op={{ op: 'flip', axis: 'vertical' }} />
        <Tool id="rot" icon={ArrowClockwiseIcon} label="Rotate 90°" op={{ op: 'rotate', degrees: 90 }} />
        <Tool id="gray" icon={CircleHalfIcon} label="Grayscale" op={{ op: 'grayscale' }} />
        <Tool id="inv" icon={SwatchesIcon} label="Invert" op={{ op: 'invert' }} />
        <Tool id="hue" icon={SwatchesIcon} label="Hue +30°" op={{ op: 'hue', degrees: 30 }} />
        <Tool id="bg" icon={ScissorsIcon} label="Remove BG" op={{ op: 'remove_bg' }} />
      </div>

      {/* Resize + convert */}
      <div className="mt-2 flex flex-wrap items-center gap-1.5">
        <input
          value={w}
          onChange={(e) => setW(e.target.value.replace(/\D/g, ''))}
          placeholder="W"
          inputMode="numeric"
          className="w-14 rounded-[8px] bg-surface/60 px-2 py-1.5 text-xs text-text outline-none placeholder:text-text-dim focus:ring-1 focus:ring-teal/40"
        />
        <span className="text-text-dim">×</span>
        <input
          value={h}
          onChange={(e) => setH(e.target.value.replace(/\D/g, ''))}
          placeholder="H"
          inputMode="numeric"
          className="w-14 rounded-[8px] bg-surface/60 px-2 py-1.5 text-xs text-text outline-none placeholder:text-text-dim focus:ring-1 focus:ring-teal/40"
        />
        <button
          onClick={() => {
            const nw = Number(w)
            const nh = Number(h)
            if (nw > 0 && nh > 0) run('resize', { op: 'resize', w: nw, h: nh })
          }}
          disabled={!!busy || !w || !h}
          className="rounded-[8px] border border-white/10 px-2.5 py-1.5 text-xs text-text-dim transition hover:text-text disabled:opacity-40"
        >
          {busy === 'resize' ? <SpinnerGapIcon size={13} className="animate-spin" /> : 'Resize'}
        </button>
        <span className="mx-1 h-4 w-px bg-white/10" />
        <button
          onClick={() => run('png', { op: 'convert', format: 'png' })}
          disabled={!!busy}
          className="rounded-[8px] border border-white/10 px-2.5 py-1.5 text-xs text-text-dim transition hover:text-text disabled:opacity-40"
        >
          → PNG
        </button>
        <button
          onClick={() => run('jpeg', { op: 'convert', format: 'jpeg' })}
          disabled={!!busy}
          className="rounded-[8px] border border-white/10 px-2.5 py-1.5 text-xs text-text-dim transition hover:text-text disabled:opacity-40"
        >
          → JPEG
        </button>
      </div>

      {error && <p className="mt-2 text-xs text-rose-300">{error}</p>}
    </div>
  )
}
