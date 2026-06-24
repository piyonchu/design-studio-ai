import { Handle, Position, type NodeProps } from '@xyflow/react'
import type { FlowNodeData } from './layout'

const KIND_LABEL: Record<string, string> = {
  screen: 'Screen',
  decision: 'Decision',
  action: 'Action',
}

export function FlowNode({ data }: NodeProps) {
  const d = data as FlowNodeData
  return (
    <div className="glass min-w-[160px] rounded-[14px] px-4 py-3 text-left">
      <Handle
        type="target"
        position={Position.Left}
        className="!size-2 !border-0 !bg-teal"
      />
      <p className="text-sm font-semibold text-text">{d.label}</p>
      {d.kind && (
        <p className="mt-0.5 text-[11px] uppercase tracking-wide text-teal-bright">
          {KIND_LABEL[d.kind] ?? d.kind}
        </p>
      )}
      <Handle
        type="source"
        position={Position.Right}
        className="!size-2 !border-0 !bg-teal"
      />
    </div>
  )
}
