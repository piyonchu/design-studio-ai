import { useEffect, useState, type FormEvent } from 'react'
import {
  SparkleIcon,
  SpinnerGapIcon,
  ImageIcon,
  UploadSimpleIcon,
  CheckIcon,
  XIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'

// Spike-proven generative derivations (recolor stays out — it drifts identity
// generatively; it belongs on the deterministic path).
const PRESETS = [
  { id: 'walk', label: 'Walk', text: 'Show the SAME character in a mid-walk side stride pose. Keep identical identity, palette, and proportions.' },
  { id: 'action', label: 'Action', text: 'Show the SAME character in a dynamic action pose. Keep identical identity, palette, and proportions.' },
  { id: 'variant', label: 'Variant', text: 'An outfit/expression variant of the SAME character. Keep identical shape and proportions.' },
  { id: 'matching', label: 'Matching', text: 'A matching set member in the EXACT same art style, palette, and outline weight. Different subject, same world.' },
]

/**
 * Project asset library — generate, upload, derive (reference-conditioned), and
 * review (approve/reject) assets. Click a tile to pick it as the derivation base.
 */
export function AssetLibrary({ projectId }: { projectId: string }) {
  const [assets, setAssets] = useState<api.Asset[]>([])
  const [prompt, setPrompt] = useState('')
  const [baseId, setBaseId] = useState<string | null>(null)
  const [instruction, setInstruction] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.listAssets(projectId).then(setAssets).catch(() => {})
  }, [projectId])

  function genError(err: unknown) {
    setError(
      err instanceof ApiError && err.status === 503
        ? 'Image generation unavailable. (Set OPENROUTER_API_KEY, or ASSET_MOCK=true.)'
        : err instanceof ApiError
          ? err.message
          : 'Request failed.',
    )
  }

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
      genError(err)
    } finally {
      setBusy(false)
    }
  }

  async function upload(file: File) {
    setBusy(true)
    setError(null)
    try {
      const created = await api.uploadAsset(projectId, file, 'base')
      setAssets((a) => [created, ...a])
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Upload failed.')
    } finally {
      setBusy(false)
    }
  }

  function pickFile() {
    const inp = document.createElement('input')
    inp.type = 'file'
    inp.accept = 'image/*'
    inp.onchange = () => {
      const f = inp.files?.[0]
      if (f) upload(f)
    }
    inp.click()
  }

  async function derive() {
    const ins = instruction.trim()
    if (!ins || !baseId || busy) return
    setBusy(true)
    setError(null)
    try {
      const created = await api.deriveAssets(projectId, baseId, ins, 2)
      setAssets((a) => [...created, ...a])
    } catch (err) {
      genError(err)
    } finally {
      setBusy(false)
    }
  }

  async function review(id: string, status: api.AssetStatus) {
    try {
      const updated = await api.setAssetStatus(id, status)
      setAssets((a) => a.map((x) => (x.id === id ? updated : x)))
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Update failed.')
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
        <button
          onClick={pickFile}
          disabled={busy}
          className="ml-auto inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-3 py-1.5 text-sm text-text-dim transition hover:text-text disabled:opacity-50"
        >
          <UploadSimpleIcon size={14} />
          Upload base
        </button>
      </div>

      {baseId ? (
        <div className="border-b border-white/8 p-4">
          <div className="mx-auto max-w-2xl">
            <div className="mb-2 flex items-center gap-2 text-xs text-text-dim">
              <span>Deriving from selected base — pick a preset or write an instruction</span>
              <button onClick={() => setBaseId(null)} className="ml-auto text-text-dim hover:text-text">
                Clear
              </button>
            </div>
            <div className="mb-2 flex flex-wrap gap-1.5">
              {PRESETS.map((p) => (
                <button
                  key={p.id}
                  onClick={() => setInstruction(p.text)}
                  className="rounded-[8px] border border-white/10 px-2.5 py-1 text-xs text-text-dim transition hover:text-text"
                >
                  {p.label}
                </button>
              ))}
            </div>
            <div className="flex items-center gap-2 rounded-[12px] bg-surface-2/60 p-2">
              <input
                value={instruction}
                onChange={(e) => setInstruction(e.target.value)}
                placeholder="Derivation instruction…"
                className="flex-1 bg-transparent px-2 text-sm text-text outline-none placeholder:text-text-dim"
              />
              <button
                onClick={derive}
                disabled={busy || !instruction.trim()}
                className="inline-flex shrink-0 items-center gap-1.5 rounded-[8px] bg-teal px-3.5 py-2 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
              >
                {busy ? <SpinnerGapIcon size={14} className="animate-spin" /> : <SparkleIcon size={14} weight="fill" />}
                Derive
              </button>
            </div>
          </div>
        </div>
      ) : (
        <form onSubmit={generate} className="border-b border-white/8 p-4">
          <div className="mx-auto flex max-w-2xl items-center gap-2 rounded-[12px] bg-surface-2/60 p-2">
            <input
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              placeholder="Describe an asset to generate… (or click a tile to derive)"
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
        </form>
      )}

      {error && <p className="px-5 pt-3 text-xs text-rose-300">{error}</p>}

      <div className="min-h-0 flex-1 overflow-y-auto p-5">
        {assets.length === 0 ? (
          <p className="px-1 py-16 text-center text-sm text-text-dim">
            No assets yet. Generate one above, or upload a base.
          </p>
        ) : (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
            {assets.map((a) => (
              <figure
                key={a.id}
                onClick={() => setBaseId(a.id === baseId ? null : a.id)}
                className={`group relative cursor-pointer overflow-hidden rounded-[12px] ring-1 transition ${
                  a.id === baseId ? 'ring-2 ring-teal' : 'ring-white/10 hover:ring-white/25'
                }`}
                title={a.derivation ?? a.prompt ?? a.role ?? ''}
              >
                <img src={a.url} alt={a.prompt ?? a.role ?? ''} className="aspect-square w-full object-cover" />
                <span className="absolute left-1.5 top-1.5 rounded-[6px] bg-black/55 px-1.5 py-0.5 text-[10px] font-medium text-white/90 backdrop-blur">
                  {a.source_kind}
                </span>
                {a.status !== 'candidate' && (
                  <span className="absolute right-1.5 top-1.5 rounded-[6px] bg-teal/80 px-1.5 py-0.5 text-[10px] font-medium text-bg">
                    {a.status}
                  </span>
                )}
                {a.status === 'candidate' && (
                  <div className="absolute inset-x-0 bottom-0 flex gap-1 bg-black/45 p-1.5 opacity-0 backdrop-blur transition group-hover:opacity-100">
                    <button
                      onClick={(e) => {
                        e.stopPropagation()
                        review(a.id, 'approved')
                      }}
                      aria-label="Approve"
                      className="flex flex-1 items-center justify-center rounded-[6px] bg-teal/85 py-1 text-bg transition hover:bg-teal"
                    >
                      <CheckIcon size={13} weight="bold" />
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation()
                        review(a.id, 'rejected')
                      }}
                      aria-label="Reject"
                      className="flex flex-1 items-center justify-center rounded-[6px] bg-white/10 py-1 text-rose-200 transition hover:bg-white/20"
                    >
                      <XIcon size={13} weight="bold" />
                    </button>
                  </div>
                )}
                {(a.derivation ?? a.prompt ?? a.role) && (
                  <figcaption className="truncate px-2 py-1.5 text-[11px] text-text-dim">
                    {a.derivation ?? a.prompt ?? a.role}
                  </figcaption>
                )}
              </figure>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
