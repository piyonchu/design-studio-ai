import {
  useCallback,
  useEffect,
  useId,
  useRef,
  type ReactNode,
} from 'react'
import { createPortal } from 'react-dom'
import { WarningIcon, SpinnerGapIcon } from '@phosphor-icons/react'

const FOCUSABLE =
  'a[href],button:not([disabled]),textarea:not([disabled]),input:not([disabled]),select:not([disabled]),[tabindex]:not([tabindex="-1"])'

/**
 * Accessible modal shell. Owns the behaviour every dialog must have but the
 * three hand-rolled overlays were missing: a labelled `role="dialog"`, focus
 * moved in on open and restored on close, a focus trap, Escape-to-close, and a
 * portal so the panel escapes any clipping stacking context. Consumers render
 * their own header/body; pass `labelledBy` the id of the title element (or use
 * the `titleId` the render-prop hands back).
 *
 * `variant` positions the panel: `center` for confirm/report modals, `right`
 * for the inspector slide-over.
 */
export function Dialog({
  onClose,
  variant = 'center',
  className = '',
  panelClassName = '',
  z = 'z-50',
  labelledBy,
  initialFocus,
  children,
}: {
  onClose: () => void
  variant?: 'center' | 'right'
  className?: string
  panelClassName?: string
  z?: string
  labelledBy?: string
  initialFocus?: React.RefObject<HTMLElement | null>
  children: (ids: { titleId: string }) => ReactNode
}) {
  const panelRef = useRef<HTMLDivElement>(null)
  const restoreRef = useRef<HTMLElement | null>(null)
  const autoTitleId = useId()
  const titleId = labelledBy ?? autoTitleId

  // Move focus in on open; restore it to the trigger on close.
  useEffect(() => {
    restoreRef.current = document.activeElement as HTMLElement | null
    const panel = panelRef.current
    const target =
      initialFocus?.current ??
      panel?.querySelector<HTMLElement>(FOCUSABLE) ??
      panel
    target?.focus()
    return () => restoreRef.current?.focus?.()
  }, [initialFocus])

  // Lock body scroll while a dialog is open.
  useEffect(() => {
    const prev = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    return () => {
      document.body.style.overflow = prev
    }
  }, [])

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation()
        onClose()
        return
      }
      if (e.key !== 'Tab') return
      const panel = panelRef.current
      if (!panel) return
      const items = [...panel.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
        (el) => el.offsetParent !== null || el === document.activeElement,
      )
      if (items.length === 0) {
        e.preventDefault()
        panel.focus()
        return
      }
      const first = items[0]
      const last = items[items.length - 1]
      const active = document.activeElement as HTMLElement
      if (e.shiftKey && (active === first || active === panel)) {
        e.preventDefault()
        last.focus()
      } else if (!e.shiftKey && active === last) {
        e.preventDefault()
        first.focus()
      }
    },
    [onClose],
  )

  const position =
    variant === 'right'
      ? 'flex justify-end'
      : 'grid place-items-center p-4'

  return createPortal(
    <div className={`fixed inset-0 ${z} ${className}`}>
      <div
        className="absolute inset-0 bg-black/50"
        onClick={onClose}
        aria-hidden
      />
      <div className={`absolute inset-0 ${position}`} onKeyDown={onKeyDown}>
        <div
          ref={panelRef}
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
          tabIndex={-1}
          className={`relative outline-none ${panelClassName}`}
        >
          {children({ titleId })}
        </div>
      </div>
    </div>,
    document.body,
  )
}

/**
 * A confirm step for high-stakes actions — the guardrail the board's costly,
 * irreversible actions (derive-all, batch-approve) were firing without. Built
 * on Dialog, so it inherits focus trap + Escape. `tone` picks the confirm
 * button colour: `primary` (teal) for spend, `danger` (rose) for destructive.
 */
export function ConfirmDialog({
  title,
  body,
  confirmLabel,
  cancelLabel = 'Cancel',
  tone = 'primary',
  busy = false,
  onConfirm,
  onCancel,
}: {
  title: string
  body: ReactNode
  confirmLabel: string
  cancelLabel?: string
  tone?: 'primary' | 'danger'
  busy?: boolean
  onConfirm: () => void
  onCancel: () => void
}) {
  const confirmRef = useRef<HTMLButtonElement>(null)
  const confirmClass =
    tone === 'danger'
      ? 'bg-danger text-bg hover:brightness-110'
      : 'bg-teal text-bg active:translate-y-px'

  return (
    <Dialog onClose={onCancel} z="z-[80]" initialFocus={confirmRef} panelClassName="w-[400px] max-w-[92vw]">
      {({ titleId }) => (
        <div className="glass rounded-[16px] p-5">
          <div className="mb-3 flex items-start gap-3">
            {tone === 'danger' && (
              <span className="mt-0.5 grid size-7 shrink-0 place-items-center rounded-[8px] bg-danger/15 text-danger">
                <WarningIcon size={16} weight="fill" />
              </span>
            )}
            <div className="min-w-0">
              <h2 id={titleId} className="text-sm font-semibold text-text">
                {title}
              </h2>
              <div className="mt-1 text-sm text-text-muted">{body}</div>
            </div>
          </div>
          <div className="mt-4 flex items-center justify-end gap-2">
            <button
              onClick={onCancel}
              className="rounded-[8px] px-3 py-2 text-sm text-text-dim transition hover:bg-white/5 hover:text-text"
            >
              {cancelLabel}
            </button>
            <button
              ref={confirmRef}
              onClick={onConfirm}
              disabled={busy}
              className={`inline-flex items-center gap-1.5 rounded-[8px] px-3.5 py-2 text-sm font-semibold transition disabled:opacity-50 ${confirmClass}`}
            >
              {busy && <SpinnerGapIcon size={14} className="animate-spin" />}
              {confirmLabel}
            </button>
          </div>
        </div>
      )}
    </Dialog>
  )
}
