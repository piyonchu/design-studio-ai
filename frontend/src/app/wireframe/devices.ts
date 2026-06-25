// Frame sizes for the wireframe renderer. Resolved from root `props.device`
// or explicit `props.width`/`props.height`. Default: web (general UI/UX studio).

export type Device = 'web' | 'desktop' | 'tablet' | 'phone'

export const DEVICE_SIZES: Record<Device, { w: number; h: number }> = {
  web: { w: 1280, h: 800 },
  desktop: { w: 1440, h: 900 },
  tablet: { w: 834, h: 1112 },
  phone: { w: 390, h: 844 },
}

export function resolveFrameSize(props: Record<string, unknown> | undefined): {
  w: number
  h: number
} {
  const width = typeof props?.width === 'number' ? props.width : undefined
  const height = typeof props?.height === 'number' ? props.height : undefined
  if (width && height) return { w: width, h: height }
  const device = (props?.device as Device) ?? 'web'
  return DEVICE_SIZES[device] ?? DEVICE_SIZES.web
}
