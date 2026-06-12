//! `satteri-arena`: generic arena-allocated tree structure.
//!
//! Provides the core data structures shared by MDAST and HAST:
//! - `Arena` for owning all nodes and associated data
//! - `ArenaBuilder` for incremental tree construction
//! - `ArenaNode` and `StringRef` for zero-copy node representation
//! - Raw buffer export/import for binary transfer
//! - `LineIndex` for offset→(line, column) conversion

pub mod arena;
pub mod builder;
pub mod codec;
mod generated;
pub mod kind;
pub mod line_index;
pub mod mdx_types;
pub mod node;
pub mod raw_buffer;
pub use arena::{Arena, TypeDataWriter};
pub use builder::ArenaBuilder;
pub use codec::{decode_string_ref_data, encode_string_ref_data};
pub use kind::{ArenaKind, Hast, Mdast};
pub use line_index::{LineIndex, LineIndexCursor};
pub use node::{ArenaNode, StringRef, NODE_STRUCT_SIZE};
