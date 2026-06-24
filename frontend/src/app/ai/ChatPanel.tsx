import { useState, type FormEvent } from 'react'
import { PaperPlaneTiltIcon, SparkleIcon, SpinnerGapIcon } from '@phosphor-icons/react'

export interface ChatMessage {
  role: 'user' | 'ai'
  text: string
}

export function ChatPanel({
  messages,
  busy,
  onSend,
}: {
  messages: ChatMessage[]
  busy: boolean
  onSend: (prompt: string) => void
}) {
  const [value, setValue] = useState('')

  function submit(e: FormEvent) {
    e.preventDefault()
    const v = value.trim()
    if (!v || busy) return
    onSend(v)
    setValue('')
  }

  return (
    <aside className="glass flex w-[360px] shrink-0 flex-col rounded-[16px]">
      <div className="flex items-center gap-2 border-b border-white/8 px-4 py-3">
        <span className="grid size-7 place-items-center rounded-[8px] bg-teal/15 text-teal-bright">
          <SparkleIcon size={15} weight="fill" />
        </span>
        <p className="text-sm font-medium text-text">AI Designer</p>
      </div>

      <div className="flex-1 space-y-3 overflow-y-auto px-4 py-4">
        {messages.length === 0 && (
          <p className="text-sm leading-relaxed text-text-dim">
            Describe the flow you want. Try{' '}
            <span className="text-teal-bright">"Design a 3-step onboarding flow"</span>.
          </p>
        )}
        {messages.map((m, i) => (
          <div
            key={i}
            className={
              m.role === 'user'
                ? 'ml-auto max-w-[85%] rounded-[12px] bg-indigo/25 px-3 py-2 text-sm text-text'
                : 'max-w-[90%] rounded-[12px] bg-white/5 px-3 py-2 text-sm text-text-muted'
            }
          >
            {m.text}
          </div>
        ))}
        {busy && (
          <div className="flex items-center gap-2 text-sm text-text-dim">
            <SpinnerGapIcon size={15} className="animate-spin" /> Mapping user journey…
          </div>
        )}
      </div>

      <form onSubmit={submit} className="border-t border-white/8 p-3">
        <div className="flex items-end gap-2 rounded-[12px] bg-surface-2/60 p-2">
          <textarea
            rows={2}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) submit(e)
            }}
            placeholder="Ask the AI to design or change the flow…"
            className="flex-1 resize-none bg-transparent text-sm text-text outline-none placeholder:text-text-dim"
          />
          <button
            type="submit"
            disabled={busy || !value.trim()}
            aria-label="Send"
            className="grid size-9 shrink-0 place-items-center rounded-[10px] bg-teal text-bg transition active:translate-y-px disabled:opacity-50"
          >
            <PaperPlaneTiltIcon size={16} weight="fill" />
          </button>
        </div>
      </form>
    </aside>
  )
}
