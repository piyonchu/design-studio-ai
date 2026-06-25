import { useEffect, useState, type FormEvent } from 'react'
import { SparkleIcon, SpinnerGapIcon, ImageIcon } from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'

/**
 * Project asset library — generate assets and browse the project's collection.
 * The foundation of the CanonForge asset board (filters, status, derivation,
 * and consistency review land on top of this — see PLAN.md).
 */
export function AssetLibrary({ projectId }: { projectId: string }) {
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
    <div className="glass flex min-h-0 flex-1 flex-col rounded-[16px]">
      <div className="flex items-center gap-2 border-b border-white/8 px-5 py-4">
        <span className="grid size-7 place-items-center rounded-[8px] bg-accent/15 text-teal-bright">
          <ImageIcon size={15} weight="fill" />
        </span>
        <p className="text-sm font-medium text-text">Assets</p>
        <span className="text-sm text-text-dim">· {assets.length}</span>
      </div>

      <form onSubmit={generate} className="border-b border-white/8 p-4">
        <div className="mx-auto flex max-w-2xl items-center gap-2 rounded-[12px] bg-surface-2/60 p-2">
          <input
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder="Describe an asset to generate…"
            className="flex-1 bg-transparent px-2 text-sm text-text outline-none placeholder:text-text-dim"
          />
          <button
            type="submit"
            disabled={busy || !prompt.trim()}
            className="inline-flex shrink-0 items-center gap-1.5 rounded-[8px] bg-teal px-3.5 py-2 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
          >
            {busy ? <SpinnerGapIcon size={14} className="animate-spin" /> : <SparkleIcon size={14} weight="fill" />}
            Generate
          </button>
        </div>
        {error && <p className="mx-auto mt-2 max-w-2xl text-xs text-rose-300">{error}</p>}
      </form>

      <div className="min-h-0 flex-1 overflow-y-auto p-5">
        {assets.length === 0 ? (
          <p className="px-1 py-16 text-center text-sm text-text-dim">
            No assets yet. Describe one above to generate it.
          </p>
        ) : (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
            {assets.map((a) => (
              <figure
                key={a.id}
                className="group overflow-hidden rounded-[12px] ring-1 ring-white/10"
                title={a.prompt ?? ''}
              >
                <img src={a.url} alt={a.prompt ?? ''} className="aspect-square w-full object-cover" />
                {a.prompt && (
                  <figcaption className="truncate px-2 py-1.5 text-[11px] text-text-dim">{a.prompt}</figcaption>
                )}
              </figure>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
