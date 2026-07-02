import { PencilRulerIcon, XIcon } from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { EditTools } from './EditTools'
import { AssetImage } from '../ui/AssetImage'

/**
 * Edit window — hosts the full B1/B2/B3 tool set (transform · color · cutout ·
 * resize · paint · inpaint) in a focused modal, opened from the inspector's
 * "Edit" button. Kept at a lower z-index than the tools' own sub-dialogs
 * (Paint/Crop/Inpaint, z-60/70) so those layer above this one.
 */
export function EditDialog({
  asset,
  onClose,
  onChanged,
}: {
  asset: api.Asset
  onClose: () => void
  onChanged: (a: api.Asset) => void
}) {
  return (
    <>
      <div className="fixed inset-0 z-[50] bg-black/60" onClick={onClose} aria-hidden />
      <div className="fixed inset-0 z-[55] grid place-items-center p-4" role="dialog" aria-modal>
        <div className="glass flex max-h-[92dvh] w-full max-w-lg flex-col overflow-hidden rounded-[16px]">
          <header className="flex items-center gap-2 border-b border-white/8 px-4 py-3">
            <span className="grid size-7 place-items-center rounded-[8px] bg-accent/15 text-teal-bright">
              <PencilRulerIcon size={15} weight="fill" />
            </span>
            <p className="text-sm font-medium text-text">Edit</p>
            <span className="text-xs text-text-dim">· {api.displayName(asset)}</span>
            <button
              onClick={onClose}
              aria-label="Close"
              className="ml-auto grid size-7 place-items-center rounded-[8px] text-text-dim transition hover:bg-white/5 hover:text-text"
            >
              <XIcon size={16} />
            </button>
          </header>

          <div className="min-h-0 flex-1 overflow-auto p-4">
            {/* Live preview — reflects the current head, so it updates as each
                edit lands a new version. */}
            <AssetImage
              src={asset.url}
              alt={api.displayName(asset)}
              className="mx-auto mb-4 max-h-[38dvh] w-full rounded-[10px] object-contain ring-1 ring-white/10"
              fallbackClassName="mx-auto mb-4 grid h-40 w-full place-items-center rounded-[10px] ring-1 ring-white/10"
            />
            <EditTools asset={asset} onChanged={onChanged} />
          </div>
        </div>
      </div>
    </>
  )
}
