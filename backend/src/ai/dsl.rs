//! UI-as-Code DSL: typed representations of each artifact kind's `content`.
//!
//! Structural artifacts are JSON trees; the AI edits the JSON, never pixels.
//! `validate` is the gate for both AI output and (future) manual writes —
//! content that doesn't deserialize into the kind's type is rejected.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::ArtifactKind;

// ── Per-kind content shapes ──────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Idea {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserFlow {
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DesignSystem {
    /// Free-form token tree (colors, typography, spacing, …). Tightened later.
    pub tokens: Value,
}

/// A node in a wireframe / UI-screen tree. Recursive — which is exactly why we
/// can't use the API's strict json_schema mode and validate by deserialization.
#[derive(Debug, Serialize, Deserialize)]
pub struct Element {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub props: Value,
    #[serde(default)]
    pub children: Vec<Element>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Screen {
    pub root: Element,
}

/// Validate that `content` is a well-formed DSL tree for `kind`.
/// Returns a human-readable error (fed back to the model on retry).
pub fn validate(kind: ArtifactKind, content: &Value) -> Result<(), String> {
    fn check<T: for<'de> Deserialize<'de>>(content: &Value) -> Result<(), String> {
        serde_json::from_value::<T>(content.clone())
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    match kind {
        ArtifactKind::Idea => check::<Idea>(content),
        ArtifactKind::UserFlow => check::<UserFlow>(content),
        ArtifactKind::DesignSystem => check::<DesignSystem>(content),
        ArtifactKind::Wireframe | ArtifactKind::UiScreen => check::<Screen>(content),
    }
}

/// The DSL shape for `kind`, injected into the system prompt so the model
/// returns JSON we can deserialize.
pub fn dsl_spec(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Idea => r#"{ "text": "<concise product idea>" }"#,
        ArtifactKind::UserFlow => {
            r#"{ "nodes": [{ "id": "n1", "label": "Step", "kind": "screen|decision|action" }],
  "edges": [{ "from": "n1", "to": "n2", "label": "optional" }] }"#
        }
        ArtifactKind::DesignSystem => {
            r##"{ "tokens": { "colors": { "primary": "#2563eb" }, "typography": { "body": "16px/1.5 Inter" }, "spacing": { "md": 16 } } }"##
        }
        ArtifactKind::Wireframe | ArtifactKind::UiScreen => {
            r#"{ "root": { "id": "root", "type": "frame", "props": {}, "children": [
    { "id": "title", "type": "text", "props": { "text": "Heading" }, "children": [] },
    { "id": "cta", "type": "button", "props": { "label": "Continue" }, "children": [] }
  ] } }"#
        }
    }
}

/// A canned, schema-valid DSL value used in `AI_MOCK` mode (no network).
pub fn mock_dsl(kind: ArtifactKind, prompt: &str) -> Value {
    match kind {
        ArtifactKind::Idea => json!({ "text": format!("Idea: {prompt}") }),
        ArtifactKind::UserFlow => json!({
            "nodes": [
                { "id": "start", "label": "Start", "kind": "screen" },
                { "id": "input", "label": "Enter details", "kind": "screen" },
                { "id": "done", "label": "Success", "kind": "screen" }
            ],
            "edges": [
                { "from": "start", "to": "input" },
                { "from": "input", "to": "done", "label": "submit" }
            ]
        }),
        ArtifactKind::DesignSystem => json!({
            "tokens": {
                "colors": { "primary": "#2563eb", "bg": "#ffffff", "text": "#111827" },
                "typography": { "body": "16px/1.5 Inter", "heading": "24px/1.3 Inter" },
                "spacing": { "sm": 8, "md": 16, "lg": 24 }
            }
        }),
        ArtifactKind::Wireframe | ArtifactKind::UiScreen => json!({
            "root": {
                "id": "root", "type": "frame", "props": { "note": prompt }, "children": [
                    { "id": "title", "type": "text", "props": { "text": "Heading" }, "children": [] },
                    { "id": "field", "type": "input", "props": { "placeholder": "Email" }, "children": [] },
                    { "id": "cta", "type": "button", "props": { "label": "Continue" }, "children": [] }
                ]
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_good_dsl_for_each_kind() {
        for kind in [
            ArtifactKind::Idea,
            ArtifactKind::UserFlow,
            ArtifactKind::DesignSystem,
            ArtifactKind::Wireframe,
            ArtifactKind::UiScreen,
        ] {
            let v = mock_dsl(kind, "test");
            assert!(validate(kind, &v).is_ok(), "{kind:?} mock should validate");
        }
    }

    #[test]
    fn rejects_malformed_dsl() {
        // UserFlow missing required `edges`
        let bad = json!({ "nodes": [] });
        assert!(validate(ArtifactKind::UserFlow, &bad).is_err());
        // Screen with a non-object root
        assert!(validate(ArtifactKind::Wireframe, &json!({ "root": 5 })).is_err());
        // Idea missing `text`
        assert!(validate(ArtifactKind::Idea, &json!({})).is_err());
    }

    #[test]
    fn recursive_element_tree_validates() {
        let nested = json!({ "root": {
            "id": "a", "type": "frame", "props": {}, "children": [
                { "id": "b", "type": "frame", "props": {}, "children": [
                    { "id": "c", "type": "text", "props": {}, "children": [] }
                ] }
            ]
        }});
        assert!(validate(ArtifactKind::UiScreen, &nested).is_ok());
    }
}
