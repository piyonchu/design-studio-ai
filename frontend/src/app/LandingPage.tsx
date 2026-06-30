import type { CSSProperties } from 'react'
import { Link } from 'react-router-dom'
import {
  ArrowRightIcon,
  BrainIcon,
  CheckCircleIcon,
  ImageIcon,
  MusicNotesIcon,
  PackageIcon,
  PaletteIcon,
  SparkleIcon,
  TreeStructureIcon,
  UsersThreeIcon,
} from '@phosphor-icons/react'

const loop = [
  'Reference',
  'Canon',
  'Derive',
  'Review',
  'Organize',
  'Export',
]

const verticals = [
  {
    title: '2D game assets',
    body: 'Sprites, props, tiles, UI assets, SFX, and engine-ready packs for Godot or Unity.',
    icon: PackageIcon,
    accent: 'teal',
    span: 'lg:col-span-2',
  },
  {
    title: 'Manhwa / webtoon',
    body: 'Keep character identity stable across poses, expressions, panels, and backgrounds.',
    icon: UsersThreeIcon,
    accent: 'indigo',
    span: '',
  },
  {
    title: 'Illustration sets',
    body: 'Build cohesive sticker packs, spot-art sets, and visual series from approved references.',
    icon: PaletteIcon,
    accent: 'teal',
    span: '',
  },
  {
    title: 'Marketing imagery',
    body: 'Produce on-brand hero variants, campaign images, and icon sets without losing house style.',
    icon: ImageIcon,
    accent: 'indigo',
    span: 'lg:col-span-2',
  },
]

const proof = [
  {
    title: 'Approved assets become exemplars',
    body: 'Only reviewed work feeds the style loop, so one bad candidate cannot poison the project.',
    icon: CheckCircleIcon,
  },
  {
    title: 'Lineage stays visible',
    body: 'Every derivative records its base and canon version, making stale assets easy to find.',
    icon: TreeStructureIcon,
  },
  {
    title: 'Context travels with the project',
    body: 'Prompts, comments, canon notes, and briefs become searchable project memory.',
    icon: BrainIcon,
  },
]

function AssetTile({
  name,
  tone,
  className = '',
  index = 0,
}: {
  name: string
  tone: 'teal' | 'indigo' | 'amber' | 'rose'
  className?: string
  index?: number
}) {
  const toneClass = {
    teal: 'from-teal/40 via-teal/10 to-surface-2 ring-teal/35',
    indigo: 'from-indigo/45 via-indigo/10 to-surface-2 ring-indigo/35',
    amber: 'from-amber-300/35 via-amber-300/10 to-surface-2 ring-amber-300/35',
    rose: 'from-rose-400/35 via-rose-400/10 to-surface-2 ring-rose-400/35',
  }[tone]

  return (
    <div
      style={{ '--i': index } as CSSProperties}
      className={`hero-tile relative overflow-hidden rounded-[14px] bg-gradient-to-br ${toneClass} p-3 ring-1 ${className}`}
    >
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_30%_15%,rgb(255_255_255_/_0.22),transparent_30%)]" />
      <div className="relative flex aspect-square items-end">
        <span className="rounded-[8px] bg-black/45 px-2 py-1 text-[11px] font-medium text-white/90 backdrop-blur">
          {name}
        </span>
      </div>
    </div>
  )
}

function SectionIntro({
  title,
  body,
}: {
  title: string
  body: string
}) {
  return (
    <div className="mx-auto mb-10 max-w-3xl text-center">
      <h2 className="text-balance text-3xl font-semibold tracking-[-0.025em] text-text sm:text-4xl">
        {title}
      </h2>
      <p className="mx-auto mt-3 max-w-2xl text-pretty text-base leading-7 text-text-muted">
        {body}
      </p>
    </div>
  )
}

export function LandingPage() {
  return (
    <div className="relative min-h-[100dvh] overflow-hidden">
      <div className="app-aurora" />

      <header className="relative z-10 mx-auto flex w-full max-w-7xl items-center justify-between px-4 py-5 sm:px-6 lg:px-8">
        <Link to="/" className="flex items-center gap-2.5" aria-label="CanonForge home">
          <span className="grid size-10 place-items-center rounded-[12px] bg-teal/15 text-teal-bright ring-1 ring-teal/25">
            <SparkleIcon size={21} weight="fill" />
          </span>
          <span className="text-sm font-semibold tracking-tight text-text">CanonForge</span>
        </Link>
        <nav className="hidden items-center gap-6 text-sm text-text-dim md:flex" aria-label="Landing navigation">
          <a href="#loop" className="transition hover:text-text">
            Loop
          </a>
          <a href="#verticals" className="transition hover:text-text">
            Verticals
          </a>
          <a href="#export" className="transition hover:text-text">
            Export
          </a>
        </nav>
        <div className="flex items-center gap-2">
          <Link
            to="/login?mode=login"
            className="hidden rounded-[10px] px-3 py-2 text-sm font-medium text-text-dim transition hover:bg-white/5 hover:text-text sm:inline-flex"
          >
            Sign in
          </Link>
          <Link
            to="/login?mode=signup"
            className="group inline-flex min-h-11 items-center gap-2 rounded-[10px] bg-teal px-4 py-2.5 text-sm font-semibold text-bg transition hover:brightness-105 active:translate-y-px"
          >
            Create workspace
            <ArrowRightIcon
              size={15}
              weight="bold"
              className="transition-transform duration-200 ease-out group-hover:translate-x-0.5"
            />
          </Link>
        </div>
      </header>

      <main className="relative z-10">
        <section className="mx-auto grid w-full max-w-7xl items-center gap-10 px-4 pb-16 pt-8 sm:px-6 sm:pb-24 sm:pt-16 lg:grid-cols-[0.95fr_1.05fr] lg:px-8">
          <div className="max-w-3xl">
            <div
              style={{ '--i': 0 } as CSSProperties}
              className="hero-rise mb-6 inline-flex items-center gap-2 rounded-full border border-teal/25 bg-teal/10 px-3 py-1.5 text-sm font-medium text-teal-bright"
            >
              <span className="size-1.5 rounded-full bg-teal-bright" />
              Reference-driven asset production
            </div>
            <h1
              style={{ '--i': 1 } as CSSProperties}
              className="hero-rise max-w-4xl text-balance text-[clamp(2.75rem,7vw,5.75rem)] font-semibold leading-[0.95] tracking-[-0.04em] text-text"
            >
              One reference. A whole consistent set.
            </h1>
            <p
              style={{ '--i': 2 } as CSSProperties}
              className="hero-rise mt-6 max-w-2xl text-pretty text-lg leading-8 text-text-muted sm:text-xl"
            >
              CanonForge turns approved art direction into project memory: derive variants, review candidates, organize libraries, and export packs that are ready for production tools.
            </p>
            <div style={{ '--i': 3 } as CSSProperties} className="hero-rise mt-8 flex flex-col gap-3 sm:flex-row">
              <Link
                to="/login?mode=signup"
                className="group inline-flex min-h-12 items-center justify-center gap-2 rounded-[12px] bg-teal px-5 py-3 text-sm font-semibold text-bg transition hover:brightness-105 active:translate-y-px"
              >
                Start with a reference
                <ArrowRightIcon
                  size={16}
                  weight="bold"
                  className="transition-transform duration-200 ease-out group-hover:translate-x-0.5"
                />
              </Link>
              <a
                href="#loop"
                className="inline-flex min-h-12 items-center justify-center rounded-[12px] border border-white/10 px-5 py-3 text-sm font-semibold text-text-dim transition hover:bg-white/5 hover:text-text"
              >
                See the workflow
              </a>
            </div>
            <p
              style={{ '--i': 4 } as CSSProperties}
              className="hero-rise mt-5 max-w-xl text-sm leading-6 text-text-dim"
            >
              Mock-first by default for free development. Real image, embedding, LLM, and audio providers stay behind explicit flags.
            </p>
          </div>

          <div
            style={{ '--i': 2 } as CSSProperties}
            className="glass hero-rise relative rounded-[24px] p-3 shadow-2xl"
          >
            <div className="rounded-[20px] border border-white/8 bg-surface/80 p-4">
              <div className="mb-4 flex items-center justify-between gap-4">
                <div>
                  <p className="text-sm font-semibold text-text">Project canon</p>
                  <p className="mt-0.5 text-xs text-text-dim">v12 · approved sprite style</p>
                </div>
                <span className="rounded-full border border-teal/25 bg-teal/10 px-3 py-1 text-xs font-medium text-teal-bright">
                  4 exemplars active
                </span>
              </div>

              <div className="grid grid-cols-4 gap-2">
                <AssetTile name="base" tone="teal" className="col-span-2 row-span-2" index={0} />
                <AssetTile name="pose" tone="indigo" index={1} />
                <AssetTile name="recolor" tone="amber" index={2} />
                <AssetTile name="attack" tone="rose" index={3} />
                <AssetTile name="matching" tone="teal" index={4} />
              </div>

              <div className="mt-4 grid gap-3 lg:grid-cols-[1fr_0.8fr]">
                <div className="rounded-[14px] border border-white/8 bg-white/[0.03] p-3">
                  <div className="mb-3 flex items-center gap-2 text-xs font-medium text-text-muted">
                    <TreeStructureIcon size={15} className="text-teal-bright" />
                    Lineage
                  </div>
                  <div className="flex items-center gap-2 text-[11px] text-text-dim">
                    <span className="rounded-[8px] bg-teal/15 px-2 py-1 text-teal-bright">base</span>
                    <span style={{ '--i': 0 } as CSSProperties} className="hero-connector h-px flex-1 bg-white/15" />
                    <span className="rounded-[8px] bg-indigo/15 px-2 py-1 text-indigo-bright">derive</span>
                    <span style={{ '--i': 1 } as CSSProperties} className="hero-connector h-px flex-1 bg-white/15" />
                    <span className="rounded-[8px] bg-white/8 px-2 py-1 text-text-muted">review</span>
                  </div>
                </div>
                <div className="rounded-[14px] border border-white/8 bg-white/[0.03] p-3">
                  <div className="mb-3 flex items-center gap-2 text-xs font-medium text-text-muted">
                    <PackageIcon size={15} className="text-teal-bright" />
                    Export pack
                  </div>
                  <div className="space-y-1.5 text-[11px] text-text-dim">
                    <p className="flex justify-between gap-3">
                      <span>manifest.json</span>
                      <span className="text-teal-bright">ready</span>
                    </p>
                    <p className="flex justify-between gap-3">
                      <span>Godot 4</span>
                      <span>textures + .import</span>
                    </p>
                    <p className="flex justify-between gap-3">
                      <span>Unity</span>
                      <span>Sprite .meta</span>
                    </p>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </section>

        <section id="loop" className="mx-auto max-w-7xl px-4 py-16 sm:px-6 lg:px-8">
          <div className="glass rounded-[24px] p-5 sm:p-7">
            <div className="flex flex-col gap-6 lg:flex-row lg:items-center lg:justify-between">
              <div className="max-w-2xl">
                <h2 className="text-balance text-2xl font-semibold tracking-tight text-text sm:text-3xl">
                  The loop is the product.
                </h2>
                <p className="mt-3 text-pretty text-base leading-7 text-text-muted">
                  Generation is only the first move. CanonForge keeps the approved style, review history, and export constraints attached to every asset that follows.
                </p>
              </div>
              <div className="flex flex-wrap gap-2">
                {loop.map((step, index) => (
                  <span
                    key={step}
                    className={`rounded-full px-3 py-1.5 text-sm font-medium ${
                      index === 1 || index === 3
                        ? 'bg-teal/15 text-teal-bright'
                        : 'border border-white/10 text-text-muted'
                    }`}
                  >
                    {step}
                  </span>
                ))}
              </div>
            </div>
          </div>
        </section>

        <section id="verticals" className="mx-auto max-w-7xl px-4 py-16 sm:px-6 lg:px-8">
          <SectionIntro
            title="One canon loop, several creative workflows."
            body="The core stays domain-neutral: references, canon versions, derivation, review, search, collections, and export. Verticals add presets and constraints without changing the workflow."
          />
          <div className="grid gap-4 lg:grid-cols-4">
            {verticals.map(({ title, body, icon: Icon, accent, span }) => (
              <article
                key={title}
                className={`glass rounded-[20px] p-5 transition-transform duration-300 ease-out will-change-transform hover:-translate-y-1 ${span}`}
              >
                <div
                  className={`mb-5 grid size-10 place-items-center rounded-[12px] ${
                    accent === 'teal' ? 'bg-teal/15 text-teal-bright' : 'bg-indigo/20 text-indigo-bright'
                  }`}
                >
                  <Icon size={21} weight="fill" />
                </div>
                <h3 className="text-lg font-semibold tracking-tight text-text">{title}</h3>
                <p className="mt-2 max-w-xl text-sm leading-6 text-text-muted">{body}</p>
              </article>
            ))}
          </div>
        </section>

        <section id="export" className="mx-auto max-w-7xl px-4 py-16 sm:px-6 lg:px-8">
          <div className="grid gap-6 lg:grid-cols-[0.85fr_1.15fr]">
            <div className="flex flex-col justify-center">
              <h2 className="text-balance text-3xl font-semibold tracking-[-0.025em] text-text sm:text-4xl">
                Built for teams that need the other 200 assets.
              </h2>
              <p className="mt-4 max-w-2xl text-pretty text-base leading-7 text-text-muted">
                CanonForge makes consistency inspectable: what an asset came from, which canon version shaped it, whether it passed review, and where it belongs in a production pack.
              </p>
              <div className="mt-7">
                <Link
                  to="/login?mode=signup"
                  className="group inline-flex min-h-12 items-center justify-center gap-2 rounded-[12px] bg-teal px-5 py-3 text-sm font-semibold text-bg transition hover:brightness-105 active:translate-y-px"
                >
                  Create your first canon
                  <ArrowRightIcon
                    size={16}
                    weight="bold"
                    className="transition-transform duration-200 ease-out group-hover:translate-x-0.5"
                  />
                </Link>
              </div>
            </div>

            <div className="grid gap-3">
              {/* Provenance-style rows — a stacked fact list, deliberately not a
                  second icon-card grid (that's the verticals section above). */}
              {proof.map(({ title, body }) => (
                <article
                  key={title}
                  className="rounded-[18px] border border-white/8 bg-white/[0.03] px-5 py-4 transition-colors duration-200 hover:border-white/15 hover:bg-white/[0.05]"
                >
                  <h3 className="text-sm font-semibold text-text">{title}</h3>
                  <p className="mt-1 text-sm leading-6 text-text-muted">{body}</p>
                </article>
              ))}
              <article className="glass rounded-[18px] p-4">
                <div className="flex flex-wrap items-center gap-3">
                  <span className="grid size-10 place-items-center rounded-[12px] bg-indigo/20 text-indigo-bright">
                    <MusicNotesIcon size={20} weight="fill" />
                  </span>
                  <div className="min-w-0 flex-1">
                    <h3 className="font-semibold text-text">Image today, audio in the same asset library.</h3>
                    <p className="mt-1 text-sm leading-6 text-text-muted">
                      SFX and loops use the same review, organization, and storage model as generated visuals.
                    </p>
                  </div>
                </div>
              </article>
            </div>
          </div>
        </section>
      </main>

      <footer className="relative z-10 mx-auto flex max-w-7xl flex-col gap-4 px-4 py-8 text-sm text-text-dim sm:flex-row sm:items-center sm:justify-between sm:px-6 lg:px-8">
        <p>CanonForge remembers your art direction.</p>
        <div className="flex gap-4">
          <Link to="/login?mode=login" className="transition hover:text-text">
            Sign in
          </Link>
          <Link to="/login?mode=signup" className="font-medium text-teal-bright transition hover:text-teal">
            Create workspace
          </Link>
        </div>
      </footer>
    </div>
  )
}
