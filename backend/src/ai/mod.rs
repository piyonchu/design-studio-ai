//! AI integration. Image generation (and, later, reference-conditioned
//! derivation) lives behind this module so the transport can be swapped.
//!
//! The earlier text-DSL generation client (Anthropic Messages API for the
//! UI-as-Code pipeline) was removed in the pivot to a visual asset studio.

pub mod audio;
pub mod embeddings;
pub mod images;
