import { useEffect, useState, type FormEvent } from 'react'
import { XIcon, SparkleIcon, SpinnerGapIcon, ImageIcon } from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'

export function AssetPanel({
  projectId,
  canAttach,
  onAttach,
  onClose,
}: {
  projectId: string
  canAttach: boolean
  onAttach: (asset: api.Asset) => void
  onClose: () => void
}) {
  const [assets, setAssets] = useState<api.Asset[]>([])
  const [prompt, setPrompt] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.listAssets(projectId).then(setAssets).catch(() => {})
  }, [projectId])

  async function generate(e: FormEvent) {
    e.preventDefault()
    const p = prompt.trim()
    if (!p || busy) return
    setBusy(true)
    setError(null)
    try {
      const created = await api.generateAssets(projectId, p, 2)
      setAssets((a) => [...created, ...a])
      setPrompt('')
    } catch (err) {
      setError(
        err instanceof ApiError && err.status === 503
          ? 'Image generation unavailable. (Set OPENROUTER_API_KEY, or ASSET_MOCK=true.)'
          : err instanceof ApiError
            ? err.message
            : 'Generation failed.',
      )
    } finally {
      setBusy(false)
    }
  }

  return (
    <aside className="glass flex w-[340px] shrink-0 flex-col rounded-[16px]">
      <div className="flex items-center justify-between border-b border-white/8 px-4 py-3">
        <div className="flex items-center gap-2">
          <span className="grid size-7 place-items-center rounded-[8px] bg-accent/15 text-teal-bright">
            <ImageIcon size={15} weight="fill" />
          </span>
          <p className="text-sm font-medium text-text">Assets — Library</p>
        </div>
        <button
          onClick={onClose}
          aria-label="Close assets"
          className="grid size-7 place-items-center rounded-[8px] text-text-dim transition hover:bg-white/5 hover:text-text"
        >
          <XIcon size={15} />
        </button>
      </div>

      <form onSubmit={generate} className="border-b border-white/8 p-3">
        <div className="flex items-center gap-2 rounded-[12px] bg-surface-2/60 p-2">
          <input
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder="Generate an illustration…"
            className="flex-1 bg-transparent px-1 text-sm text-text outline-none placeholder:text-text-dim"
          />
          <button
            type="submit"
            disabled={busy || !prompt.trim()}
            className="inline-flex shrink-0 items-center gap-1.5 rounded-[8px] bg-teal px-3 py-1.5 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
          >
            {busy ? <SpinnerGapIcon size={14} className="animate-spin" /> : <SparkleIcon size={14} weight="fill" />}
            Generate
          </button>
        </div>
        {error && <p className="mt-2 text-xs text-rose-300">{error}</p>}
      </form>

      <div className="flex-1 overflow-y-auto p-3">
        {assets.length === 0 ? (
          <p className="px-1 py-6 text-center text-sm text-text-dim">
            No assets yet. Describe an image above to generate one.
          </p>
        ) : (
          <div className="grid grid-cols-2 gap-2.5">
            {assets.map((a) => (
              <div key={a.id} className="group relative overflow-hidden rounded-[12px] ring-1 ring-white/10">
                <img src={a.s3_key} alt={a.prompt ?? ''} className="aspect-square w-full object-cover" />
                <button
                  onClick={() => onAttach(a)}
                  disabled={!canAttach}
                  title={canAttach ? 'Use in the current screen' : 'Open a UI screen with an image to attach'}
                  className="absolute inset-x-1.5 bottom-1.5 rounded-[8px] bg-black/70 py-1 text-[11px] font-medium text-white opacity-0 backdrop-blur transition group-hover:opacity-100 disabled:cursor-not-allowed"
                >
                  Use in screen
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </aside>
  )
}
