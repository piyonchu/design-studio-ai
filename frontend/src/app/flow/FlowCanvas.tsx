import { useEffect } from 'react'
import {
  ReactFlow,
  Background,
  BackgroundVariant,
  Controls,
  useNodesState,
  useEdgesState,
  type Node,
  type Edge,
} from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import { FlowNode } from './FlowNode'
import { dslToFlow, type FlowDsl, type FlowNodeData } from './layout'

const nodeTypes = { flow: FlowNode }

export function FlowCanvas({
  dsl,
  onGraphChange,
}: {
  dsl: FlowDsl
  onGraphChange: (nodes: Node<FlowNodeData>[], edges: Edge[]) => void
}) {
  const initial = dslToFlow(dsl)
  const [nodes, , onNodesChange] = useNodesState(initial.nodes)
  const [edges, , onEdgesChange] = useEdgesState(initial.edges)

  // Surface the live graph to the parent for saving.
  useEffect(() => {
    onGraphChange(nodes, edges)
  }, [nodes, edges, onGraphChange])

  return (
    <ReactFlow
      colorMode="dark"
      nodes={nodes}
      edges={edges}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      nodeTypes={nodeTypes}
      fitView
      proOptions={{ hideAttribution: true }}
      defaultEdgeOptions={{
        type: 'default',
        animated: true,
        style: { stroke: 'var(--color-teal)', strokeWidth: 2 },
      }}
    >
      <Background variant={BackgroundVariant.Dots} gap={22} size={1} color="#2a3142" />
      <Controls showInteractive={false} />
    </ReactFlow>
  )
}
