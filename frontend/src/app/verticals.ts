// Vertical packs — the ONLY domain-specific config. The core (derivation,
// canon, management, RAG, export) is domain-neutral; a new vertical is just a
// preset + canon-hint set here. This is the rule-of-three proof: adding manhwa
// required no core changes. Extract an adapter framework once there are 2–3.

export type Vertical = 'game_2d' | 'manhwa' | 'illustration'

export interface DerivePreset {
  id: string
  label: string
  text: string
}

/** Engines an export pack can target — mirrors the backend `verticals::Engine`. */
export type Engine = 'godot' | 'unity'

export interface VerticalConfig {
  label: string
  /** Derivation presets offered when deriving from a base asset. */
  derivePresets: DerivePreset[]
  /** Canon style fields: [key, label, placeholder]. */
  canonFields: [string, string, string][]
  /** Engines this vertical can emit an import-ready export pack for. */
  engines?: Engine[]
}

export const VERTICALS: Record<Vertical, VerticalConfig> = {
  game_2d: {
    label: 'Game (2D)',
    engines: ['godot', 'unity'],
    derivePresets: [
      { id: 'walk', label: 'Walk', text: 'Show the SAME character in a mid-walk side stride pose. Keep identical identity, palette, and proportions.' },
      { id: 'action', label: 'Action', text: 'Show the SAME character in a dynamic action pose. Keep identical identity, palette, and proportions.' },
      { id: 'variant', label: 'Variant', text: 'An outfit/expression variant of the SAME character. Keep identical shape and proportions.' },
      { id: 'matching', label: 'Matching', text: 'A matching set member in the EXACT same art style, palette, and outline weight. Different subject, same world.' },
    ],
    canonFields: [
      ['render_style', 'Render style', '16-bit pixel art, retro SNES-era sprite'],
      ['perspective', 'Perspective', 'side view'],
      ['palette', 'Palette', 'warm earthy tones, ~16 colors'],
      ['outline', 'Outline', 'clean 1px dark outline'],
      ['shading', 'Shading', 'flat shading with light dithering'],
      ['composition', 'Composition', 'one centered isolated asset, transparent bg'],
    ],
  },
  manhwa: {
    label: 'Manhwa / Webtoon',
    derivePresets: [
      { id: 'expression', label: 'Expression', text: 'The SAME character with a different facial expression. Keep identical identity, hairstyle, outfit, and line style.' },
      { id: 'pose', label: 'Pose', text: 'The SAME character in a new full-body pose for a panel. Keep identical identity, proportions, and costume.' },
      { id: 'angle', label: 'Angle', text: 'The SAME character from a different camera angle. Keep identical identity, costume, and rendering.' },
      { id: 'castmate', label: 'Cast mate', text: 'A different character in the EXACT same art style, line weight, and shading, for the same webtoon.' },
    ],
    canonFields: [
      ['art_style', 'Art style', 'clean webtoon lineart, soft cel shading'],
      ['line_work', 'Line work', 'consistent 2px ink lines, tapered'],
      ['coloring', 'Coloring', 'soft gradients, pastel skin tones'],
      ['character', 'Character notes', 'lead: silver hair, red eyes, school uniform'],
      ['background', 'Background style', 'soft-focus painterly interiors'],
      ['composition', 'Composition', 'vertical-scroll panel, character centered, transparent bg'],
    ],
  },
  illustration: {
    label: 'Illustration',
    derivePresets: [
      { id: 'colorway', label: 'Colorway', text: 'The SAME subject in an alternate color palette. Keep identical shapes, linework, and composition.' },
      { id: 'linework', label: 'Linework', text: 'A clean linework / inked version of the SAME subject. Keep identical proportions and composition.' },
      { id: 'sticker', label: 'Sticker', text: 'A die-cut sticker version of the SAME subject with a bold clean outline. Keep identical identity and palette.' },
      { id: 'seriesmate', label: 'Series mate', text: 'A new subject in the EXACT same illustration style, palette, and linework — a cohesive series member.' },
    ],
    canonFields: [
      ['medium', 'Medium', 'flat vector illustration, gouache texture'],
      ['palette', 'Palette', 'muted retro pastels, ~8 colors'],
      ['linework', 'Linework', 'bold even outlines, rounded corners'],
      ['lighting', 'Lighting', 'soft top-left light, gentle long shadows'],
      ['composition', 'Composition', 'single subject, generous padding, transparent bg'],
    ],
  },
}

export const verticalConfig = (v?: string | null): VerticalConfig =>
  VERTICALS[(v as Vertical) in VERTICALS ? (v as Vertical) : 'game_2d']

/** The engine export targets a project's vertical supports (possibly none). */
export const enginesFor = (v?: string | null): Engine[] => verticalConfig(v).engines ?? []
