# Reference-derivation spike

Throwaway experiment to de-risk the **core mechanism** of the product before the rewrite. Full rationale in [PLAN.md §8](../../PLAN.md).

**Question:** can we take **one base asset** and generate *consistent derivatives* that preserve its identity + style? This is the shared mechanism behind every vertical (game sprites, manhwa characters, illustration sets, marketing imagery) — if reference conditioning isn't strong enough, that's the whole product.

## What it does

Resolves a **base** (your `base.png`, or one generated from `canon.json`), then derives a set from it — **recolor, new pose, action pose, matching set member** (`derivations.json`) — each conditioned on the base image. Writes a contact sheet with the base alongside the derivatives.

## Run it

```bash
# dry-run (default): writes out/plan.md, spends nothing
node run.mjs

# real run: ~4-5 images (~$0.20), needs an OpenRouter key
RUN=1 node run.mjs
```

Drop your **own `base.png`** in this folder first for a far more honest test (then it derives from yours instead of a generated one). Key is read from `OPENROUTER_API_KEY` or the repo root `.env`. Open `out/index.html` after.

## Cost

Bills **per image** (~$0.04 on `google/gemini-2.5-flash-image` — chosen because it's strong at reference-consistent editing). Resolution does **not** change price; the lever is **count**. `MAX_IMAGES=8` is a hard guard. Shared key budget is small (~$9.68) — one run is ~$0.20.

## Reading the result (decision gate)

In `out/index.html`, compare each derivative to the base:
- ✅ Identity + style clearly preserved across derivatives → reference conditioning holds; proceed to build.
- ⚠️ Identity drifts → try stronger conditioning (multiple references, IP-adapter-style, or a per-character fine-tune) and re-test before committing the rewrite.

**Animation** (multi-frame cycles) is deliberately **not** in this spike — it's the hardest case and needs its own test. This proves the reliable derivations (recolor/pose/variant/set) first.

`out/` and `base.png` are gitignored (throwaway).
