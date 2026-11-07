//! Shared error type for the binary command buffer system.
//!
//! The command dispatch logic lives in `satteri_plugin_api::js_commands`.

use std::fmt;

use satteri_arena::{ArenaKind, Document, Hast, Mdast};

use crate::hast::HastNodeType;
use crate::hast::codec::decode_element_tag;
use crate::mdast::MdastNodeType;

/// A node as a plugin author finds it in the document: its type, and where it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeLabel {
    pub id: u32,
    /// The node's type, or `<tag>` for an HTML element.
    pub node_type: String,
    /// 1-based line and column, for a node read from the source.
    pub start: Option<(usize, usize)>,
}

impl NodeLabel {
    pub fn of<K: ArenaKind>(arena: &Document<'_, K>, id: u32) -> Self {
        let node = arena.get_node(id);
        let node_type = if K::KIND_TAG == Mdast::KIND_TAG {
            MdastNodeType::from_u8(node.node_type).map(|t| t.name().to_string())
        } else if K::KIND_TAG == Hast::KIND_TAG {
            HastNodeType::from_u8(node.node_type).map(|t| match t {
                HastNodeType::Element if arena.get_type_data(id).len() >= 8 => {
                    format!(
                        "<{}>",
                        arena.get_str(decode_element_tag(arena.get_type_data(id)))
                    )
                }
                _ => t.name().to_string(),
            })
        } else {
            None
        };
        // Offsets are recorded even when line and column tracking is off.
        let start = if node.start_line > 0 {
            Some((node.start_line as usize, node.start_column as usize))
        } else if node.end_offset > node.start_offset {
            arena
                .source()
                .get(..node.start_offset as usize)
                .map(|before| {
                    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
                    (
                        before.matches('\n').count() + 1,
                        before[line_start..].chars().count() + 1,
                    )
                })
        } else {
            None
        };
        NodeLabel {
            id,
            node_type: node_type.unwrap_or_else(|| "node".to_string()),
            start,
        }
    }
}

impl fmt::Display for NodeLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.start {
            Some((line, column)) => write!(f, "the {} at {line}:{column}", self.node_type),
            None => write!(f, "the {} node without a source position", self.node_type),
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
    UnknownField {
        node_type: String,
        name: String,
    },
    /// The property exists on the node type, but the supplied value's type is
    /// not one the field can hold (e.g. a string for `heading.depth`).
    InvalidPropertyValue {
        node_type: String,
        name: String,
    },
    /// A numeric property value is not an integer in the field's stored range.
    /// Erroring beats writing `value as u8`, which would silently mask bits.
    PropertyValueOutOfRange {
        node_type: String,
        name: String,
        value: String,
        max: u32,
    },
    /// A set-property value-type byte the dispatcher doesn't handle.
    InvalidPropertyValueType(u8),
    /// The node type exists but can never be built as plugin content (`root`
    /// is only the stream's top-level wrapper; `doctype` carries nothing the
    /// op-stream can express).
    UnencodableNodeType(&'static str),
    /// An op-stream nests OPENs deeper than the replay supports.
    OpstreamTooDeep(usize),
    /// A patch has nowhere to land: its anchor lives inside a removed subtree,
    /// or it edits a node that is removed, or replaced by content that cannot take the edit.
    PatchOnRemovedSubtree(NodeLabel),
    /// Payloads place or arrange the same node twice; a node can only be in one place.
    NodePlacedTwice(NodeLabel),
    /// Content placed into or next to a node, or replacing the root, contains that node.
    NodeInsideItself(NodeLabel),
    /// Content placed into or next to `node` contains one of its ancestors.
    AncestorPlacedInside {
        ancestor: NodeLabel,
        node: NodeLabel,
    },
    /// Edits to two nodes each place the other, so neither can settle first.
    CircularPlacement(NodeLabel, NodeLabel),
    /// An op-stream's `OP_OPEN`s and `OP_CLOSE`s did not pair up.
    UnbalancedOpstream,
    /// A document op-stream is not the one `root` wrapping the tree that
    /// rendering starts from.
    MissingDocumentRoot,
    /// A wire-supplied node id does not exist in the target arena.
    InvalidNodeId(u32),
    /// A stored node's `type_data` is shorter than its declared layout, so a
    /// field write would spill into the next node's data.
    TypeDataTooShort,
    /// A structurally valid but re-entrant patch shape (e.g. a payload ref
    /// naming its own anchor's ancestor) that in-place application rejects.
    UnsupportedPatchShape(&'static str),
    /// A `wrapNode` `{raw}` payload with no usable wrapper; the string
    /// says why.
    InvalidRawWrapper(String),
    /// A void element (the tag) as the wrapper would render without children,
    /// dropping the wrapped node.
    VoidWrapParent(String),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "unexpected end of command buffer"),
            Self::UnsupportedPatchShape(reason) => {
                write!(f, "unsupported patch shape: {reason}")
            }
            Self::UnknownCommand(c) => write!(f, "unknown command byte: 0x{c:02x}"),
            Self::UnknownPayloadType(p) => write!(f, "unknown payload type: 0x{p:02x}"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in command buffer"),
            Self::InvalidJson(e) => write!(f, "invalid JSON in command payload: {e}"),
            Self::UnknownNodeType(t) => write!(f, "unknown node type: {t}"),
            Self::UnknownField { node_type, name } => {
                write!(f, "cannot set property '{name}' on a '{node_type}' node")
            }
            Self::InvalidPropertyValue { node_type, name } => write!(
                f,
                "property '{name}' on a '{node_type}' node cannot hold a value of this type"
            ),
            Self::PropertyValueOutOfRange {
                node_type,
                name,
                value,
                max,
            } => write!(
                f,
                "value {value} for property '{name}' on a '{node_type}' node must be an integer between 0 and {max}"
            ),
            Self::InvalidPropertyValueType(t) => {
                write!(f, "unknown set-property value type: 0x{t:02x}")
            }
            Self::UnencodableNodeType(t) => {
                write!(f, "node type '{t}' cannot appear in plugin-built content")
            }
            Self::OpstreamTooDeep(max) => {
                write!(f, "content nests deeper than {max} levels")
            }
            Self::UnbalancedOpstream => {
                write!(f, "unbalanced op-stream: OPEN and CLOSE ops do not pair up")
            }
            Self::MissingDocumentRoot => {
                write!(f, "op-stream document must be one root wrapping the tree")
            }
            Self::InvalidNodeId(id) => {
                write!(
                    f,
                    "invalid node id: {id} does not exist in the target arena"
                )
            }
            Self::TypeDataTooShort => {
                write!(
                    f,
                    "stored node type_data is shorter than its layout requires"
                )
            }
            Self::NodePlacedTwice(node) => write!(
                f,
                "{node} is placed in two places, but a node can only be in one; place structuredClone(node) where you want a copy"
            ),
            Self::NodeInsideItself(node) => write!(
                f,
                "content placed next to or into {node} contains that same node; place structuredClone(node) there for a copy"
            ),
            Self::AncestorPlacedInside { ancestor, node } => write!(
                f,
                "{ancestor} is placed next to or inside {node}, which it contains"
            ),
            Self::CircularPlacement(a, b) => write!(
                f,
                "the edits to {a} and {b} each place the other, so neither can go first; to reorder siblings, set their parent's children instead"
            ),
            Self::PatchOnRemovedSubtree(node) => write!(
                f,
                "an edit to {node} has nowhere to land: the node, or a subtree holding it, is removed or replaced"
            ),
            Self::InvalidRawWrapper(reason) => {
                write!(f, "wrapNode: {{raw}} wrapper {reason}")
            }
            Self::VoidWrapParent(tag) => write!(
                f,
                "wrapNode: <{tag}> is a void element, which cannot hold the wrapped node"
            ),
        }
    }
}

impl std::error::Error for CommandError {}
