import { useState } from 'react'
import { ImageBrokenIcon } from '@phosphor-icons/react'

/**
 * Asset thumbnail with a stable fallback when the file proxy 404s, the URL is
 * stale, or the browser cannot decode the image.
 */
export function AssetImage({
  src,
  alt = '',
  className = '',
  fallbackClassName,
}: {
  src: string
  alt?: string
  className?: string
  /** Defaults to `className` so the placeholder matches the tile footprint. */
  fallbackClassName?: string
}) {
  const [failed, setFailed] = useState(false)
  const fallbackCls = fallbackClassName ?? className

  if (!src || failed) {
    return (
      <div
        className={`grid place-items-center bg-surface-2/50 text-text-dim ${fallbackCls}`}
        role="img"
        aria-label={alt ? `${alt} (preview unavailable)` : 'Preview unavailable'}
      >
        <ImageBrokenIcon size={22} weight="duotone" />
      </div>
    )
  }

  return (
    <img
      src={src}
      alt={alt}
      loading="lazy"
      decoding="async"
      className={className}
      onError={() => setFailed(true)}
    />
  )
}
