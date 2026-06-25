import { useCallback, useEffect, useRef, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { ReactFlowProvider, type Node, type Edge } from '@xyflow/react'
import {
  ArrowLeftIcon,
  FloppyDiskIcon,
  CheckIcon,
  SpinnerGapIcon,
  PlusIcon,
  CaretDownIcon,
  FlowArrowIcon,
  BrowsersIcon,
  PaletteIcon,
  AppWindowIcon,
} from '@phosphor-icons/react'
import * as api from '../lib/api'
import { ApiError } from '../lib/api'
import { ChatPanel, type ChatMessage } from './ai/ChatPanel'
import { FlowCanvas } from './flow/FlowCanvas'
import { flowToDsl, type FlowDsl, type FlowNodeData } from './flow/layout'
import { WireframeCanvas } from './wireframe/WireframeCanvas'
import type { Element } from './wireframe/renderElement'
import { DesignSystemView } from './design/DesignSystemView'
import { resolveTokens, DEFAULT_TOKENS, type DesignTokens } from './design/tokens'

const KIND_META: Record<string, { label: string; icon: typeof FlowArrowIcon }> = {
  user_flow: { label: 'User Flow', icon: FlowArrowIcon },
  wireframe: { label: 'Wireframe', icon: BrowsersIcon },
  ui_screen: { label: 'UI Screen', icon: AppWindowIcon },
  design_system: { label: 'Design System', icon: PaletteIcon },
}

const NEW_KINDS: { kind: api.ArtifactKind; label: string }[] = [
  { kind: 'user_flow', label: 'User Flow' },
  { kind: 'wireframe', label: 'Wireframe' },
  { kind: 'ui_screen', label: 'UI Screen' },
  { kind: 'design_system', label: 'Design System' },
]

export function ProjectWorkspace() {
  const { projectId } = useParams<{ projectId: string }>()
  const navigate = useNavigate()

  const [project, setProject] = useState<api.Project | null>(null)
  const [artifacts, setArtifacts] = useState<api.Artifact[]>([])
  const [activeId, setActiveId] = useState<string | null>(null)
  const [pendingKind, setPendingKind] = useState<api.ArtifactKind | null>(null)
  const [content, setContent] = useState<unknown>(null)
  const [canvasKey, setCanvasKey] = useState(0)
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [busy, setBusy] = useState(false)
  const [saving, setSaving] = useState<'idle' | 'saving' | 'saved'>('idle')
  const [error, setError] = useState<string | null>(null)
  const [newOpen, setNewOpen] = useState(false)
  const [tokens, setTokens] = useState<DesignTokens>(DEFAULT_TOKENS) // project design system
  const [hasDS, setHasDS] = useState(false)

  const active = artifacts.find((a) => a.id === activeId) ?? null

  const graphRef = useRef<{ nodes: Node<FlowNodeData>[]; edges: Edge[] }>({ nodes: [], edges: [] })
  const onGraphChange = useCallback((nodes: Node<FlowNodeData>[], edges: Edge[]) => {
    graphRef.current = { nodes, edges }
  }, [])

  const loadContent = useCallback(async (id: string) => {
    const full = await api.getArtifact(id)
    setContent(full.head_version?.content ?? null)
    setCanvasKey((k) => k + 1)
  }, [])

  // Initial load: project + artifacts; select the first, else default to a new flow.
  useEffect(() => {
    if (!projectId) return
    let alive = true
    ;(async () => {
      try {
        const [proj, list] = await Promise.all([
          api.getProject(projectId),
          api.listArtifacts(projectId),
        ])
        if (!alive) return
        setProject(proj)
        setArtifacts(list)
        // Resolve the project's design system (themes hi-fi screens).
        const ds = list.find((a) => a.kind === 'design_system')
        if (ds) {
          const full = await api.getArtifact(ds.id)
          if (!alive) return
          setTokens(resolveTokens(full.head_version?.content))
          setHasDS(true)
        }
        if (list[0]) {
          setActiveId(list[0].id)
          await loadContent(list[0].id)
        } else {
          setPendingKind('user_flow')
        }
      } catch (err) {
        if (alive) setError(err instanceof ApiError ? err.message : 'Failed to load project.')
      }
    })()
    return () => {
      alive = false
    }
  }, [projectId, loadContent])

  function selectArtifact(id: string) {
    setActiveId(id)
    setPendingKind(null)
    setMessages([])
    setError(null)
    setContent(null) // avoid rendering the previous artifact's content into the new canvas
    loadContent(id).catch((err) =>
      setError(err instanceof ApiError ? err.message : 'Failed to load artifact.'),
    )
  }

  function startNew(kind: api.ArtifactKind) {
    setNewOpen(false)
    setActiveId(null)
    setPendingKind(kind)
    setContent(null)
    setMessages([])
    setError(null)
    setCanvasKey((k) => k + 1)
  }

  async function onSend(prompt: string) {
    if (!projectId) return
    setBusy(true)
    setError(null)
    setMessages((m) => [...m, { role: 'user', text: prompt }])
    try {
      if (active) {
        const version = await api.aiEdit(active.id, prompt)
        setContent(version.content ?? null)
        if (active.kind === 'design_system') {
          setTokens(resolveTokens(version.content))
          setHasDS(true)
        }
        setMessages((m) => [...m, { role: 'ai', text: 'Updated the design.' }])
      } else {
        const kind = pendingKind ?? 'user_flow'
        const created = await api.generate(projectId, { kind, prompt })
        setArtifacts((a) => [created, ...a])
        setActiveId(created.id)
        setPendingKind(null)
        setContent(created.head_version?.content ?? null)
        if (kind === 'design_system') {
          setTokens(resolveTokens(created.head_version?.content))
          setHasDS(true)
        }
        setMessages((m) => [...m, { role: 'ai', text: `Created the ${KIND_META[kind]?.label ?? kind}.` }])
      }
      setCanvasKey((k) => k + 1)
    } catch (err) {
      const msg =
        err instanceof ApiError && err.status === 503
          ? 'AI is unavailable right now. (Set ANTHROPIC_API_KEY, or AI_MOCK=true for local dev.)'
          : err instanceof ApiError
            ? err.message
            : 'Something went wrong.'
      setMessages((m) => [...m, { role: 'ai', text: msg }])
    } finally {
      setBusy(false)
    }
  }

  async function onSave() {
    if (!active || active.kind !== 'user_flow') return
    setSaving('saving')
    try {
      await api.addVersion(active.id, {
        content: flowToDsl(graphRef.current.nodes, graphRef.current.edges),
        change_source: 'manual',
        change_summary: 'Manual canvas edit',
      })
      setSaving('saved')
      setTimeout(() => setSaving('idle'), 1500)
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Failed to save.')
      setSaving('idle')
    }
  }

  const isFlow = active?.kind === 'user_flow'
  const isWireframe = active?.kind === 'wireframe' || active?.kind === 'ui_screen'
  const isDesignSystem = active?.kind === 'design_system'
  const wireRoot = (content as { root?: Element } | null)?.root

  return (
    <div className="relative flex h-[100dvh] flex-col">
      <div className="app-aurora" />

      {/* Top bar */}
      <header className="relative z-20 flex items-center gap-3 px-4 py-3">
        <button
          onClick={() => navigate('/')}
          aria-label="Back to workspace"
          className="grid size-9 place-items-center rounded-[10px] text-text-dim transition hover:bg-white/5 hover:text-text"
        >
          <ArrowLeftIcon size={18} />
        </button>
        <p className="shrink-0 text-sm font-semibold text-text">{project?.name ?? 'Project'}</p>

        {/* Artifact tabs */}
        <div className="ml-2 flex min-w-0 items-center gap-1 overflow-x-auto">
          {artifacts.map((a) => {
            const meta = KIND_META[a.kind] ?? { label: a.kind, icon: BrowsersIcon }
            const Icon = meta.icon
            const on = a.id === activeId
            return (
              <button
                key={a.id}
                onClick={() => selectArtifact(a.id)}
                className={`inline-flex shrink-0 items-center gap-1.5 rounded-[10px] px-3 py-1.5 text-sm transition ${
                  on ? 'bg-white/10 text-text' : 'text-text-dim hover:bg-white/5 hover:text-text'
                }`}
              >
                <Icon size={15} weight={on ? 'fill' : 'regular'} />
                {meta.label}
              </button>
            )
          })}

          {/* New menu */}
          <div className="relative shrink-0">
            <button
              onClick={() => setNewOpen((v) => !v)}
              className="inline-flex items-center gap-1 rounded-[10px] px-2.5 py-1.5 text-sm text-text-dim transition hover:bg-white/5 hover:text-text"
            >
              <PlusIcon size={15} /> New <CaretDownIcon size={12} />
            </button>
            {newOpen && (
              <div className="glass absolute left-0 top-10 z-30 w-44 rounded-[12px] p-1.5">
                {NEW_KINDS.map(({ kind, label }) => {
                  const Icon = KIND_META[kind].icon
                  return (
                    <button
                      key={kind}
                      onClick={() => startNew(kind)}
                      className="flex w-full items-center gap-2 rounded-[8px] px-3 py-2 text-left text-sm text-text transition hover:bg-white/5"
                    >
                      <Icon size={15} /> {label}
                    </button>
                  )
                })}
              </div>
            )}
          </div>
        </div>

        {isFlow && (
          <button
            onClick={onSave}
            disabled={saving === 'saving'}
            className="ml-auto inline-flex shrink-0 items-center gap-2 rounded-[10px] bg-white/5 px-3.5 py-2 text-sm font-medium text-text transition hover:bg-white/10 disabled:opacity-50"
          >
            {saving === 'saving' ? (
              <SpinnerGapIcon size={15} className="animate-spin" />
            ) : saving === 'saved' ? (
              <CheckIcon size={15} className="text-teal-bright" />
            ) : (
              <FloppyDiskIcon size={15} />
            )}
            {saving === 'saved' ? 'Saved' : 'Save'}
          </button>
        )}
      </header>

      {error && (
        <p className="relative z-10 mx-4 mb-2 rounded-[10px] border border-rose-500/30 bg-rose-500/10 px-3 py-2 text-sm text-rose-300">
          {error}
        </p>
      )}

      {/* Workspace: chat + canvas */}
      <div className="relative z-10 flex min-h-0 flex-1 gap-3 px-3 pb-3">
        <ChatPanel messages={messages} busy={busy} onSend={onSend} />
        <div className="glass relative min-w-0 flex-1 overflow-hidden rounded-[16px]">
          {isFlow && content ? (
            <ReactFlowProvider>
              <FlowCanvas key={canvasKey} dsl={content as FlowDsl} onGraphChange={onGraphChange} />
            </ReactFlowProvider>
          ) : isDesignSystem && content ? (
            <DesignSystemView key={canvasKey} content={content} />
          ) : isWireframe && wireRoot ? (
            <WireframeCanvas
              key={canvasKey}
              root={wireRoot}
              tokens={tokens}
              hasDesignSystem={hasDS}
            />
          ) : (
            <div className="grid h-full place-items-center px-6 text-center">
              <p className="max-w-sm text-sm text-text-dim">
                {pendingKind
                  ? `Ask the AI Designer to create your ${KIND_META[pendingKind]?.label ?? pendingKind}.`
                  : 'Pick an artifact or create a new one to start designing.'}
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
