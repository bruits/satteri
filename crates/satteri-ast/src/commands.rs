//! Shared error type for the binary command buffer system.
//!
//! The command dispatch logic lives in `satteri_plugin_api::js_commands`.

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
    /// An op-stream's `OP_OPEN`s and `OP_CLOSE`s did not pair up.
    UnbalancedOpstream,
    /// A wire-supplied node id does not exist in the target arena.
    InvalidNodeId(u32),
    /// A stored node's `type_data` is shorter than its declared layout, so a
    /// field write would spill into the next node's data.
    TypeDataTooShort,
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
            Self::UnknownField { node_type, name } => {
                write!(f, "cannot set property '{name}' on a '{node_type}' node")
            }
            Self::InvalidPropertyValue { node_type, name } => write!(
                f,
                "property '{name}' on a '{node_type}' node cannot hold a value of this type"
            ),
            Self::WrapOnRemovedNode(id) => {
                write!(f, "wrapNode targets node {id} which is also removed")
            }
            Self::UnbalancedOpstream => {
                write!(f, "unbalanced op-stream: OPEN and CLOSE ops do not pair up")
            }
            Self::InvalidNodeId(id) => {
                write!(f, "node id {id} does not exist in the target arena")
            }
            Self::TypeDataTooShort => {
                write!(
                    f,
                    "stored node type_data is shorter than its layout requires"
                )
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
