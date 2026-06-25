#!/usr/bin/env node
// Reference-derivation spike — see ../../PLAN.md §8.
//
// Tests the CORE mechanism of the product (every vertical): take ONE base asset
// and generate consistent derivatives that preserve its identity + style.
//   base (upload base.png, or generate once from canon.json)
//     -> recolor, new pose, action pose, matching set member
//
// SAFE BY DEFAULT: dry-run prints the plan and spends nothing.
// Real generation only with RUN=1 (and an OpenRouter key). Bills per image
// (~$0.04 on google/gemini-2.5-flash-image, which is strong at reference-
// consistent editing). Resolution doesn't change price; COUNT is the lever.
//
// Usage:
//   node run.mjs            # dry-run: write plan, no spend
//   RUN=1 node run.mjs      # real: ~4-5 images into ./out
//   (drop your own ./base.png first for a far more honest test)

import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url))
const OUT = join(HERE, 'out')
const BASE_PNG = join(HERE, 'base.png')
const REPO_ENV = join(HERE, '..', '..', '.env')

const ENDPOINT = 'https://openrouter.ai/api/v1/chat/completions'
const MAX_IMAGES = 8
const DRY_RUN = process.env.RUN !== '1'

function fromDotenv(key) {
  try {
    for (const line of readFileSync(REPO_ENV, 'utf8').split('\n')) {
      const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*(.*)\s*$/)
      if (m && m[1] === key) return m[2].replace(/^["']|["']$/g, '').trim()
    }
  } catch { /* no .env */ }
  return undefined
}
const API_KEY = process.env.OPENROUTER_API_KEY || fromDotenv('OPENROUTER_API_KEY')
const MODEL =
  process.env.OPENROUTER_IMAGE_MODEL ||
  fromDotenv('OPENROUTER_IMAGE_MODEL') ||
  'google/gemini-2.5-flash-image'

const canon = JSON.parse(readFileSync(join(HERE, 'canon.json'), 'utf8'))
const derivations = JSON.parse(readFileSync(join(HERE, 'derivations.json'), 'utf8'))

const styleLine = Object.values(canon.style).join(', ')
const negLine = canon.negative.join('; ')
const basePrompt =
  `A single ${canon.base.role} game asset: ${canon.base.subject}. ` +
  `Art style: ${styleLine}. Must NOT include: ${negLine}.`
const derivePrompt = (d) =>
  `${d.instruction} Maintain this exact art style: ${styleLine}. ` +
  `One centered isolated asset, transparent background. Must NOT include: ${negLine}.`

const haveBase = existsSync(BASE_PNG)
const willGenBase = !haveBase
const total = derivations.length + (willGenBase ? 1 : 0)

console.log(`Canon: ${canon.name}`)
console.log(`Base: ${haveBase ? 'using your base.png' : 'will generate from canon.json (no base.png found)'}`)
console.log(`Derivations: ${derivations.map((d) => d.id).join(', ')}`)
console.log(`Total images: ${total}  (~$${(total * 0.04).toFixed(2)} on ${MODEL})`)

mkdirSync(OUT, { recursive: true })
writeFileSync(
  join(OUT, 'plan.md'),
  `# Spike plan\n\n## Base ${haveBase ? '(your base.png)' : '(generated)'}\n\n${willGenBase ? basePrompt : '(uploaded base.png)'}\n\n` +
    derivations.map((d) => `## derive: ${d.id}\n\n${derivePrompt(d)}\n`).join('\n'),
)

if (DRY_RUN) {
  console.log(`\nDRY RUN — wrote out/plan.md. No images, $0 spent.`)
  console.log(`Optional: drop your own base.png here first. Then:  RUN=1 node run.mjs`)
  process.exit(0)
}

if (!API_KEY) {
  console.error('RUN=1 but no OPENROUTER_API_KEY (env or ../../.env). Aborting, $0 spent.')
  process.exit(1)
}
if (total > MAX_IMAGES) {
  console.error(`Refusing: ${total} images exceeds MAX_IMAGES=${MAX_IMAGES}.`)
  process.exit(1)
}

// content is a string (text-only, for seeding the base) or an array with a
// reference image (for derivations).
async function generate(content) {
  const res = await fetch(ENDPOINT, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${API_KEY}`,
      'content-type': 'application/json',
      'x-title': 'CanonForge Spike',
    },
    body: JSON.stringify({
      model: MODEL,
      messages: [{ role: 'user', content }],
      modalities: ['image', 'text'],
    }),
  })
  if (!res.ok) throw new Error(`HTTP ${res.status}: ${(await res.text()).slice(0, 200)}`)
  const data = await res.json()
  if (data.error) throw new Error(data.error.message || 'generation error')
  const url = data?.choices?.[0]?.message?.images?.[0]?.image_url?.url
  if (!url) throw new Error('no image in response')
  return Buffer.from(url.split(',')[1], 'base64')
}

const refPart = (buf) => ({
  type: 'image_url',
  image_url: { url: `data:image/png;base64,${buf.toString('base64')}` },
})

// 1) resolve the base
let baseBuf
if (haveBase) {
  baseBuf = readFileSync(BASE_PNG)
  console.log('using your base.png as reference')
} else {
  process.stdout.write('generating base ... ')
  baseBuf = await generate(basePrompt)
  writeFileSync(BASE_PNG, baseBuf)
  writeFileSync(join(OUT, 'base.png'), baseBuf)
  console.log(`ok (${(baseBuf.length / 1024).toFixed(0)} KB)`)
}
if (!existsSync(join(OUT, 'base.png'))) writeFileSync(join(OUT, 'base.png'), baseBuf)

// 2) derive from the base (reference-conditioned)
const cells = []
let spent = willGenBase ? 0.04 : 0
for (const d of derivations) {
  process.stdout.write(`deriving ${d.id} ... `)
  try {
    const buf = await generate([{ type: 'text', text: derivePrompt(d) }, refPart(baseBuf)])
    writeFileSync(join(OUT, `${d.id}.png`), buf)
    spent += 0.04
    console.log(`ok (${(buf.length / 1024).toFixed(0)} KB)`)
    cells.push({ id: d.id, src: `${d.id}.png` })
  } catch (e) {
    console.log(`FAILED: ${e.message}`)
    cells.push({ id: d.id, err: e.message })
  }
}

// 3) contact sheet: base alongside derivatives → eyeball identity preservation
const tiles = [{ id: 'BASE', src: 'base.png' }, ...cells]
  .map(
    (c) =>
      `<figure>${c.src ? `<img src="${c.src}">` : `<div class="fail">failed</div>`}<figcaption>${c.id}</figcaption></figure>`,
  )
  .join('')
writeFileSync(
  join(OUT, 'index.html'),
  `<!doctype html><meta charset=utf8><title>Reference derivation</title>
<style>body{background:#111;color:#eee;font:14px system-ui;padding:24px}
.grid{display:flex;flex-wrap:wrap;gap:16px}figure{margin:0;text-align:center}
img{width:180px;height:180px;object-fit:contain;background:#0a0a0a;border-radius:8px;border:1px solid #333}
figure:first-child img{border-color:#5a7}figcaption{color:#9aa0ad;font-size:12px;margin-top:6px}
.fail{width:180px;height:180px;display:grid;place-items:center;color:#a55;border:1px solid #533;border-radius:8px}</style>
<h2>Reference-derivation spike — ${canon.name}</h2>
<p>Does each derivative keep the BASE's identity + style? (base outlined in green)</p>
<div class="grid">${tiles}</div>`,
)

console.log(`\nDone. ~$${spent.toFixed(2)} spent. Open out/index.html to compare against the base.`)
