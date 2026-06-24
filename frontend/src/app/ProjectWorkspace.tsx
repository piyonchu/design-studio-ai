import { useCallback, useEffect, useRef, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { ReactFlowProvider, type Node, type Edge } from '@xyflow/react'
import {
  ArrowLeftIcon,
  FloppyDiskIcon,
  CheckIcon,
  SpinnerGapIcon,
} from '@phosphor-icons/react'
import * as api from '../lib/api'
import { ApiError } from '../lib/api'
import { ChatPanel, type ChatMessage } from './ai/ChatPanel'
import { FlowCanvas } from './flow/FlowCanvas'
import { flowToDsl, type FlowDsl, type FlowNodeData } from './flow/layout'

const EMPTY: FlowDsl = { nodes: [], edges: [] }

export function ProjectWorkspace() {
  const { projectId } = useParams<{ projectId: string }>()
  const navigate = useNavigate()

  const [project, setProject] = useState<api.Project | null>(null)
  const [flowArtifact, setFlowArtifact] = useState<api.Artifact | null>(null)
  const [dsl, setDsl] = useState<FlowDsl>(EMPTY)
  const [canvasKey, setCanvasKey] = useState(0) // remount canvas on new version
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [busy, setBusy] = useState(false)
  const [saving, setSaving] = useState<'idle' | 'saving' | 'saved'>('idle')
  const [error, setError] = useState<string | null>(null)

  const graphRef = useRef<{ nodes: Node<FlowNodeData>[]; edges: Edge[] }>({
    nodes: [],
    edges: [],
  })
  const onGraphChange = useCallback(
    (nodes: Node<FlowNodeData>[], edges: Edge[]) => {
      graphRef.current = { nodes, edges }
    },
    [],
  )

  // Load project + its (first) user_flow artifact.
  useEffect(() => {
    if (!projectId) return
    let active = true
    ;(async () => {
      try {
        const [proj, artifacts] = await Promise.all([
          api.getProject(projectId),
          api.listArtifacts(projectId),
        ])
        if (!active) return
        setProject(proj)
        const flow = artifacts.find((a) => a.kind === 'user_flow') ?? null
        setFlowArtifact(flow)
        if (flow) {
          const full = await api.getArtifact(flow.id)
          if (!active) return
          setDsl((full.head_version?.content as FlowDsl) ?? EMPTY)
          setCanvasKey((k) => k + 1)
        }
      } catch (err) {
        if (active) setError(err instanceof ApiError ? err.message : 'Failed to load project.')
      }
    })()
    return () => {
      active = false
    }
  }, [projectId])

  async function onSend(prompt: string) {
    if (!projectId) return
    setBusy(true)
    setError(null)
    setMessages((m) => [...m, { role: 'user', text: prompt }])
    try {
      if (!flowArtifact) {
        const created = await api.generate(projectId, { kind: 'user_flow', prompt })
        setFlowArtifact(created)
        setDsl((created.head_version?.content as FlowDsl) ?? EMPTY)
        setMessages((m) => [...m, { role: 'ai', text: 'Created the user flow.' }])
      } else {
        const version = await api.aiEdit(flowArtifact.id, prompt)
        setDsl((version.content as FlowDsl) ?? EMPTY)
        setMessages((m) => [...m, { role: 'ai', text: 'Updated the flow.' }])
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
    if (!flowArtifact) return
    setSaving('saving')
    try {
      const content = flowToDsl(graphRef.current.nodes, graphRef.current.edges)
      await api.addVersion(flowArtifact.id, {
        content,
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

  return (
    <div className="relative flex h-[100dvh] flex-col">
      <div className="app-aurora" />

      {/* Top bar */}
      <header className="relative z-10 flex items-center gap-3 px-4 py-3">
        <button
          onClick={() => navigate('/')}
          aria-label="Back to workspace"
          className="grid size-9 place-items-center rounded-[10px] text-text-dim transition hover:bg-white/5 hover:text-text"
        >
          <ArrowLeftIcon size={18} />
        </button>
        <div className="leading-tight">
          <p className="text-sm font-semibold text-text">{project?.name ?? 'Project'}</p>
          <p className="text-xs text-text-dim">User Flow</p>
        </div>
        <div className="ml-auto">
          <button
            onClick={onSave}
            disabled={!flowArtifact || saving === 'saving'}
            className="inline-flex items-center gap-2 rounded-[10px] bg-white/5 px-3.5 py-2 text-sm font-medium text-text transition hover:bg-white/10 disabled:opacity-50"
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
        </div>
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
          {flowArtifact ? (
            <ReactFlowProvider>
              <FlowCanvas key={canvasKey} dsl={dsl} onGraphChange={onGraphChange} />
            </ReactFlowProvider>
          ) : (
            <div className="grid h-full place-items-center px-6 text-center">
              <p className="max-w-sm text-sm text-text-dim">
                No flow yet. Ask the AI Designer on the left to create one to see it on the
                canvas.
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
