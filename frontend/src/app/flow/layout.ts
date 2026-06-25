// UserFlow DSL <-> xyflow mapping + a tiny left-to-right auto-layout.
// Positions saved into the DSL (extra `position` key, ignored by the backend
// validator) take precedence; otherwise nodes are ranked by longest path.

import type { Node, Edge } from '@xyflow/react'

export interface FlowDslNode {
  id: string
  label: string
  kind?: string
  position?: { x: number; y: number }
}
export interface FlowDslEdge {
  from: string
  to: string
  label?: string
}
export interface FlowDsl {
  nodes: FlowDslNode[]
  edges: FlowDslEdge[]
}

export type FlowNodeData = { label: string; kind?: string }

const COL = 280
const ROW = 150

/** Longest-path rank per node (sources at rank 0). */
function rank(nodes: FlowDslNode[], edges: FlowDslEdge[]): Map<string, number> {
  const out = new Map<string, string[]>()
  const indeg = new Map<string, number>()
  nodes.forEach((n) => {
    out.set(n.id, [])
    indeg.set(n.id, 0)
  })
  edges.forEach((e) => {
    if (out.has(e.from) && indeg.has(e.to)) {
      out.get(e.from)!.push(e.to)
      indeg.set(e.to, (indeg.get(e.to) ?? 0) + 1)
    }
  })
  const ranks = new Map<string, number>()
  // Kahn's algorithm; rank = max(pred rank)+1.
  const queue = nodes.filter((n) => (indeg.get(n.id) ?? 0) === 0).map((n) => n.id)
  queue.forEach((id) => ranks.set(id, 0))
  const deg = new Map(indeg)
  while (queue.length) {
    const id = queue.shift()!
    const r = ranks.get(id) ?? 0
    for (const next of out.get(id) ?? []) {
      ranks.set(next, Math.max(ranks.get(next) ?? 0, r + 1))
      deg.set(next, (deg.get(next) ?? 1) - 1)
      if ((deg.get(next) ?? 0) === 0) queue.push(next)
    }
  }
  nodes.forEach((n) => { if (!ranks.has(n.id)) ranks.set(n.id, 0) })
  return ranks
}

export function dslToFlow(input: FlowDsl): { nodes: Node<FlowNodeData>[]; edges: Edge[] } {
  // Tolerate partial/foreign content (e.g. a brief render race on tab switch).
  const dsl: FlowDsl = { nodes: input?.nodes ?? [], edges: input?.edges ?? [] }
  const ranks = rank(dsl.nodes, dsl.edges)
  const perRank = new Map<number, number>() // how many placed in each rank column

  const nodes: Node<FlowNodeData>[] = dsl.nodes.map((n) => {
    let position = n.position
    if (!position) {
      const r = ranks.get(n.id) ?? 0
      const idx = perRank.get(r) ?? 0
      perRank.set(r, idx + 1)
      position = { x: r * COL, y: idx * ROW }
    }
    return { id: n.id, type: 'flow', position, data: { label: n.label, kind: n.kind } }
  })

  const edges: Edge[] = dsl.edges.map((e, i) => ({
    id: `e${i}-${e.from}-${e.to}`,
    source: e.from,
    target: e.to,
    label: e.label,
    type: 'default',
    animated: true,
  }))

  return { nodes, edges }
}

/** Serialize the live xyflow graph back to DSL (with positions) for saving. */
export function flowToDsl(nodes: Node<FlowNodeData>[], edges: Edge[]): FlowDsl {
  return {
    nodes: nodes.map((n) => ({
      id: n.id,
      label: n.data.label,
      kind: n.data.kind,
      position: { x: Math.round(n.position.x), y: Math.round(n.position.y) },
    })),
    edges: edges.map((e) => ({ from: e.source, to: e.target, label: e.label as string | undefined })),
  }
}
