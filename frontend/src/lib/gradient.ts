// Deterministic indigo↔teal gradient seeded from an id, for project-card tiles
// (placeholder until real rendered-DSL previews exist).

function hash(s: string): number {
  let h = 2166136261
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  return h >>> 0
}

/** A CSS gradient string in the locked indigo/teal accent range. */
export function seededGradient(id: string): string {
  const h = hash(id)
  const angle = h % 360
  // Hue wanders between indigo (~245) and teal (~170).
  const h1 = 170 + (h % 80)
  const h2 = 230 + ((h >> 8) % 30)
  return `linear-gradient(${angle}deg, hsl(${h1} 70% 45% / 0.55), hsl(${h2} 75% 55% / 0.45))`
}
