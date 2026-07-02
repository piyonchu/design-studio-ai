import { useState, type ReactNode } from 'react'
import {
  FlipHorizontalIcon,
  FlipVerticalIcon,
  ArrowClockwiseIcon,
  CircleHalfIcon,
  SwatchesIcon,
  SunIcon,
  ScissorsIcon,
  CropIcon,
  PaintBrushIcon,
  SpinnerGapIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'
import { InpaintDialog } from './InpaintDialog'
import { PaintDialog } from './PaintDialog'
import { CropDialog } from './CropDialog'

/**
 * Deterministic edits (Pro pipeline B1) — free, instant, model-free transforms.
 * Each one applies to the head bytes and lands as a NEW version (history keeps
 * the original), so "change one little thing" never re-rolls the whole image.
 * Grouped by intent: editors (canvas) · transform · color · cutout · resize.
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
  const [hue, setHue] = useState(0)
  const [bright, setBright] = useState(0)
  const [inpainting, setInpainting] = useState(false)
  const [painting, setPainting] = useState(false)
  const [cropping, setCropping] = useState(false)

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

  const Group = ({ label, children }: { label: string; children: ReactNode }) => (
    <div className="mt-2.5">
      <p className="mb-1 text-[10px] uppercase tracking-wide text-text-dim/70">{label}</p>
      <div className="flex flex-wrap items-center gap-1.5">{children}</div>
    </div>
  )

  return (
    <div>
      {/* Editors — open a canvas; the most powerful tools, so they lead. */}
      <div className="flex gap-1.5">
        <button
          onClick={() => setPainting(true)}
          className="inline-flex flex-1 items-center justify-center gap-1.5 rounded-[8px] border border-teal/30 bg-teal/10 px-2 py-2 text-xs text-teal-bright transition hover:bg-teal/15"
        >
          <PaintBrushIcon size={13} weight="fill" />
          Paint
        </button>
        <button
          onClick={() => setInpainting(true)}
          className="inline-flex flex-1 items-center justify-center gap-1.5 rounded-[8px] border border-teal/30 bg-teal/10 px-2 py-2 text-xs text-teal-bright transition hover:bg-teal/15"
        >
          <PaintBrushIcon size={13} weight="fill" />
          Edit a region
        </button>
      </div>

      {painting && (
        <PaintDialog asset={asset} onClose={() => setPainting(false)} onSaved={(u) => onChanged(u)} />
      )}
      {inpainting && (
        <InpaintDialog asset={asset} onClose={() => setInpainting(false)} onSaved={(u) => onChanged(u)} />
      )}
      {cropping && (
        <CropDialog asset={asset} onClose={() => setCropping(false)} onSaved={(u) => onChanged(u)} />
      )}

      <Group label="Transform">
        <Tool id="flh" icon={FlipHorizontalIcon} label="Flip H" op={{ op: 'flip', axis: 'horizontal' }} />
        <Tool id="flv" icon={FlipVerticalIcon} label="Flip V" op={{ op: 'flip', axis: 'vertical' }} />
        <Tool id="rot" icon={ArrowClockwiseIcon} label="Rotate 90°" op={{ op: 'rotate', degrees: 90 }} />
        <button
          onClick={() => setCropping(true)}
          disabled={!!busy}
          className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-2.5 py-1.5 text-xs text-text-dim transition hover:bg-white/5 hover:text-text disabled:opacity-40"
        >
          <CropIcon size={13} />
          Crop
        </button>
      </Group>

      <Group label="Color">
        <Tool id="gray" icon={CircleHalfIcon} label="Grayscale" op={{ op: 'grayscale' }} />
        {/* Hue rotate — a real recolor across -180..180°, applied on click. */}
        <span className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-2.5 py-1.5 text-xs text-text-dim">
          <SwatchesIcon size={13} />
          Hue
          <input
            type="range"
            min={-180}
            max={180}
            value={hue}
            onChange={(e) => setHue(Number(e.target.value))}
            className="w-20 accent-teal"
            aria-label="Hue degrees"
          />
          <span className="w-9 tabular-nums text-right">{hue > 0 ? `+${hue}` : hue}°</span>
          <button
            onClick={() => run('hue', { op: 'hue', degrees: hue })}
            disabled={!!busy || hue === 0}
            className="rounded-[6px] bg-white/10 px-1.5 py-0.5 text-[11px] text-text transition hover:bg-white/15 disabled:opacity-40"
          >
            {busy === 'hue' ? <SpinnerGapIcon size={11} className="animate-spin" /> : 'Apply'}
          </button>
        </span>
        {/* Brightness — brighten (+) / darken (-). */}
        <span className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-2.5 py-1.5 text-xs text-text-dim">
          <SunIcon size={13} />
          Bright
          <input
            type="range"
            min={-100}
            max={100}
            value={bright}
            onChange={(e) => setBright(Number(e.target.value))}
            className="w-20 accent-teal"
            aria-label="Brightness"
          />
          <span className="w-9 tabular-nums text-right">{bright > 0 ? `+${bright}` : bright}</span>
          <button
            onClick={() => run('bright', { op: 'brighten', value: bright })}
            disabled={!!busy || bright === 0}
            className="rounded-[6px] bg-white/10 px-1.5 py-0.5 text-[11px] text-text transition hover:bg-white/15 disabled:opacity-40"
          >
            {busy === 'bright' ? <SpinnerGapIcon size={11} className="animate-spin" /> : 'Apply'}
          </button>
        </span>
      </Group>

      <Group label="Cutout">
        <Tool id="bg" icon={ScissorsIcon} label="Remove BG" op={{ op: 'remove_bg' }} />
      </Group>

      <Group label="Resize · export format">
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
      </Group>

      {error && <p className="mt-2 text-xs text-rose-300">{error}</p>}
    </div>
  )
}
