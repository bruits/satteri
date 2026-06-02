//! Shared types for the binary command buffer system.
//!
//! The command dispatch logic lives in `satteri_plugin_api::js_commands`.
//! This module defines the shared error type, JS node representation,
//! and property-type constants that are used by both MDAST and HAST
//! command handlers.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct JsNode {
    // Defaulted so a bare reference node (`{ "_ref": N }`, no `type`)
    // deserializes; `ref_id` is checked before `node_type` is ever used.
    #[serde(rename = "type", default)]
    pub node_type: String,
    #[serde(default)]
    pub children: Option<Vec<JsNode>>,
    pub depth: Option<u8>,
    pub value: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub alt: Option<String>,
    pub lang: Option<String>,
    pub meta: Option<String>,
    pub ordered: Option<bool>,
    pub start: Option<u32>,
    pub spread: Option<bool>,
    pub checked: Option<bool>,
    pub identifier: Option<String>,
    pub label: Option<String>,
    #[serde(rename = "referenceType")]
    pub reference_type: Option<String>,
    pub name: Option<String>,
    #[serde(default)]
    pub attributes: Option<JsNodeAttributes>,
    // HAST-specific fields
    #[serde(rename = "tagName")]
    pub tag_name: Option<String>,
    #[serde(default)]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,
    /// Marker: when true, this node is a HAST node (not MDAST).
    #[serde(rename = "_hast", default)]
    pub is_hast: bool,
    /// When true, keep the original node's children instead of replacing them.
    #[serde(rename = "_keepChildren", default)]
    pub keep_children: bool,
    /// Reference to an existing original node id. When set, this node is a
    /// placeholder: the rebuild splices that original node's subtree in place
    /// (preserving its id and applying any pending patch on it) instead of
    /// building a fresh node. Lets a returned replacement pass through existing
    /// children at any depth without stranding patches queued on them.
    #[serde(rename = "_ref", default)]
    pub ref_id: Option<u32>,
    /// On a fresh node, equivalent to `ctx.setProperty(node, "data", …)`.
    #[serde(default)]
    pub data: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum JsNodeAttribute {
    #[serde(rename = "mdxJsxAttribute")]
    Attribute {
        name: String,
        /// null → boolean, string → literal, object with `value` → expression
        value: Option<serde_json::Value>,
    },
    #[serde(rename = "mdxJsxExpressionAttribute")]
    ExpressionAttribute { value: String },
}

/// `attributes` on a JS node has two shapes depending on the node type:
/// MDX JSX elements use a list of `{type, name, value}` records, while
/// directive nodes (`containerDirective` / `leafDirective` / `textDirective`)
/// use a flat `{key: stringValue}` map. Untagged so serde tries the array
/// shape first and falls back to the map.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum JsNodeAttributes {
    Jsx(Vec<JsNodeAttribute>),
    Directive(serde_json::Map<String, serde_json::Value>),
}

impl JsNodeAttributes {
    pub fn as_jsx(&self) -> Option<&[JsNodeAttribute]> {
        match self {
            Self::Jsx(v) => Some(v.as_slice()),
            Self::Directive(_) => None,
        }
    }

    pub fn as_directive(&self) -> Option<&serde_json::Map<String, serde_json::Value>> {
        match self {
            Self::Directive(m) => Some(m),
            Self::Jsx(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum CommandError {
    UnexpectedEof,
    UnknownCommand(u8),
    UnknownPayloadType(u8),
    InvalidUtf8,
    InvalidJson(String),
    UnknownNodeType(String),
    UnknownField(u16),
    /// `wrapNode` was issued against a node that is also removed in the same
    /// command buffer. There's no defined way to "wrap then remove" or
    /// "remove then wrap" the same anchor.
    WrapOnRemovedNode(u32),
    /// `prependChild` or `appendChild` was issued against a node that is
    /// also removed. The removed node has no inside to receive children.
    ChildPatchOnRemovedNode(u32),
    /// A patch's anchor lives inside a subtree whose root was removed, so
    /// the patch can never apply.
    PatchOnRemovedSubtree(u32),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "unexpected end of command buffer"),
            Self::UnknownCommand(c) => write!(f, "unknown command byte: 0x{c:02x}"),
            Self::UnknownPayloadType(p) => write!(f, "unknown payload type: 0x{p:02x}"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in command buffer"),
            Self::InvalidJson(e) => write!(f, "invalid JSON in command payload: {e}"),
            Self::UnknownNodeType(t) => write!(f, "unknown node type in JSON: {t}"),
            Self::UnknownField(f_id) => write!(f, "unknown field ID: 0x{f_id:04x}"),
            Self::WrapOnRemovedNode(id) => {
                write!(f, "wrapNode targets node {id} which is also removed")
            }
            Self::ChildPatchOnRemovedNode(id) => write!(
                f,
                "prependChild/appendChild targets node {id} which is also removed"
            ),
            Self::PatchOnRemovedSubtree(id) => {
                write!(f, "patch targets node {id} inside a removed subtree")
            }
        }
    }
}

impl std::error::Error for CommandError {}
