import { XIcon } from '@phosphor-icons/react'

/**
 * Inline error with optional recovery. role="alert" so screen readers announce
 * failures; retry/dismiss keep one broken panel from blocking the whole surface.
 */
export function ErrorBanner({
  message,
  onRetry,
  onDismiss,
  className = '',
}: {
  message: string
  onRetry?: () => void
  onDismiss?: () => void
  className?: string
}) {
  return (
    <div
      role="alert"
      className={`flex flex-wrap items-center gap-x-3 gap-y-1.5 rounded-[10px] border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger ${className}`}
    >
      <p className="min-w-0 flex-1 break-words">{message}</p>
      {onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="shrink-0 font-medium text-danger underline-offset-2 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-danger/40"
        >
          Try again
        </button>
      )}
      {onDismiss && (
        <button
          type="button"
          onClick={onDismiss}
          aria-label="Dismiss error"
          className="icon-btn size-8 shrink-0 text-danger/80 hover:bg-danger/15 hover:text-danger"
        >
          <XIcon size={14} />
        </button>
      )}
    </div>
  )
}
