# CanonForge → Pro Asset Pipeline — Plan

Direction reset after senior review (2026-06-30). The product currently *reads*
like an AI-gen playground; the moat and the audience are **pro / small studios**
(indie game teams, webtoon studios, brand teams). This plan reframes CanonForge
as a **production pipeline for AI-generated art** — *manage a consistent,
versioned, reviewable body of work*, where generation is one step inside the
workflow, not the headline.

> Source-of-truth note: [PLAN.md](PLAN.md) = original vision, [ROADMAP.md](ROADMAP.md) = shipped status,
> [HANDOFF.md](HANDOFF.md) = setup. **This file = the pro-pipeline forward plan.**

## 1. The thesis (what changes)

| From (playground) | To (pro pipeline) |
|---|---|
| Big "generate" box is the hero | The **library** (folders + versions + review) is the hero; generate is a step |
| One-off images | A **managed, versioned body of work** bound to a canon |
| "Look, AI made a picture" | "Here's how we keep 500 assets on-style, reviewed, and shippable" |
| Flat collections | **Folder tree** (home) + collections (cross-cutting sets) |
| Re-prompt to change anything | **Localized, non-destructive, versioned edits** |
| Workspace-wide roles | **Per-project roles + a reviewer gate** |

**Pitch:** *"Git for AI art assets — bring a reference, derive a consistent set
bound to your canon, version and review every asset, edit without re-rolling,
and export engine-ready."*

## 2. Design decisions (locked 2026-06-30)

1. **Per-asset versioning → a dedicated `asset_versions` table** with a `head`
   pointer on `assets`. Git-like history / rollback / diff. (Not link-edges.)
2. **Permissions → per-project role overrides + a `reviewer` gate**, layered on
   the existing workspace roles. (Not full per-folder ACLs — too heavy for a
   small-team product; revisit later.)
3. **Masked/inpaint editing → design the UX + a provider seam now, defer the
   real provider.** Mock it (like the image/audio boundary); wire fal.ai or
   Replicate when ready (new key + per-edit spend). Deterministic edits ship
   without any provider.
4. **Folders coexist with Collections.** Folder = an asset's canonical home (one
   tree, like files). Collection = a cross-cutting curated set (many-to-many,
   like labels/playlists). Export-from-collection stays.

## 3. Phased plan

Effort key: **S** ≈ ½–1 day · **M** ≈ 1–3 days · **L** ≈ multi-day. "Spend" =
shared OpenRouter / new-provider cost.

---

### Phase A — Asset management as the hero  *(answers the senior; mostly no spend)*

#### A1. Folder tree  *(M, no spend)*
- **Migration** `folders`: `id, project_id, parent_id (nullable FK self), name, created_at`; add `assets.folder_id` (nullable FK). Root = `folder_id IS NULL` or a synthetic root.
- **API**: `GET/POST /projects/:id/folders` (list tree, create), `PATCH /folders/:id` (rename/move), `DELETE /folders/:id` (only if empty, or cascade to trash), `PATCH /assets/:id` gains `folder_id` (move asset).
- **Frontend**: a collapsible tree in the board's left rail (Characters / Props / UI / Tiles…); drag-or-menu move; breadcrumb. The board lists the selected folder (+ "all"). Keep the existing filter rail.
- **Backfill**: existing assets → `folder_id NULL` (root). No data loss.

#### A2. Per-asset version history + rollback + diff  *(L, no spend — the headline)*
The big one. Moves an asset's *bytes* out of `assets` and into versions.
- **Migration** `asset_versions`: `id, asset_id (FK), version (int, per-asset seq), s3_key, mime_type, prompt, change_note, created_by (FK users), created_at`. Add `assets.current_version_id (FK asset_versions)`. Move the embedding link to the version (or re-embed head on change).
- **Write path**: generate/derive/upload/audio insert the `asset` **and** its `v1` (bytes go to the version). `assets.current_version_id` → v1. Every edit (Phase B) or regenerate appends `v(n+1)` and advances head, with a `change_note`.
- **Backfill**: for each existing asset, create a `v1` from its current `s3_key`/`mime_type`; set `current_version_id`. (Reuses the same storage pointers — no byte copying.)
- **Read path**: `with_url` / file route resolve the **head** version's bytes (or `?version=` for a specific one). The board/list is unchanged from the client's view.
- **API**: `GET /assets/:id/versions` (history + notes + authors), `POST /assets/:id/versions/:vid/restore` (rollback = set head, or append a copy), `GET /assets/:id/file?version=:vid`.
- **Frontend**: a **History panel** in the inspector (version list, change notes, author, time); **rollback** button; a **before/after visual diff** (slider or side-by-side) between any two versions + a metadata diff (prompt/canon-version changed).
- **Why headline:** the senior literally asked for "versioning of each pic." This is what makes it read as a pro system, not a gallery.

#### A3. Reposition the UI + landing  *(S–M, no spend)*
- Make the **library/folders/review the first thing** you see in a project, not the generate box (generate becomes a clear action, not the centerpiece).
- Rewrite landing + in-app copy to the pro-pipeline framing (PRODUCT.md/DESIGN.md aligned). Emphasize: canon → consistent set → version/review → export.

---

### Phase B — Editing: change one thing without re-prompting  *(makes it a tool, not a toy)*

#### B1. Deterministic edits  *(M, FREE, instant, reversible)*
No model. Each edit creates a **new version** (A2), so it's non-destructive.
- Ops: **recolor / palette-swap**, **crop**, **resize**, **flip/rotate**, **background-removal** (e.g. a bundled `rembg`-style or simple chroma/alpha for clean BGs), **format convert** (png/webp/jpg).
- **Backend**: an `edit` module operating on bytes (uses the `image` crate already in deps for crop/resize/flip/convert; recolor = palette map; bg-remove = simplest viable). `POST /assets/:id/edit/<op>` → new version.
- **Frontend**: an Edit tab in the inspector with these tools; live preview; "Save as new version."
- **Why:** pros do these constantly; free + instant + versioned = obviously professional.

#### B2. Masked / inpaint edit — seam now, provider later  *(M for seam; L + spend when wired)*
- **UX**: open an asset → brush a mask over a region → type what changes ("red hat", "remove the extra finger") → regenerate **only the masked region** → new version.
- **Backend seam**: `ai::edit::inpaint(bytes, mask, prompt) -> bytes` behind a boundary like `ai::images`, with `EDIT_MOCK=true` default (returns the original or a stub) so dev/CI/demo are free. `POST /assets/:id/inpaint`.
- **Real provider (deferred, decision #3)**: fal.ai (`fast-sdxl`/inpaint) or Replicate SD-inpaint — new key + ~$0.01–0.05/edit. Flip `EDIT_MOCK=false`.
- **Why:** "tweak without re-rolling the whole image" is the single feature that most separates a tool from a prompt box.

---

### Phase C — Permissions: per-project roles + reviewer gate  *(M, no spend)*
- **Migration** `project_members`: `project_id, user_id, role (viewer|editor|reviewer|owner)`. Absent → fall back to the workspace role.
- **Reviewer gate**: status transitions to **approved** require `reviewer`+; editors can submit `needs_review` but not self-approve. (Approval is what feeds the exemplar/canon moat, so gating it matters.)
- **Backend**: extend `require_project_access` to check `project_members` override first, then workspace role; add the reviewer check on the approve path.
- **Frontend**: the Team page (built) gains **per-project** assignment + a Reviewer role; the Review queue shows who may approve.

---

### Phase D — Consistency depth (the deep moat)  *(spend; later)*
- **D1. Smartest-exemplar pick / multi-reference** *(S–M, no new infra)* — choose the approved exemplar whose embedding best matches the prompt (uses existing dual embeddings), and/or pass several reference images to the multimodal model. Cheap, immediate quality lift over "newest exemplar wins."
- **D2. ControlNet / pose** *(L, spend, new provider)* — give a skeleton → exact posture; pairs with animation frames. fal/Replicate behind the image seam.
- **D3. Per-project LoRA** *(L, spend + training infra)* — train a small model on the project's *approved* assets → a model that **is** your style. The deepest, least-copyable moat; the true evolution of the exemplar loop. fal/Replicate LoRA training; store a LoRA per project; generation uses it.

---

## 4. Sequencing & the two that flip perception

Build order: **A1 → A2 → A3 → B1 → C → B2 → D1 → (D2/D3 when spend is greenlit).**

If only two things get done, do **A2 (per-asset versioning + diff)** and **B2/B1
(edit without re-prompt)** — together they say "professional pipeline" louder
than anything else and directly answer the senior.

## 5. Risks / notes
- **A2 is a real refactor** — moving bytes from `assets` to `asset_versions`
  touches generate/derive/upload/audio + the file route + a backfill. Do it on
  its own branch with the integration test green before/after. Highest-value,
  highest-care item.
- Keep **mock-first** everywhere (deterministic edits free; inpaint mocked;
  LoRA/ControlNet gated on explicit spend go-ahead).
- Storage: pairs with the **R2 upgrade** (see [DEPLOY.md](DEPLOY.md)) — versioning multiplies
  stored bytes, so object storage matters more once A2 lands.
- Don't break the engine-export / canon / exemplar systems — versioning should
  feed them (export the *head* version; exemplar = an approved asset's head).

## 6. Success criteria
- A project reads as a **structured, versioned asset library** on first open — not a generate box.
- Any asset shows its **history**, supports **rollback** and a **visual diff**.
- A user can **change one region** of an asset and keep the original as a prior version.
- **Reviewers** gate approvals; **per-project** access works.
- The pitch survives the question *"isn't this just calling an API?"* — because versioning, review, permissions, localized editing, and (later) a per-project trained model are the product; the API is one commodity step.

## 7. Open questions (none blocking; defaults chosen)
- Inpaint provider when wired: **fal.ai vs Replicate** (cost/quality/latency) — decide at B2 wiring time.
- LoRA: per-project vs per-character granularity — decide at D3.
- Folder delete semantics: block-if-nonempty vs cascade-to-trash — proposing **cascade to Trash** (reuses soft-delete).
