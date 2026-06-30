---
name: CanonForge
description: Reference-driven visual asset studio for canon-bound creative production.
colors:
  obsidian-bg: "#0a0b0f"
  surface: "#12141b"
  surface-raised: "#181b24"
  border: "#242938"
  text: "#e7e9ee"
  text-muted: "#9aa1b0"
  text-dim: "#828a9b"
  signal-teal: "#2dd4bf"
  signal-teal-bright: "#5eead4"
  quiet-indigo: "#6366f1"
  quiet-indigo-bright: "#818cf8"
typography:
  display:
    fontFamily: "Geist Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "1.5rem"
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: "-0.025em"
  title:
    fontFamily: "Geist Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 600
    lineHeight: 1.4
  body:
    fontFamily: "Geist Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "Geist Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 500
    lineHeight: 1.4
rounded:
  control: "10px"
  panel: "16px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "20px"
  2xl: "24px"
components:
  button-primary:
    backgroundColor: "{colors.signal-teal}"
    textColor: "{colors.obsidian-bg}"
    rounded: "{rounded.control}"
    padding: "10px 16px"
  button-secondary:
    backgroundColor: "{colors.quiet-indigo}"
    textColor: "#ffffff"
    rounded: "{rounded.control}"
    padding: "8px 16px"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.text-dim}"
    rounded: "{rounded.control}"
    padding: "8px 12px"
  input:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    padding: "10px 12px"
  panel:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.panel}"
---

# Design System: CanonForge

## 1. Overview

**Creative North Star: "The Canon Control Room"**

CanonForge is a precise, dark, production-focused control surface for creative asset work. Its visual system uses obsidian surfaces, controlled translucency, and sharp signal colors to keep the user's attention on canon, status, lineage, review, and export readiness.

This is not a decorative AI playground. The interface should feel like a serious studio tool: dense enough for production, restrained enough for trust, and explicit about state. Glass treatment is allowed only as functional containment for app panels, rails, dialogs, and cards; it is never a generic luxury effect.

**Key Characteristics:**
- Dark obsidian canvas with layered glass panels and subtle aurora background.
- Signal teal for primary decisions, approvals, selections, and current navigation.
- Quiet indigo for secondary actions and account/project accents.
- Compact Geist typography with small labels, tight controls, and clear state language.
- Dense production surfaces that expose provenance, status, and readiness.

## 2. Colors

The palette is obsidian surfaces with signal teal and quiet indigo: dark enough for long creative sessions, bright enough to make workflow state unmistakable.

### Primary
- **Signal Teal**: The primary action and state color. Use it for generate/derive buttons, approval actions, selected tabs, selected filters, active toggles, and success-ready signals.
- **Signal Teal Bright**: The luminous companion for icons, active nav text, badges, and subtle emphasis where the flat teal fill would be too heavy.

### Secondary
- **Quiet Indigo**: The secondary action color. Use it for account accents, create-confirm actions, focus rings where teal is already occupied, and rare supporting emphasis.
- **Quiet Indigo Bright**: Use sparingly for elevated indigo text or icon moments.

### Neutral
- **Obsidian Background**: The app canvas. It should remain dominant and calm.
- **Surface**: The base panel layer for glass fallback and stable containers.
- **Surface Raised**: The field, toolbar, dialog, and nested control layer.
- **Border**: The solid border token for fields and high-clarity edges.
- **Text**: Primary foreground text on all dark surfaces.
- **Text Muted**: Secondary body and metadata text.
- **Text Dim**: Low-priority labels, inactive icons, counts, and helper text.

### Named Rules

**The Signal Rarity Rule.** Teal is the user's production signal, not decoration. If more than a few elements compete in teal, the hierarchy is broken.

**The Status Must Survive Rule.** Approval, rejection, needs-review, warnings, and export readiness must use color plus shape, icon, ring, text, or placement. Color alone is not enough.

## 3. Typography

**Display Font:** Geist Variable with ui-sans-serif and system fallbacks.
**Body Font:** Geist Variable with ui-sans-serif and system fallbacks.
**Label/Mono Font:** Geist Variable; use tabular numerals where counts or dimensions align.

**Character:** Geist keeps the product compact and technical without becoming sterile. The system uses one family across headings, labels, controls, and metadata so the interface disappears into the task.

### Hierarchy
- **Display** (600, 1.5rem, 1.2 line-height): Workspace and major page headings only.
- **Headline** (600, 1rem, 1.35 line-height): Panel titles, dialog titles, and important subheads.
- **Title** (600, 0.875rem, 1.4 line-height): Cards, toolbar labels, asset names, and section headers.
- **Body** (400, 0.875rem, 1.5 line-height): Form fields, explanations, comments, synthesized answers, and inspector content.
- **Label** (500-600, 0.625-0.75rem, tight line-height): Filter section labels, badges, metadata, source labels, and compact controls.

### Named Rules

**The Fixed Scale Rule.** This is product UI, not a landing page. Use fixed rem sizes and a tight ratio; do not introduce fluid hero typography inside app surfaces.

**The Label Restraint Rule.** Uppercase tracked labels are allowed for rail group names and report groupings only. They should not become a section-decoration habit.

## 4. Elevation

CanonForge uses a hybrid of tonal layering, translucent panels, 1px borders, and one ambient shadow. The shadow exists to separate glass containers from the obsidian field; it should not be stacked into heavy card piles. Depth is mostly structural: side rails, top bars, dialogs, inspectors, and board panels sit on clear layers.

### Shadow Vocabulary
- **Glass Ambient** (`inset 0 1px 0 rgb(255 255 255 / 0.06), 0 12px 40px rgb(0 0 0 / 0.35)`): Use for the `.glass` container treatment only.
- **Modal Lift** (`shadow-2xl` equivalent): Use only for blocking dialogs that must sit above the workspace.

### Named Rules

**The Functional Glass Rule.** Glass is a containment material for product regions. If the blur does not clarify hierarchy or separate a working surface, remove it.

## 5. Components

### Buttons
- **Shape:** Gently curved controls with a 10px radius.
- **Primary:** Signal teal fill with obsidian text, medium-semibold label, and compact padding.
- **Hover / Focus:** Use ring or color-strength changes; active state may depress by 1px.
- **Secondary / Ghost:** Indigo fill for secondary confirmation; transparent ghost buttons use dim text and a subtle white border or hover fill.

### Chips
- **Style:** Small rounded pills with tinted backgrounds, compact labels, and optional icons.
- **State:** Selected filters use teal tint and bright teal text. Inactive chips stay dim until hover.

### Cards / Containers
- **Corner Style:** Panels use a 16px radius; nested controls use 8-10px.
- **Background:** Main containers use `.glass`; toolbars and fields use raised surface tints.
- **Shadow Strategy:** Follow the Functional Glass Rule. Avoid nested shadow stacks.
- **Border:** Prefer 1px white-alpha or border token edges; use rings for asset review state.
- **Internal Padding:** Dense panels use 12-20px padding depending on task weight.

### Inputs / Fields
- **Style:** Raised dark field, 8-12px radius, primary text, dim placeholder, and no ornamental chrome.
- **Focus:** Teal or indigo focus ring with visible contrast; never rely on border color alone.
- **Error / Disabled:** Error text and containers use rose tones with border/background pairing; disabled controls reduce opacity but keep labels readable.

### Navigation
- **Style, typography, default/hover/active states, mobile treatment.** Navigation is rail-first. Active items use teal tint and bright teal text; inactive items use dim text and white-alpha hover fill. Keep labels short and icons consistent.

### Asset Board

The asset board is the signature product surface. It combines filter rail, generate/derive bar, semantic search, batch toolbar, tile grid, review state rings, and inspector entry points. The board may be dense, but every tile must preserve source kind, status, exemplar state, and inspect/review affordances.

## 6. Do's and Don'ts

### Do:
- **Do** keep the user's production loop visible: reference, canon, derivation, review, collection, export.
- **Do** use Signal Teal for decisions, selected state, approvals, and primary action.
- **Do** pair status color with iconography, text, ring thickness, opacity, or placement.
- **Do** keep controls compact and stateful; users are managing libraries, not reading a brochure.
- **Do** preserve reduced-motion support for every animation and transition.

### Don't:
- **Don't** make CanonForge feel like a text-to-image slot machine, generic asset manager, or novelty AI playground.
- **Don't** drift into Figma/v0-style page layout or UI-code generation patterns.
- **Don't** use generic SaaS marketing tropes inside product surfaces: ornamental metrics, decorative gradients, vague magic copy, or hero-metric layouts.
- **Don't** use glassmorphism as decoration. Glass must clarify containment or hierarchy.
- **Don't** hide provenance, canon version, review state, export readiness, or lineage to make a surface look cleaner.
