import { useMemo, useState, type ReactNode } from 'react'
import {
  CaretRightIcon,
  FolderIcon,
  FolderOpenIcon,
  FolderPlusIcon,
  PencilSimpleIcon,
  TrashIcon,
  StackIcon,
  CheckIcon,
  XIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ConfirmDialog } from '../ui/Dialog'

/**
 * The project's folder tree — the asset's canonical home (one tree, like files),
 * distinct from collections (cross-cutting sets). Select a folder to scope the
 * board; drag a tile onto a folder (or "Unfiled") to move it. Inline create /
 * rename / delete. `selected`: null = all assets, 'root' = unfiled, else a id.
 */
export function FolderTree({
  projectId,
  folders,
  selected,
  onSelect,
  onChanged,
  onMoveAsset,
}: {
  projectId: string
  folders: api.FolderNode[]
  selected: string | null
  onSelect: (sel: string | null) => void
  onChanged: () => void
  onMoveAsset: (assetId: string, folderId: string | null) => void
}) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set())
  const [editingId, setEditingId] = useState<string | null>(null)
  const [addingUnder, setAddingUnder] = useState<string | null | undefined>(undefined)
  const [draft, setDraft] = useState('')
  const [dropTarget, setDropTarget] = useState<string | null | undefined>(undefined)
  const [pendingDelete, setPendingDelete] = useState<api.FolderNode | null>(null)

  // Children indexed by parent_id ('' = root) for O(1) tree walks.
  const byParent = useMemo(() => {
    const m = new Map<string, api.FolderNode[]>()
    for (const f of folders) {
      const k = f.parent_id ?? ''
      ;(m.get(k) ?? m.set(k, []).get(k)!).push(f)
    }
    return m
  }, [folders])

  function toggle(id: string) {
    setExpanded((s) => {
      const next = new Set(s)
      next.has(id) ? next.delete(id) : next.add(id)
      return next
    })
  }

  async function commitCreate(parentId: string | null) {
    const name = draft.trim()
    if (!name) {
      setAddingUnder(undefined)
      return
    }
    try {
      await api.createFolder(projectId, name, parentId)
      if (parentId) setExpanded((s) => new Set(s).add(parentId))
      onChanged()
    } catch {
      /* ignore */
    } finally {
      setAddingUnder(undefined)
      setDraft('')
    }
  }

  async function commitRename(id: string) {
    const name = draft.trim()
    setEditingId(null)
    if (!name) return
    try {
      await api.updateFolder(id, { name })
      onChanged()
    } catch {
      /* ignore */
    }
  }

  async function doRemove(f: api.FolderNode) {
    try {
      await api.deleteFolder(f.id)
      if (selected === f.id) onSelect(null)
      onChanged()
    } catch {
      /* ignore */
    }
  }

  function NewInput({ onCommit, onCancel }: { onCommit: () => void; onCancel: () => void }) {
    return (
      <div className="flex items-center gap-1 px-1 py-0.5">
        <input
          autoFocus
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') onCommit()
            if (e.key === 'Escape') onCancel()
          }}
          placeholder="Folder name…"
          aria-label="Folder name"
          className="min-w-0 flex-1 rounded-[6px] bg-surface/80 px-2 py-1 text-xs text-text outline-none ring-1 ring-teal/40"
        />
        <button onClick={onCommit} aria-label="Save" className="text-teal-bright hover:text-teal">
          <CheckIcon size={13} weight="bold" />
        </button>
        <button onClick={onCancel} aria-label="Cancel" className="text-text-dim hover:text-text">
          <XIcon size={13} />
        </button>
      </div>
    )
  }

  function Row({ node, depth }: { node: api.FolderNode; depth: number }): ReactNode {
    const kids = byParent.get(node.id) ?? []
    const isOpen = expanded.has(node.id)
    const isSel = selected === node.id
    const isDrop = dropTarget === node.id
    return (
      <div key={node.id}>
        {editingId === node.id ? (
          <div style={{ paddingLeft: depth * 12 }}>
            <NewInput onCommit={() => commitRename(node.id)} onCancel={() => setEditingId(null)} />
          </div>
        ) : (
          <div
            onDragOver={(e) => {
              e.preventDefault()
              setDropTarget(node.id)
            }}
            onDragLeave={() => setDropTarget((t) => (t === node.id ? undefined : t))}
            onDrop={(e) => {
              e.preventDefault()
              setDropTarget(undefined)
              const id = e.dataTransfer.getData('text/asset-id')
              if (id) onMoveAsset(id, node.id)
            }}
            style={{ paddingLeft: depth * 12 }}
            className={`group flex items-center gap-1 rounded-[8px] pr-1 transition ${
              isDrop ? 'bg-teal/20 ring-1 ring-teal/50' : isSel ? 'bg-teal/15' : 'hover:bg-white/5'
            }`}
          >
            <button
              onClick={() => kids.length && toggle(node.id)}
              className={`grid size-5 shrink-0 place-items-center text-text-dim ${kids.length ? '' : 'invisible'}`}
              aria-label={isOpen ? 'Collapse' : 'Expand'}
            >
              <CaretRightIcon size={11} weight="bold" className={`transition ${isOpen ? 'rotate-90' : ''}`} />
            </button>
            <button
              onClick={() => onSelect(node.id)}
              className={`flex min-w-0 flex-1 items-center gap-1.5 py-1 text-left text-xs transition ${
                isSel ? 'text-teal-bright' : 'text-text-dim group-hover:text-text'
              }`}
            >
              {isOpen ? <FolderOpenIcon size={14} weight="fill" /> : <FolderIcon size={14} weight={isSel ? 'fill' : 'regular'} />}
              <span className="flex-1 truncate">{node.name}</span>
              <span className="text-[10px] tabular-nums text-text-dim">{node.asset_count || ''}</span>
            </button>
            {/* Hover actions */}
            <div className="flex shrink-0 items-center gap-0.5 opacity-0 transition group-hover:opacity-100">
              <button
                onClick={() => {
                  setDraft('')
                  setAddingUnder(node.id)
                  setExpanded((s) => new Set(s).add(node.id))
                }}
                aria-label="New subfolder"
                title="New subfolder"
                className="grid size-5 place-items-center text-text-dim hover:text-text"
              >
                <FolderPlusIcon size={12} />
              </button>
              <button
                onClick={() => {
                  setDraft(node.name)
                  setEditingId(node.id)
                }}
                aria-label="Rename"
                title="Rename"
                className="grid size-5 place-items-center text-text-dim hover:text-text"
              >
                <PencilSimpleIcon size={12} />
              </button>
              <button
                onClick={() => setPendingDelete(node)}
                aria-label="Delete"
                title="Delete"
                className="grid size-5 place-items-center text-text-dim hover:text-rose-300"
              >
                <TrashIcon size={12} />
              </button>
            </div>
          </div>
        )}
        {addingUnder === node.id && (
          <div style={{ paddingLeft: (depth + 1) * 12 }}>
            <NewInput onCommit={() => commitCreate(node.id)} onCancel={() => setAddingUnder(undefined)} />
          </div>
        )}
        {isOpen && kids.map((k) => Row({ node: k, depth: depth + 1 }))}
      </div>
    )
  }

  const roots = byParent.get('') ?? []

  return (
    <div className="mb-4">
      <div className="mb-1 flex items-center gap-1 px-2.5">
        <p className="flex-1 text-[10px] font-semibold uppercase tracking-wider text-text-dim">Folders</p>
        <button
          onClick={() => {
            setDraft('')
            setAddingUnder(null)
          }}
          aria-label="New folder"
          title="New folder"
          className="grid size-5 place-items-center text-text-dim transition hover:text-text"
        >
          <FolderPlusIcon size={13} />
        </button>
      </div>

      {/* All assets (no folder filter) */}
      <button
        onClick={() => onSelect(null)}
        className={`flex w-full items-center gap-1.5 rounded-[8px] px-2.5 py-1.5 text-left text-xs transition ${
          selected === null ? 'bg-teal/15 text-teal-bright' : 'text-text-dim hover:bg-white/5 hover:text-text'
        }`}
      >
        <StackIcon size={14} weight={selected === null ? 'fill' : 'regular'} />
        All assets
      </button>

      {/* Unfiled (root) — also a drop target to clear an asset's folder */}
      <button
        onClick={() => onSelect('root')}
        onDragOver={(e) => {
          e.preventDefault()
          setDropTarget('root')
        }}
        onDragLeave={() => setDropTarget((t) => (t === 'root' ? undefined : t))}
        onDrop={(e) => {
          e.preventDefault()
          setDropTarget(undefined)
          const id = e.dataTransfer.getData('text/asset-id')
          if (id) onMoveAsset(id, null)
        }}
        className={`flex w-full items-center gap-1.5 rounded-[8px] px-2.5 py-1.5 text-left text-xs transition ${
          dropTarget === 'root'
            ? 'bg-teal/20 ring-1 ring-teal/50'
            : selected === 'root'
              ? 'bg-teal/15 text-teal-bright'
              : 'text-text-dim hover:bg-white/5 hover:text-text'
        }`}
      >
        <FolderIcon size={14} weight={selected === 'root' ? 'fill' : 'regular'} className="opacity-70" />
        Unfiled
      </button>

      <div className="mt-0.5">
        {roots.map((r) => Row({ node: r, depth: 0 }))}
        {addingUnder === null && (
          <NewInput onCommit={() => commitCreate(null)} onCancel={() => setAddingUnder(undefined)} />
        )}
      </div>

      {pendingDelete && (
        <ConfirmDialog
          title={`Delete folder “${pendingDelete.name}”?`}
          body={
            pendingDelete.asset_count > 0
              ? `Its ${pendingDelete.asset_count} asset${pendingDelete.asset_count > 1 ? 's' : ''} will move to Unfiled — they're not deleted.`
              : 'This folder is empty — nothing will move when you delete it.'
          }
          confirmLabel="Delete folder"
          tone="danger"
          onCancel={() => setPendingDelete(null)}
          onConfirm={() => {
            const f = pendingDelete
            setPendingDelete(null)
            doRemove(f)
          }}
        />
      )}
    </div>
  )
}
