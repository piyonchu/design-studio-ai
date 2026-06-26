//! Engine export packers — the implementation half of the per-vertical export
//! adapter hook (`crate::verticals::Engine`). A vertical declares which engine
//! it can target; the export route ([`crate::routes::export`]) dispatches to the
//! matching packer here to turn the generic, vertical-neutral pack (the
//! role/tag `groups[]` + decoded asset bytes) into an engine-import-ready bundle.
//!
//! Adding an engine target = a `verticals::Engine` variant + a packer module.

pub mod godot;
pub mod unity;

/// A generated text file to drop into an export zip at `path`.
pub struct TextFile {
    pub path: String,
    pub contents: String,
}
