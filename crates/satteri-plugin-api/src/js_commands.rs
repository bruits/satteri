//! Binary command buffer parser and mutation applicator.
//!
//! Reads a command buffer produced by the JS `CommandBuffer` class, converts
//! commands into arena mutations, and returns the rebuilt arena.
//!
//! ## Wire format
//!
//! All multi-byte integers are **little-endian**. A command is one `CMD_*`
//! byte plus `[nodeId: u32]`; structural commands then carry a
//! `[payloadType: u8][len: u32][payload…]` sub-tree. The command, payload,
//! op-stream, and property-kind byte values (with their operand layouts) are
//! declared once in `satteri-layout-codegen/src/schema.rs` and generated into
//! `generated/wire_constants.rs` here and
//! `packages/satteri/src/generated/wire-constants.ts` on the JS side.
//!
//! The MDAST and HAST command paths are deliberately separate functions
//! (`apply_mdast_commands`, `apply_hast_commands`). Numeric `node_type`
//! values overlap between the two arenas (e.g. mdast Paragraph=1 collides
//! with HastNodeType::Element=1), so a single dispatcher trying to handle
//! both kinds would silently misroute nodes. The phantom-typed `Arena<K>`
//! signature on each entry point makes a cross-kind call a compile error.

use satteri_arena::{Arena, ArenaBuilder, ArenaKind, Hast, Mdast, StringRef};
use satteri_ast::commands::CommandError;
use satteri_ast::hast::HastNodeType;
use satteri_ast::mdast::codec::*;
use satteri_ast::mdast::MdastNodeType;
use satteri_ast::rebuild::{Patch, REF_NODE_TYPE};
#[cfg(feature = "mdx")]
use satteri_ast::shared::{MDX_ATTR_BOOLEAN_PROP, MDX_ATTR_LITERAL_PROP, MDX_ATTR_SPREAD};
use satteri_ast::shared::{
    PROP_BOOL_FALSE, PROP_BOOL_TRUE, PROP_INT, PROP_NULL, PROP_SPACE_SEP, PROP_STRING,
};

use crate::generated::prop_slots::{mdast_prop_slot, MdastPropSlot};
use crate::generated::wire_constants::*;

struct BufReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BufReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn read_u8(&mut self) -> Result<u8, CommandError> {
        if self.remaining() < 1 {
            return Err(CommandError::UnexpectedEof);
        }
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_u32(&mut self) -> Result<u32, CommandError> {
        if self.remaining() < 4 {
            return Err(CommandError::UnexpectedEof);
        }
        let v = u32::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], CommandError> {
        if self.remaining() < len {
            return Err(CommandError::UnexpectedEof);
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn read_str(&mut self, len: usize) -> Result<&'a str, CommandError> {
        let bytes = self.read_bytes(len)?;
        std::str::from_utf8(bytes).map_err(|_| CommandError::InvalidUtf8)
    }
}

/// `data` JSON blob is stored in the per-node `node_data` map; it doesn't
/// dispatch on node-type bytes, so it's safe under any kind.
fn apply_data_property<K: ArenaKind>(
    arena: &mut Arena<K>,
    node_id: u32,
    value_type: u8,
    value_str: &str,
) {
    if value_type == PROP_NULL {
        arena.set_node_data(node_id, Vec::new());
    } else {
        arena.set_node_data(node_id, value_str.as_bytes().to_vec());
    }
}

/// The canonical MDAST type name for a node-type byte, for error messages.
fn mdast_type_name(node_type: u8) -> String {
    match MdastNodeType::from_u8(node_type) {
        Some(t) => t.name().to_string(),
        None => format!("unknown({node_type})"),
    }
}

/// MDAST set-property: writes a typed field (or `data` JSON) onto an MDAST
/// node. Kind-tight to `Arena<Mdast>` — the HAST element-properties writer
/// can no longer be reached from here.
fn apply_mdast_set_property(
    arena: &mut Arena<Mdast>,
    node_id: u32,
    prop_name: &str,
    value_type: u8,
    value_str: &str,
) -> Result<(), CommandError> {
    if node_id as usize >= arena.len() {
        return Err(CommandError::InvalidNodeId(node_id));
    }
    if prop_name == "data" {
        apply_data_property(arena, node_id, value_type, value_str);
        return Ok(());
    }

    let node_type = arena.get_node(node_id).node_type;

    // The property doesn't resolve to a slot for this node type at all.
    let slot = mdast_prop_slot(node_type, prop_name).ok_or_else(|| CommandError::UnknownField {
        node_type: mdast_type_name(node_type),
        name: prop_name.to_string(),
    })?;

    // The slot resolved, so a `None` from the writer means the value's type is
    // one the slot can't hold — report that rather than "unknown".
    let written = write_mdast_prop_slot(arena, node_id, slot, prop_name, value_type, value_str)?;
    written.ok_or_else(|| CommandError::InvalidPropertyValue {
        node_type: mdast_type_name(node_type),
        name: prop_name.to_string(),
    })
}

/// Write a resolved slot. `Ok(None)` means the slot can't hold this value
/// type. String slots error `TypeDataTooShort` on short `type_data`; scalar
/// slots skip the write silently instead (the historical semantics).
fn write_mdast_prop_slot(
    arena: &mut Arena<Mdast>,
    node_id: u32,
    slot: MdastPropSlot,
    prop_name: &str,
    value_type: u8,
    value_str: &str,
) -> Result<Option<()>, CommandError> {
    use MdastPropSlot as S;
    match value_type {
        PROP_STRING | PROP_SPACE_SEP => match slot {
            S::Str { offset } => {
                let sref = arena.alloc_string(value_str);
                set_mdast_string_ref(arena, node_id, offset, sref)?;
            }
            S::Enum8 { offset, values } => {
                let Some(v) = values.iter().position(|v| *v == value_str) else {
                    return Ok(None);
                };
                set_mdast_scalar(arena, node_id, offset, &[v as u8]);
            }
            _ => return Ok(None),
        },
        PROP_BOOL_TRUE | PROP_BOOL_FALSE => match slot {
            S::Bool { offset } => {
                let value = value_type == PROP_BOOL_TRUE;
                set_mdast_scalar(arena, node_id, offset, &[value as u8]);
            }
            _ => return Ok(None),
        },
        PROP_INT => {
            // Accept a float spelling like "3.0", but anything outside the
            // slot's range must error — `as u8` would silently mask the bits.
            let parsed = value_str.parse::<i64>().ok().or_else(|| {
                value_str
                    .parse::<f64>()
                    .ok()
                    .filter(|f| f.is_finite() && f.fract() == 0.0)
                    .map(|f| f as i64)
            });
            let node_type = arena.get_node(node_id).node_type;
            let fitted = |max: u32| -> Result<u32, CommandError> {
                match parsed {
                    Some(v) if (0..=max as i64).contains(&v) => Ok(v as u32),
                    _ => Err(CommandError::PropertyValueOutOfRange {
                        node_type: mdast_type_name(node_type),
                        name: prop_name.to_string(),
                        value: value_str.to_string(),
                        max,
                    }),
                }
            };
            match slot {
                S::U8 { offset } | S::CheckedTri { offset } => {
                    let value = fitted(u8::MAX as u32)?;
                    set_mdast_scalar(arena, node_id, offset, &[value as u8]);
                }
                S::U32 { offset } => {
                    let value = fitted(u32::MAX)?;
                    set_mdast_scalar(arena, node_id, offset, &value.to_le_bytes());
                }
                _ => return Ok(None),
            }
        }
        PROP_NULL => match slot {
            // 2 = not a task item.
            S::CheckedTri { offset } => set_mdast_scalar(arena, node_id, offset, &[2]),
            S::Str { offset } => set_mdast_string_ref(arena, node_id, offset, StringRef::empty())?,
            _ => return Ok(None),
        },
        _ => return Err(CommandError::InvalidPropertyValueType(value_type)),
    }
    Ok(Some(()))
}

/// Write an 8-byte `StringRef` at `offset` into the node's `type_data`.
fn set_mdast_string_ref(
    arena: &mut Arena<Mdast>,
    node_id: u32,
    offset: usize,
    sref: StringRef,
) -> Result<(), CommandError> {
    let node = arena.get_node(node_id);
    let data_offset = node.data_offset as usize;
    let data_len = node.data_len as usize;
    if data_len < offset + 8 {
        return Err(CommandError::TypeDataTooShort);
    }
    let abs_offset = data_offset + offset;
    arena.type_data[abs_offset..abs_offset + 8].copy_from_slice(&sref.as_bytes());
    Ok(())
}

/// Write a scalar at `offset` into the node's `type_data`; too-short data
/// skips the write.
fn set_mdast_scalar(arena: &mut Arena<Mdast>, node_id: u32, offset: usize, bytes: &[u8]) {
    let node = arena.get_node(node_id);
    let data_offset = node.data_offset as usize;
    let data_len = node.data_len as usize;
    if data_len >= offset + bytes.len() {
        let abs_offset = data_offset + offset;
        arena.type_data[abs_offset..abs_offset + bytes.len()].copy_from_slice(bytes);
    }
}

/// Escape `{` and `}` in HTML text content so they are not interpreted as MDX
/// expressions when the HTML is re-parsed through the MDX parser.
///
/// Only braces in **text content** (outside of HTML tags) are escaped; braces
/// inside quoted attribute values are left untouched. The escape form `{'{'}` /
/// `{'}'}` produces a valid MDX expression that evaluates to the literal brace
/// character.
fn escape_braces_in_html_text(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_quote: Option<char> = None;

    for ch in html.chars() {
        if in_tag {
            match ch {
                '"' | '\'' if in_quote == Some(ch) => {
                    in_quote = None;
                    result.push(ch);
                }
                '"' | '\'' if in_quote.is_none() => {
                    in_quote = Some(ch);
                    result.push(ch);
                }
                '>' if in_quote.is_none() => {
                    in_tag = false;
                    result.push(ch);
                }
                _ => result.push(ch),
            }
        } else {
            match ch {
                '<' => {
                    in_tag = true;
                    result.push(ch);
                }
                '{' => result.push_str("{'{'}"),
                '}' => result.push_str("{'}'}"),
                _ => result.push(ch),
            }
        }
    }
    result
}

/// Options controlling how MDAST command buffers are applied.
#[derive(Debug, Clone, Copy)]
pub struct MdastCommandOptions {
    /// Escape raw HTML text braces before re-parsing it through an MDX parser.
    ///
    /// MDX needs this so literal `{` / `}` in HTML text are not interpreted as
    /// expressions. Plain Markdown-to-HTML pipelines should leave raw HTML
    /// opaque so the final HTML preserves those braces verbatim.
    pub escape_raw_html_braces: bool,
}

impl Default for MdastCommandOptions {
    fn default() -> Self {
        Self {
            escape_raw_html_braces: true,
        }
    }
}

/// Emit a reference placeholder: a `REF_NODE_TYPE` node carrying the target
/// original id (u32 LE) in its type_data. The rebuild resolves it by splicing
/// that original subtree and applying any pending patch on it.
fn emit_ref_node<K: ArenaKind>(ref_id: u32, builder: &mut ArenaBuilder<K>) {
    builder.open_node_raw(REF_NODE_TYPE);
    builder.set_data_current(&ref_id.to_le_bytes());
    builder.close_node();
}

// Generated per-type arena encoder, driven by the node registry. See
// `crates/satteri-layout-codegen`.
use crate::generated::encode::{
    encode_hast_tail_from_ops, encode_mdast_tail_from_ops, encode_mdast_type_data_from_ops,
    MAX_FIXED_TYPE_DATA,
};

pub(crate) fn alloc_opt_str<K: ArenaKind>(
    builder: &mut ArenaBuilder<K>,
    s: Option<&str>,
) -> StringRef {
    match s {
        Some(v) if !v.is_empty() => builder.alloc_string(v),
        _ => StringRef::empty(),
    }
}

// HAST command handlers

/// HAST set-property: dispatches by `HastNodeType` to the matching writer.
/// Kind-tight to `Arena<Hast>` — the MDAST field-resolver can no longer be
/// reached from here.
fn apply_hast_set_property(
    arena: &mut Arena<Hast>,
    node_id: u32,
    prop_name: &str,
    value_type: u8,
    value_str: &str,
) -> Result<(), CommandError> {
    if node_id as usize >= arena.len() {
        return Err(CommandError::InvalidNodeId(node_id));
    }
    if prop_name == "data" {
        apply_data_property(arena, node_id, value_type, value_str);
        return Ok(());
    }

    let raw_type = arena.get_node(node_id).node_type;
    let node_type = HastNodeType::from_u8(raw_type)
        .ok_or_else(|| CommandError::UnknownNodeType(format!("hast type 0x{raw_type:02x}")))?;

    match node_type {
        HastNodeType::Element => {
            apply_hast_element_property(arena, node_id, prop_name, value_type, value_str)
        }

        HastNodeType::Text
        | HastNodeType::Comment
        | HastNodeType::Raw
        | HastNodeType::MdxFlowExpression
        | HastNodeType::MdxTextExpression
        | HastNodeType::MdxEsm
            if prop_name == "value" =>
        {
            let sref = arena.alloc_string(value_str);
            let data = arena.get_type_data(node_id);
            if data.len() >= 8 {
                let data_offset = arena.get_node(node_id).data_offset as usize;
                arena.type_data[data_offset..data_offset + 8].copy_from_slice(&sref.as_bytes());
                Ok(())
            } else {
                Err(CommandError::TypeDataTooShort)
            }
        }

        #[cfg(feature = "mdx")]
        HastNodeType::MdxJsxElement | HastNodeType::MdxJsxTextElement => {
            apply_hast_mdx_jsx_attribute(arena, node_id, prop_name, value_type, value_str)
        }

        _ => Err(CommandError::UnknownField {
            node_type: node_type.name().to_string(),
            name: prop_name.to_string(),
        }),
    }
}

/// Upsert a single attribute on an MDX JSX flow/text element. Avoids
/// re-serializing the whole node (and materializing its children) just to
/// change one attribute.
///
/// Any existing named attribute (boolean, literal, or expression-valued) with
/// the same name is removed and the new attribute appended at the end, so the
/// write wins over earlier spreads — the same ordering as the JS fold path.
/// Only spreads are never matched: they have no name.
///
/// Value-type mapping (matches the JS fold path this replaces):
///   bool-true / null -> boolean attribute (no value)
///   bool-false       -> literal `"false"`
///   string / int / … -> literal attribute carrying the value
#[cfg(feature = "mdx")]
fn apply_hast_mdx_jsx_attribute(
    arena: &mut Arena<Hast>,
    node_id: u32,
    attr_name: &str,
    value_type: u8,
    value_str: &str,
) -> Result<(), CommandError> {
    let old_data = arena.get_type_data(node_id).to_vec();
    if old_data.len() < 16 {
        return Err(CommandError::TypeDataTooShort);
    }
    let elem_name = decode_mdx_jsx_element_name(&old_data);
    let explicit = decode_mdx_jsx_explicit(&old_data);
    let attr_count = decode_mdx_jsx_attr_count(&old_data);

    // Map the binary value-type to a JSX attribute (kind, value).
    let (kind, val_ref) = match value_type {
        PROP_BOOL_TRUE | PROP_NULL => (MDX_ATTR_BOOLEAN_PROP, StringRef::empty()),
        PROP_BOOL_FALSE => (MDX_ATTR_LITERAL_PROP, arena.alloc_string("false")),
        _ if value_str.is_empty() => (MDX_ATTR_LITERAL_PROP, StringRef::empty()),
        _ => (MDX_ATTR_LITERAL_PROP, arena.alloc_string(value_str)),
    };

    let mut attrs: Vec<(u8, StringRef, StringRef)> = Vec::with_capacity(attr_count as usize + 1);
    let mut name_ref: Option<StringRef> = None;
    for i in 0..attr_count {
        let (existing_kind, existing_name, existing_value) = decode_mdx_jsx_attr(&old_data, i);
        if existing_kind != MDX_ATTR_SPREAD && arena.get_str(existing_name) == attr_name {
            name_ref = Some(existing_name);
            continue;
        }
        attrs.push((existing_kind, existing_name, existing_value));
    }
    let name_ref = name_ref.unwrap_or_else(|| arena.alloc_string(attr_name));
    attrs.push((kind, name_ref, val_ref));

    arena.set_type_data(
        node_id,
        &encode_mdx_jsx_element_data(elem_name, &attrs, explicit),
    );
    Ok(())
}

/// Set or add a single property on a HAST element node.
fn apply_hast_element_property(
    arena: &mut Arena<Hast>,
    node_id: u32,
    prop_name: &str,
    value_type: u8,
    value_str: &str,
) -> Result<(), CommandError> {
    let old_data = arena.get_type_data(node_id).to_vec();
    if old_data.len() < 16 {
        return Err(CommandError::TypeDataTooShort);
    }

    let old_prop_count = u32::from_le_bytes(old_data[8..12].try_into().unwrap()) as usize;

    let mut found_index: Option<usize> = None;
    for i in 0..old_prop_count {
        let base = 16 + i * 20;
        let name_off = u32::from_le_bytes(old_data[base..base + 4].try_into().unwrap());
        let name_len = u32::from_le_bytes(old_data[base + 4..base + 8].try_into().unwrap());
        let existing_name = arena.get_str(StringRef::new(name_off, name_len));
        if existing_name == prop_name {
            found_index = Some(i);
            break;
        }
    }

    let name_ref = arena.alloc_string(prop_name);
    let val_ref = if value_str.is_empty() {
        StringRef::empty()
    } else {
        arena.alloc_string(value_str)
    };

    if let Some(idx) = found_index {
        let mut new_data = old_data;
        let base = 16 + idx * 20;
        new_data[base..base + 4].copy_from_slice(&name_ref.offset.to_le_bytes());
        new_data[base + 4..base + 8].copy_from_slice(&name_ref.len.to_le_bytes());
        new_data[base + 8] = value_type;
        new_data[base + 9..base + 12].copy_from_slice(&[0u8; 3]);
        new_data[base + 12..base + 16].copy_from_slice(&val_ref.offset.to_le_bytes());
        new_data[base + 16..base + 20].copy_from_slice(&val_ref.len.to_le_bytes());
        arena.set_type_data(node_id, &new_data);
    } else {
        let new_prop_count = (old_prop_count + 1) as u32;
        let mut new_data = Vec::with_capacity(16 + new_prop_count as usize * 20);
        new_data.extend_from_slice(&old_data[0..8]);
        new_data.extend_from_slice(&new_prop_count.to_le_bytes());
        new_data.extend_from_slice(&0u32.to_le_bytes());
        if old_prop_count > 0 {
            new_data.extend_from_slice(&old_data[16..16 + old_prop_count * 20]);
        }
        new_data.extend_from_slice(&name_ref.offset.to_le_bytes());
        new_data.extend_from_slice(&name_ref.len.to_le_bytes());
        new_data.push(value_type);
        new_data.extend_from_slice(&[0u8; 3]);
        new_data.extend_from_slice(&val_ref.offset.to_le_bytes());
        new_data.extend_from_slice(&val_ref.len.to_le_bytes());
        arena.set_type_data(node_id, &new_data);
    }

    Ok(())
}

// The JS visitors compile declarative trees to an op-stream (OPEN/CLOSE/field
// sets/REF/KEEP_CHILDREN) that we replay directly into an ArenaBuilder — no
// intermediate node tree, no heap allocation per node beyond the arena itself. A node's
// fields are collected after its OPEN and flushed into its type_data the moment
// the next op needs the node finalized (a child OPEN, a CLOSE, or a spliced
// REF/KEEP_CHILDREN).

/// Per-kind hooks for [`replay_opstream`]. `Kind` ties a collector to one
/// arena flavor, so a cross-kind replay stays a compile error (see the module
/// header on why MDAST/HAST must not share a dispatcher).
trait OpCollector<'a>: Sized {
    type Kind: ArenaKind;
    /// Whether `OP_U8` / `OP_U32` / `OP_ALIGN` are decoded. When false those
    /// opcodes fall through to the unknown-command error *without consuming
    /// operands*, so the reported byte is the opcode itself.
    const NUMERIC_OPS: bool;

    fn open(node_type: u8) -> Self;
    fn check_tag(tag: u8) -> Result<(), CommandError>;
    fn finalize(&mut self, builder: &mut ArenaBuilder<Self::Kind>);
    fn str_field(&mut self, field: u8, value: &'a str);
    fn bool_field(&mut self, field: u8, value: bool);
    fn prop(&mut self, name: &'a str, kind: u8, value: &'a str);
    fn data(&mut self, bytes: &'a [u8]);
    fn u8_field(&mut self, _field: u8, _value: u8) {}
    fn u32_field(&mut self, _field: u8, _value: u32) {}
    fn align(&mut self, _bytes: &'a [u8]) {}
}

/// Deepest `OP_OPEN` nesting the replay accepts: the rebuild splices a
/// replayed sub-arena recursively, so its depth must stay well inside the
/// host stack. 128 = serde_json's default recursion limit, ample for content.
const MAX_OPSTREAM_DEPTH: usize = 128;

/// Replay an op-stream into a fresh sub-arena. `orig`/`anchor` resolve
/// `KEEP_CHILDREN` (splice the replaced node's original children, as refs).
fn replay_opstream<'a, C: OpCollector<'a>>(
    ops: &'a [u8],
    orig: &Arena<C::Kind>,
    anchor: u32,
) -> Result<Arena<C::Kind>, CommandError> {
    let mut builder = ArenaBuilder::<C::Kind>::new(String::new());
    let mut reader = BufReader::new(ops);
    let mut stack: Vec<C> = Vec::new();

    while reader.remaining() > 0 {
        match reader.read_u8()? {
            OP_OPEN => {
                if let Some(c) = stack.last_mut() {
                    c.finalize(&mut builder);
                }
                let node_type = reader.read_u8()?;
                C::check_tag(node_type)?;
                // A root is only valid as the stream's top-level wrapper
                // (the rebuild splices its children); nested it would smuggle
                // a node the JS visitors refuse to encode.
                if !stack.is_empty() && node_type == <C::Kind as ArenaKind>::ROOT_TAG {
                    return Err(CommandError::UnencodableNodeType("root"));
                }
                if stack.len() >= MAX_OPSTREAM_DEPTH {
                    return Err(CommandError::OpstreamTooDeep(MAX_OPSTREAM_DEPTH));
                }
                builder.open_node(node_type);
                stack.push(C::open(node_type));
            }
            OP_CLOSE => {
                let Some(mut c) = stack.pop() else {
                    return Err(CommandError::UnbalancedOpstream);
                };
                c.finalize(&mut builder);
                builder.close_node();
            }
            OP_REF => {
                if let Some(c) = stack.last_mut() {
                    c.finalize(&mut builder);
                }
                let id = reader.read_u32()?;
                // A stale id (e.g. a node cached across passes) would
                // otherwise panic deep inside the rebuild's arena indexing.
                if id as usize >= orig.len() {
                    return Err(CommandError::InvalidNodeId(id));
                }
                emit_ref_node(id, &mut builder);
            }
            OP_KEEP_CHILDREN => {
                if let Some(c) = stack.last_mut() {
                    c.finalize(&mut builder);
                }
                if anchor as usize >= orig.len() {
                    return Err(CommandError::InvalidNodeId(anchor));
                }
                for &child in orig.get_children(anchor) {
                    emit_ref_node(child, &mut builder);
                }
            }
            OP_STR => {
                let field = reader.read_u8()?;
                let len = reader.read_u32()? as usize;
                let value = reader.read_str(len)?;
                if let Some(c) = stack.last_mut() {
                    c.str_field(field, value);
                }
            }
            OP_U8 if C::NUMERIC_OPS => {
                let field = reader.read_u8()?;
                let value = reader.read_u8()?;
                if let Some(c) = stack.last_mut() {
                    c.u8_field(field, value);
                }
            }
            OP_U32 if C::NUMERIC_OPS => {
                let field = reader.read_u8()?;
                let value = reader.read_u32()?;
                if let Some(c) = stack.last_mut() {
                    c.u32_field(field, value);
                }
            }
            OP_BOOL => {
                let field = reader.read_u8()?;
                let value = reader.read_u8()? != 0;
                if let Some(c) = stack.last_mut() {
                    c.bool_field(field, value);
                }
            }
            OP_PROP => {
                let name_len = reader.read_u32()? as usize;
                let name = reader.read_str(name_len)?;
                let kind = reader.read_u8()?;
                let val_len = reader.read_u32()? as usize;
                let value = reader.read_str(val_len)?;
                if let Some(c) = stack.last_mut() {
                    c.prop(name, kind, value);
                }
            }
            OP_ALIGN if C::NUMERIC_OPS => {
                let len = reader.read_u32()? as usize;
                let bytes = reader.read_bytes(len)?;
                if let Some(c) = stack.last_mut() {
                    c.align(bytes);
                }
            }
            OP_DATA => {
                let len = reader.read_u32()? as usize;
                let bytes = reader.read_bytes(len)?;
                if let Some(c) = stack.last_mut() {
                    c.data(bytes);
                }
            }
            other => return Err(CommandError::UnknownCommand(other)),
        }
    }

    if !stack.is_empty() {
        return Err(CommandError::UnbalancedOpstream);
    }
    Ok(builder.finish())
}

/// Intern MDX-JSX attribute strings into `(kind, name, value)` rows; spreads
/// carry no name, boolean attrs no value.
#[cfg(feature = "mdx")]
pub(crate) fn intern_mdx_jsx_attrs<K: ArenaKind>(
    props: &[(&str, u8, &str)],
    builder: &mut ArenaBuilder<K>,
) -> Vec<(u8, StringRef, StringRef)> {
    let mut attrs = Vec::with_capacity(props.len());
    for &(name, kind, value) in props {
        let nr = if kind == MDX_ATTR_SPREAD {
            StringRef::empty()
        } else {
            builder.alloc_string(name)
        };
        let vr = if kind == MDX_ATTR_BOOLEAN_PROP {
            StringRef::empty()
        } else {
            builder.alloc_string(value)
        };
        attrs.push((kind, nr, vr));
    }
    attrs
}

/// Accumulates one node's fields between its OPEN and finalization. Strings
/// borrow the op-stream buffer; they're interned into the arena at finalize.
#[derive(Default)]
pub(crate) struct FieldCollector<'a> {
    node_type: u8,
    finalized: bool,
    pub(crate) strs: [Option<&'a str>; OF_FIELD_COUNT],
    pub(crate) depth: Option<u8>,
    checked: Option<u8>,
    start: Option<u32>,
    ordered: Option<bool>,
    spread: Option<bool>,
    /// Directive / MDX JSX attributes (`OP_PROP`): (name, kind, value).
    pub(crate) props: Vec<(&'a str, u8, &'a str)>,
    /// Table column-alignment bytes (`OP_ALIGN`).
    pub(crate) align: Option<&'a [u8]>,
    /// MDX JSX `_mdxExplicitJsx` flag (`OP_BOOL` on `OF_EXPLICIT`).
    pub(crate) explicit: Option<bool>,
    data: Option<&'a [u8]>,
}

/// Encode a collector's fields into the current node's type_data (and node_data).
fn finalize_collector(c: &mut FieldCollector<'_>, builder: &mut ArenaBuilder<Mdast>) {
    const LIST: u8 = MdastNodeType::List as u8;
    const LIST_ITEM: u8 = MdastNodeType::ListItem as u8;

    if c.finalized {
        return;
    }
    c.finalized = true;
    let mut fixed = [0u8; MAX_FIXED_TYPE_DATA];
    if let Some(len) = encode_mdast_type_data_from_ops(c, c.node_type, builder, &mut fixed) {
        builder.set_data_current(&fixed[..len]);
    } else if let Some(type_data) = encode_mdast_tail_from_ops(c, c.node_type, builder) {
        // Generated tail encoder (directive attributes, MDX JSX); see encode.rs.
        builder.set_data_current(&type_data);
    } else {
        let type_data: Vec<u8> = match c.node_type {
            LIST => encode_list_data(
                c.ordered.unwrap_or(false),
                c.start.unwrap_or(1),
                c.spread.unwrap_or(false),
            ),
            // checked: 2 = not a task item
            LIST_ITEM => encode_list_item_data(c.checked.unwrap_or(2), c.spread.unwrap_or(false)),
            // Remaining tags carry no type_data.
            _ => Vec::new(),
        };
        if !type_data.is_empty() {
            builder.set_data_current(&type_data);
        }
    }
    if let Some(data) = c.data {
        let id = builder.current_node_id();
        builder.arena_mut().set_node_data(id, data.to_vec());
    }
}

impl<'a> OpCollector<'a> for FieldCollector<'a> {
    type Kind = Mdast;
    const NUMERIC_OPS: bool = true;

    fn open(node_type: u8) -> Self {
        FieldCollector {
            node_type,
            ..Default::default()
        }
    }

    /// Reject op-stream tags this build can't construct — unknown bytes
    /// always, MDX tags without the `mdx` feature.
    fn check_tag(tag: u8) -> Result<(), CommandError> {
        let known = MdastNodeType::from_u8(tag).is_some();
        #[cfg(not(feature = "mdx"))]
        let known = known
            && !matches!(
                MdastNodeType::from_u8(tag),
                Some(
                    MdastNodeType::MdxJsxFlowElement
                        | MdastNodeType::MdxJsxTextElement
                        | MdastNodeType::MdxFlowExpression
                        | MdastNodeType::MdxTextExpression
                        | MdastNodeType::MdxjsEsm
                )
            );
        if known {
            Ok(())
        } else {
            Err(CommandError::UnknownNodeType(format!(
                "op-stream tag {tag}"
            )))
        }
    }

    fn finalize(&mut self, builder: &mut ArenaBuilder<Mdast>) {
        finalize_collector(self, builder);
    }

    fn str_field(&mut self, field: u8, value: &'a str) {
        let field = field as usize;
        if field < self.strs.len() {
            self.strs[field] = Some(value);
        }
    }

    fn bool_field(&mut self, field: u8, value: bool) {
        match field {
            OF_ORDERED => self.ordered = Some(value),
            OF_SPREAD => self.spread = Some(value),
            OF_EXPLICIT => self.explicit = Some(value),
            _ => {}
        }
    }

    fn prop(&mut self, name: &'a str, kind: u8, value: &'a str) {
        self.props.push((name, kind, value));
    }

    fn data(&mut self, bytes: &'a [u8]) {
        self.data = Some(bytes);
    }

    fn u8_field(&mut self, field: u8, value: u8) {
        match field {
            OF_DEPTH => self.depth = Some(value),
            OF_CHECKED => self.checked = Some(value),
            _ => {}
        }
    }

    fn u32_field(&mut self, field: u8, value: u32) {
        if field == OF_START {
            self.start = Some(value);
        }
    }

    fn align(&mut self, bytes: &'a [u8]) {
        self.align = Some(bytes);
    }
}

fn replay_mdast_opstream(
    ops: &[u8],
    orig: &Arena<Mdast>,
    anchor: u32,
) -> Result<Arena<Mdast>, CommandError> {
    replay_opstream::<FieldCollector>(ops, orig, anchor)
}

/// Returns (arena, keep_children) for an MDAST sub-tree payload. `orig`/`anchor`
/// are the arena and the command's target node, used by an op-stream's
/// `KEEP_CHILDREN`.
fn read_mdast_payload(
    reader: &mut BufReader<'_>,
    parse_markdown: &dyn Fn(&str) -> Arena<Mdast>,
    orig: &Arena<Mdast>,
    anchor: u32,
    options: MdastCommandOptions,
) -> Result<(Arena<Mdast>, bool), CommandError> {
    let payload_type = reader.read_u8()?;
    let len = reader.read_u32()? as usize;

    match payload_type {
        PAYLOAD_RAW_MARKDOWN => {
            let md = reader.read_str(len)?;
            Ok((parse_markdown(md), false))
        }
        PAYLOAD_RAW_HTML => {
            let html = reader.read_str(len)?;
            if options.escape_raw_html_braces {
                let escaped = escape_braces_in_html_text(html);
                Ok((parse_markdown(&escaped), false))
            } else {
                Ok((parse_markdown(html), false))
            }
        }
        PAYLOAD_OPSTREAM => {
            let ops = reader.read_bytes(len)?;
            Ok((replay_mdast_opstream(ops, orig, anchor)?, false))
        }
        other => Err(CommandError::UnknownPayloadType(other)),
    }
}

/// HAST op-stream replay (mirrors the MDAST version). Builds element type_data
/// (tag + properties) and text/comment/raw values; `OP_PROP` carries properties,
/// `OF_TAGNAME` the tag.
#[derive(Default)]
pub(crate) struct HastFieldCollector<'a> {
    node_type: u8,
    finalized: bool,
    /// HAST element `tagName` or MDX JSX element `name`.
    pub(crate) tag: Option<&'a str>,
    value: Option<&'a str>,
    pub(crate) props: Vec<(&'a str, u8, &'a str)>,
    /// MDX JSX `_mdxExplicitJsx` flag (`OP_BOOL` on `OF_EXPLICIT`).
    pub(crate) explicit: Option<bool>,
    data: Option<&'a [u8]>,
}

fn finalize_hast_collector(c: &mut HastFieldCollector<'_>, builder: &mut ArenaBuilder<Hast>) {
    const TEXT: u8 = HastNodeType::Text as u8;
    const COMMENT: u8 = HastNodeType::Comment as u8;
    const RAW: u8 = HastNodeType::Raw as u8;
    const MDX_FLOW_EXPRESSION: u8 = HastNodeType::MdxFlowExpression as u8;
    const MDX_ESM: u8 = HastNodeType::MdxEsm as u8;
    const MDX_TEXT_EXPRESSION: u8 = HastNodeType::MdxTextExpression as u8;

    if c.finalized {
        return;
    }
    c.finalized = true;
    if let Some(type_data) = encode_hast_tail_from_ops(c, c.node_type, builder) {
        // Generated tail encoder (element properties, MDX JSX); see encode.rs.
        builder.set_data_current(&type_data);
    } else {
        let type_data: Vec<u8> = match c.node_type {
            TEXT | COMMENT | RAW | MDX_FLOW_EXPRESSION | MDX_ESM | MDX_TEXT_EXPRESSION => {
                let sref = builder.alloc_string(c.value.unwrap_or(""));
                encode_string_ref_data(sref)
            }
            // Remaining tags carry no type_data.
            _ => Vec::new(),
        };
        if !type_data.is_empty() {
            builder.set_data_current(&type_data);
        }
    }
    if let Some(data) = c.data {
        let id = builder.current_node_id();
        builder.arena_mut().set_node_data(id, data.to_vec());
    }
}

impl<'a> OpCollector<'a> for HastFieldCollector<'a> {
    type Kind = Hast;
    const NUMERIC_OPS: bool = false;

    fn open(node_type: u8) -> Self {
        HastFieldCollector {
            node_type,
            ..Default::default()
        }
    }

    /// HAST twin of the MDAST `check_tag`.
    fn check_tag(tag: u8) -> Result<(), CommandError> {
        // The JS visitor refuses to encode a doctype (it's not in
        // `HAST_OPSTREAM_TYPES`); enforce the same here so a crafted buffer
        // can't smuggle one in.
        if HastNodeType::from_u8(tag) == Some(HastNodeType::Doctype) {
            return Err(CommandError::UnencodableNodeType("doctype"));
        }
        let known = HastNodeType::from_u8(tag).is_some();
        #[cfg(not(feature = "mdx"))]
        let known = known
            && !matches!(
                HastNodeType::from_u8(tag),
                Some(
                    HastNodeType::MdxJsxElement
                        | HastNodeType::MdxJsxTextElement
                        | HastNodeType::MdxFlowExpression
                        | HastNodeType::MdxEsm
                        | HastNodeType::MdxTextExpression
                )
            );
        if known {
            Ok(())
        } else {
            Err(CommandError::UnknownNodeType(format!(
                "op-stream tag {tag}"
            )))
        }
    }

    fn finalize(&mut self, builder: &mut ArenaBuilder<Hast>) {
        finalize_hast_collector(self, builder);
    }

    fn str_field(&mut self, field: u8, value: &'a str) {
        match field {
            OF_TAGNAME | OF_NAME => self.tag = Some(value),
            OF_VALUE => self.value = Some(value),
            _ => {}
        }
    }

    fn bool_field(&mut self, field: u8, value: bool) {
        if field == OF_EXPLICIT {
            self.explicit = Some(value);
        }
    }

    fn prop(&mut self, name: &'a str, kind: u8, value: &'a str) {
        self.props.push((name, kind, value));
    }

    fn data(&mut self, bytes: &'a [u8]) {
        self.data = Some(bytes);
    }
}

fn replay_hast_opstream(
    ops: &[u8],
    orig: &Arena<Hast>,
    anchor: u32,
) -> Result<Arena<Hast>, CommandError> {
    replay_opstream::<HastFieldCollector>(ops, orig, anchor)
}

/// Returns (arena, keep_children) for a HAST sub-tree payload. Only
/// `PAYLOAD_OPSTREAM` (declarative-compiled) is accepted — HAST has no source
/// grammar, so raw markdown / HTML are not, and there is no JSON path.
fn read_hast_payload(
    reader: &mut BufReader<'_>,
    orig: &Arena<Hast>,
    anchor: u32,
) -> Result<(Arena<Hast>, bool), CommandError> {
    let payload_type = reader.read_u8()?;
    let len = reader.read_u32()? as usize;

    match payload_type {
        PAYLOAD_OPSTREAM => {
            let ops = reader.read_bytes(len)?;
            Ok((replay_hast_opstream(ops, orig, anchor)?, false))
        }
        other => Err(CommandError::UnknownPayloadType(other)),
    }
}

/// Apply a command buffer to an MDAST arena. Set-property mutations are
/// applied in-place; structural mutations are collected as `Patch<Mdast>`
/// objects and applied via `rebuild()`.
///
/// `parse_markdown` avoids a circular dependency on the parser crate; it
/// is invoked for `RAW_MARKDOWN` and `RAW_HTML` payloads.
///
/// Passing a HAST arena is a compile error — the prior single-dispatch
/// `apply_commands` would silently misroute MDAST nodes into the HAST
/// element-properties writer (numeric `node_type` values overlap between
/// the two arenas):
///
/// ```compile_fail
/// use satteri_arena::{Arena, Hast};
/// use satteri_plugin_api::apply_mdast_commands;
///
/// let arena: Arena<Hast> = Arena::new(String::new());
/// let parse_markdown = |_: &str| -> Arena<satteri_arena::Mdast> {
///     Arena::new(String::new())
/// };
/// let _ = apply_mdast_commands(arena, &[], &parse_markdown);
/// ```
pub fn apply_mdast_commands(
    arena: Arena<Mdast>,
    command_buf: &[u8],
    parse_markdown: &dyn Fn(&str) -> Arena<Mdast>,
) -> Result<Arena<Mdast>, CommandError> {
    apply_mdast_commands_with_options(
        arena,
        command_buf,
        parse_markdown,
        MdastCommandOptions::default(),
    )
}

/// Like [`apply_mdast_commands`], with explicit command application options.
pub fn apply_mdast_commands_with_options(
    arena: Arena<Mdast>,
    command_buf: &[u8],
    parse_markdown: &dyn Fn(&str) -> Arena<Mdast>,
    options: MdastCommandOptions,
) -> Result<Arena<Mdast>, CommandError> {
    let (arena, dropped) =
        apply_mdast_commands_lenient_with_options(arena, command_buf, parse_markdown, options)?;
    if let Some(anchor) = dropped.first() {
        return Err(CommandError::PatchOnRemovedSubtree(*anchor));
    }
    Ok(arena)
}

/// Like [`apply_mdast_commands`], but rather than erroring when a patch targets
/// a node inside a removed/replaced subtree, drops it and returns the dropped
/// anchors. Such a patch is moot — the plugin discarded that subtree. A
/// *passed-through* child is not dropped: it rides a `_ref` placeholder that
/// splices it back with its id intact, so a transform queued on a nested node
/// (e.g. a `:::tip` inside a `:::note`) still applies, in the same pass.
pub fn apply_mdast_commands_lenient(
    arena: Arena<Mdast>,
    command_buf: &[u8],
    parse_markdown: &dyn Fn(&str) -> Arena<Mdast>,
) -> Result<(Arena<Mdast>, Vec<u32>), CommandError> {
    apply_mdast_commands_lenient_with_options(
        arena,
        command_buf,
        parse_markdown,
        MdastCommandOptions::default(),
    )
}

/// Like [`apply_mdast_commands_lenient`], with explicit command application
/// options.
pub fn apply_mdast_commands_lenient_with_options(
    mut arena: Arena<Mdast>,
    command_buf: &[u8],
    parse_markdown: &dyn Fn(&str) -> Arena<Mdast>,
    options: MdastCommandOptions,
) -> Result<(Arena<Mdast>, Vec<u32>), CommandError> {
    if command_buf.is_empty() {
        return Ok((arena, Vec::new()));
    }

    let mut patches: Vec<Patch<Mdast>> = Vec::new();
    let mut reader = BufReader::new(command_buf);

    while reader.remaining() > 0 {
        let cmd = reader.read_u8()?;

        match cmd {
            CMD_REMOVE => {
                let node_id = reader.read_u32()?;
                patches.push(Patch::Remove { node_id });
            }

            CMD_SET_PROPERTY => {
                let node_id = reader.read_u32()?;
                let value_type = reader.read_u8()?;
                let name_len = reader.read_u32()? as usize;
                let name = reader.read_str(name_len)?;
                let value_len = reader.read_u32()? as usize;
                let value = reader.read_str(value_len)?;
                apply_mdast_set_property(&mut arena, node_id, name, value_type, value)?;
            }

            CMD_INSERT_BEFORE => {
                let node_id = reader.read_u32()?;
                let (new_tree, _) =
                    read_mdast_payload(&mut reader, parse_markdown, &arena, node_id, options)?;
                patches.push(Patch::InsertBefore { node_id, new_tree });
            }

            CMD_INSERT_AFTER => {
                let node_id = reader.read_u32()?;
                let (new_tree, _) =
                    read_mdast_payload(&mut reader, parse_markdown, &arena, node_id, options)?;
                patches.push(Patch::InsertAfter { node_id, new_tree });
            }

            CMD_PREPEND_CHILD => {
                let node_id = reader.read_u32()?;
                let (child_tree, _) =
                    read_mdast_payload(&mut reader, parse_markdown, &arena, node_id, options)?;
                patches.push(Patch::PrependChild {
                    node_id,
                    child_tree,
                });
            }

            CMD_APPEND_CHILD => {
                let node_id = reader.read_u32()?;
                let (child_tree, _) =
                    read_mdast_payload(&mut reader, parse_markdown, &arena, node_id, options)?;
                patches.push(Patch::AppendChild {
                    node_id,
                    child_tree,
                });
            }

            CMD_WRAP => {
                let node_id = reader.read_u32()?;
                let (parent_tree, _) =
                    read_mdast_payload(&mut reader, parse_markdown, &arena, node_id, options)?;
                patches.push(Patch::Wrap {
                    node_id,
                    parent_tree,
                });
            }

            CMD_REPLACE => {
                let node_id = reader.read_u32()?;
                let (new_tree, keep_children) =
                    read_mdast_payload(&mut reader, parse_markdown, &arena, node_id, options)?;
                patches.push(Patch::Replace {
                    node_id,
                    new_tree,
                    keep_children,
                });
            }

            CMD_SET_CHILDREN => {
                let node_id = reader.read_u32()?;
                let (new_children, _) =
                    read_mdast_payload(&mut reader, parse_markdown, &arena, node_id, options)?;
                patches.push(Patch::SetChildren {
                    node_id,
                    new_children,
                });
            }

            other => return Err(CommandError::UnknownCommand(other)),
        }
    }

    if patches.is_empty() {
        Ok((arena, Vec::new()))
    } else {
        let result = satteri_ast::rebuild::rebuild_lenient(&arena, &patches)?;
        Ok((result.arena, result.dropped))
    }
}

/// Apply a command buffer to a HAST arena. Set-property mutations are
/// applied in-place; structural mutations are collected as `Patch<Hast>`
/// objects and applied via `rebuild()`. Errors if a patch is stranded inside a
/// removed/replaced subtree; [`apply_hast_commands_lenient`] drops it instead.
///
/// HAST plugins inject sub-trees via `PAYLOAD_OPSTREAM` only — there is
/// no `parse_markdown` callback because HAST has no source-level grammar.
///
/// Passing an MDAST arena is a compile error:
///
/// ```compile_fail
/// use satteri_arena::{Arena, Mdast};
/// use satteri_plugin_api::apply_hast_commands;
///
/// let arena: Arena<Mdast> = Arena::new(String::new());
/// let _ = apply_hast_commands(arena, &[]);
/// ```
pub fn apply_hast_commands(
    arena: Arena<Hast>,
    command_buf: &[u8],
) -> Result<Arena<Hast>, CommandError> {
    let (arena, dropped) = apply_hast_commands_lenient(arena, command_buf)?;
    if let Some(anchor) = dropped.first() {
        return Err(CommandError::PatchOnRemovedSubtree(*anchor));
    }
    Ok(arena)
}

/// Like [`apply_hast_commands`], but rather than erroring when a patch targets a
/// node inside a removed/replaced subtree, drops it and returns the dropped
/// anchors — mirroring [`apply_mdast_commands_lenient`]. Such a patch is moot:
/// the plugin discarded that subtree. A passed-through child keeps its identity
/// (via `_ref`) and so is never stranded this way.
pub fn apply_hast_commands_lenient(
    mut arena: Arena<Hast>,
    command_buf: &[u8],
) -> Result<(Arena<Hast>, Vec<u32>), CommandError> {
    if command_buf.is_empty() {
        return Ok((arena, Vec::new()));
    }

    let mut patches: Vec<Patch<Hast>> = Vec::new();
    let mut reader = BufReader::new(command_buf);

    while reader.remaining() > 0 {
        let cmd = reader.read_u8()?;

        match cmd {
            CMD_REMOVE => {
                let node_id = reader.read_u32()?;
                patches.push(Patch::Remove { node_id });
            }

            CMD_SET_PROPERTY => {
                let node_id = reader.read_u32()?;
                let value_type = reader.read_u8()?;
                let name_len = reader.read_u32()? as usize;
                let name = reader.read_str(name_len)?;
                let value_len = reader.read_u32()? as usize;
                let value = reader.read_str(value_len)?;
                apply_hast_set_property(&mut arena, node_id, name, value_type, value)?;
            }

            CMD_INSERT_BEFORE => {
                let node_id = reader.read_u32()?;
                let (new_tree, _) = read_hast_payload(&mut reader, &arena, node_id)?;
                patches.push(Patch::InsertBefore { node_id, new_tree });
            }

            CMD_INSERT_AFTER => {
                let node_id = reader.read_u32()?;
                let (new_tree, _) = read_hast_payload(&mut reader, &arena, node_id)?;
                patches.push(Patch::InsertAfter { node_id, new_tree });
            }

            CMD_PREPEND_CHILD => {
                let node_id = reader.read_u32()?;
                let (child_tree, _) = read_hast_payload(&mut reader, &arena, node_id)?;
                patches.push(Patch::PrependChild {
                    node_id,
                    child_tree,
                });
            }

            CMD_APPEND_CHILD => {
                let node_id = reader.read_u32()?;
                let (child_tree, _) = read_hast_payload(&mut reader, &arena, node_id)?;
                patches.push(Patch::AppendChild {
                    node_id,
                    child_tree,
                });
            }

            CMD_WRAP => {
                let node_id = reader.read_u32()?;
                let (parent_tree, _) = read_hast_payload(&mut reader, &arena, node_id)?;
                patches.push(Patch::Wrap {
                    node_id,
                    parent_tree,
                });
            }

            CMD_REPLACE => {
                let node_id = reader.read_u32()?;
                let (new_tree, keep_children) = read_hast_payload(&mut reader, &arena, node_id)?;
                patches.push(Patch::Replace {
                    node_id,
                    new_tree,
                    keep_children,
                });
            }

            CMD_SET_CHILDREN => {
                let node_id = reader.read_u32()?;
                let (new_children, _) = read_hast_payload(&mut reader, &arena, node_id)?;
                patches.push(Patch::SetChildren {
                    node_id,
                    new_children,
                });
            }

            other => return Err(CommandError::UnknownCommand(other)),
        }
    }

    if patches.is_empty() {
        Ok((arena, Vec::new()))
    } else {
        let result = satteri_ast::rebuild::rebuild_lenient(&arena, &patches)?;
        Ok((result.arena, result.dropped))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use satteri_ast::shared::PROP_INT;

    fn op_open(b: &mut Vec<u8>, t: MdastNodeType) {
        b.push(OP_OPEN);
        b.push(t as u8);
    }
    fn op_close(b: &mut Vec<u8>) {
        b.push(OP_CLOSE);
    }
    fn op_str(b: &mut Vec<u8>, field: u8, s: &str) {
        b.push(OP_STR);
        b.push(field);
        b.extend_from_slice(&(s.len() as u32).to_le_bytes());
        b.extend_from_slice(s.as_bytes());
    }
    fn op_u8(b: &mut Vec<u8>, field: u8, v: u8) {
        b.push(OP_U8);
        b.push(field);
        b.push(v);
    }

    #[test]
    fn opstream_replay_builds_subtree() {
        // blockquote > [ heading(3) > text("Note"), paragraph > text("Body") ]
        let mut ops = Vec::new();
        op_open(&mut ops, MdastNodeType::Blockquote);
        op_open(&mut ops, MdastNodeType::Heading);
        op_u8(&mut ops, OF_DEPTH, 3);
        op_open(&mut ops, MdastNodeType::Text);
        op_str(&mut ops, OF_VALUE, "Note");
        op_close(&mut ops);
        op_close(&mut ops);
        op_open(&mut ops, MdastNodeType::Paragraph);
        op_open(&mut ops, MdastNodeType::Text);
        op_str(&mut ops, OF_VALUE, "Body");
        op_close(&mut ops);
        op_close(&mut ops);
        op_close(&mut ops);

        let empty = ArenaBuilder::<Mdast>::new(String::new()).finish();
        let arena = replay_mdast_opstream(&ops, &empty, 0).unwrap();

        // node 0 = blockquote with 2 children
        assert_eq!(arena.get_node(0).node_type, MdastNodeType::Blockquote as u8);
        let top = arena.get_children(0).to_vec();
        assert_eq!(top.len(), 2);
        // heading depth 3, child text "Note"
        let h = top[0];
        assert_eq!(arena.get_node(h).node_type, MdastNodeType::Heading as u8);
        assert_eq!(decode_heading_data(arena.get_type_data(h)).depth, 3);
        let h_text = arena.get_children(h)[0];
        assert_eq!(arena.get_node(h_text).node_type, MdastNodeType::Text as u8);
        let sref = decode_string_ref_data(arena.get_type_data(h_text));
        assert_eq!(arena.get_str(sref), "Note");
        // paragraph > text "Body"
        let p = top[1];
        assert_eq!(arena.get_node(p).node_type, MdastNodeType::Paragraph as u8);
        let p_text = arena.get_children(p)[0];
        assert_eq!(
            arena.get_str(decode_string_ref_data(arena.get_type_data(p_text))),
            "Body"
        );
    }

    #[test]
    fn opstream_replay_rejects_unbalanced_close() {
        let empty = ArenaBuilder::<Mdast>::new(String::new()).finish();
        let err = replay_mdast_opstream(&[OP_CLOSE], &empty, 0).unwrap_err();
        assert!(matches!(err, CommandError::UnbalancedOpstream));

        let empty_hast = ArenaBuilder::<Hast>::new(String::new()).finish();
        let err = replay_hast_opstream(&[OP_CLOSE], &empty_hast, 0).unwrap_err();
        assert!(matches!(err, CommandError::UnbalancedOpstream));

        // A balanced prefix doesn't excuse a trailing extra close.
        let mut ops = Vec::new();
        op_open(&mut ops, MdastNodeType::Paragraph);
        op_close(&mut ops);
        op_close(&mut ops);
        let err = replay_mdast_opstream(&ops, &empty, 0).unwrap_err();
        assert!(matches!(err, CommandError::UnbalancedOpstream));
    }

    #[test]
    fn opstream_replay_rejects_unclosed_node() {
        // A truncated stream leaves its OPENed nodes on the stack; finishing
        // would hand back nodes with empty type_data.
        let mut ops = Vec::new();
        op_open(&mut ops, MdastNodeType::Heading);
        op_u8(&mut ops, OF_DEPTH, 2);

        let empty = ArenaBuilder::<Mdast>::new(String::new()).finish();
        let err = replay_mdast_opstream(&ops, &empty, 0).unwrap_err();
        assert!(matches!(err, CommandError::UnbalancedOpstream));

        let hast_ops = vec![OP_OPEN, HastNodeType::Element as u8];
        let empty_hast = ArenaBuilder::<Hast>::new(String::new()).finish();
        let err = replay_hast_opstream(&hast_ops, &empty_hast, 0).unwrap_err();
        assert!(matches!(err, CommandError::UnbalancedOpstream));
    }

    #[test]
    fn opstream_keep_children_rejects_out_of_range_anchor() {
        let orig = test_parse_markdown("Hello");
        let bad_anchor = orig.len() as u32;

        let mut ops = Vec::new();
        op_open(&mut ops, MdastNodeType::Heading);
        ops.push(OP_KEEP_CHILDREN);
        op_close(&mut ops);

        let err = replay_mdast_opstream(&ops, &orig, bad_anchor).unwrap_err();
        assert!(matches!(err, CommandError::InvalidNodeId(id) if id == bad_anchor));
    }

    #[test]
    fn set_property_rejects_out_of_range_node_id() {
        let arena = build_hello_world();
        let bad_id = arena.len() as u32;
        let mut buf = Vec::new();
        push_set_property(&mut buf, bad_id, PROP_INT, "depth", "3");
        let err = apply_mdast_commands(arena, &buf, &test_parse_markdown).unwrap_err();
        assert!(matches!(err, CommandError::InvalidNodeId(id) if id == bad_id));

        let hast = build_hast_element(&[]);
        let bad_id = hast.len() as u32;
        let mut buf = Vec::new();
        push_set_property(&mut buf, bad_id, PROP_STRING, "class", "x");
        let err = apply_hast_commands(hast, &buf).unwrap_err();
        assert!(matches!(err, CommandError::InvalidNodeId(id) if id == bad_id));
    }

    #[test]
    fn opstream_replay_rejects_unknown_tags() {
        let empty = ArenaBuilder::<Mdast>::new(String::new()).finish();
        let err = replay_mdast_opstream(&[OP_OPEN, 200], &empty, 0).unwrap_err();
        assert!(matches!(err, CommandError::UnknownNodeType(_)));

        let empty_hast = ArenaBuilder::<Hast>::new(String::new()).finish();
        let err = replay_hast_opstream(&[OP_OPEN, 200], &empty_hast, 0).unwrap_err();
        assert!(matches!(err, CommandError::UnknownNodeType(_)));
    }

    #[test]
    fn opstream_keep_children_splices_original_children() {
        // orig: root > paragraph > text("Hello"); replace the paragraph with
        // heading(2) keeping its children.
        let orig = test_parse_markdown("Hello");
        let para = orig.get_children(0)[0];
        let orig_text = orig.get_children(para)[0];

        let mut ops = Vec::new();
        op_open(&mut ops, MdastNodeType::Heading);
        op_u8(&mut ops, OF_DEPTH, 2);
        ops.push(OP_KEEP_CHILDREN);
        op_close(&mut ops);

        let arena = replay_mdast_opstream(&ops, &orig, para).unwrap();
        assert_eq!(arena.get_node(0).node_type, MdastNodeType::Heading as u8);
        assert_eq!(decode_heading_data(arena.get_type_data(0)).depth, 2);
        let children = arena.get_children(0).to_vec();
        assert_eq!(children.len(), 1);
        assert_eq!(arena.get_node(children[0]).node_type, REF_NODE_TYPE);
        assert_eq!(
            u32::from_le_bytes(arena.get_type_data(children[0]).try_into().unwrap()),
            orig_text
        );
    }

    #[test]
    fn opstream_ref_rejects_out_of_range_id() {
        // A stale id (a node cached across passes) must error at decode, not
        // panic inside the rebuild's arena indexing.
        let orig = test_parse_markdown("Hello");
        let bad = orig.len() as u32 + 100;

        let mut ops = Vec::new();
        op_open(&mut ops, MdastNodeType::Paragraph);
        ops.push(OP_REF);
        ops.extend_from_slice(&bad.to_le_bytes());
        op_close(&mut ops);

        let err = replay_mdast_opstream(&ops, &orig, 0).unwrap_err();
        assert!(matches!(err, CommandError::InvalidNodeId(id) if id == bad));
    }

    #[test]
    fn opstream_rejects_nested_root() {
        let empty = ArenaBuilder::<Mdast>::new(String::new()).finish();

        // Top-level root is the set-children wrapper and must pass.
        let mut ok = Vec::new();
        op_open(&mut ok, MdastNodeType::Root);
        op_close(&mut ok);
        assert!(replay_mdast_opstream(&ok, &empty, 0).is_ok());

        let mut ops = Vec::new();
        op_open(&mut ops, MdastNodeType::Root);
        op_open(&mut ops, MdastNodeType::Root);
        let err = replay_mdast_opstream(&ops, &empty, 0).unwrap_err();
        assert!(matches!(err, CommandError::UnencodableNodeType("root")));
    }

    #[test]
    fn hast_opstream_rejects_doctype() {
        let empty = ArenaBuilder::<Hast>::new(String::new()).finish();
        let ops = vec![OP_OPEN, HastNodeType::Doctype as u8, OP_CLOSE];
        let err = replay_hast_opstream(&ops, &empty, 0).unwrap_err();
        assert!(matches!(err, CommandError::UnencodableNodeType("doctype")));
    }

    #[test]
    fn opstream_rejects_over_deep_nesting() {
        // The rebuild splices replayed content recursively, so unbounded
        // nesting would overflow the host stack (an abort napi can't catch).
        let empty = ArenaBuilder::<Mdast>::new(String::new()).finish();
        let mut ops = Vec::new();
        for _ in 0..(MAX_OPSTREAM_DEPTH + 1) {
            op_open(&mut ops, MdastNodeType::Blockquote);
        }
        let err = replay_mdast_opstream(&ops, &empty, 0).unwrap_err();
        assert!(matches!(err, CommandError::OpstreamTooDeep(_)));
    }

    #[test]
    fn set_property_rejects_out_of_range_or_unparseable_int() {
        // build_hello_world: root(0) > heading(1) > text(2), paragraph > text.
        let heading_id = 1;

        let mut buf = Vec::new();
        push_set_property(&mut buf, heading_id, PROP_INT, "depth", "9999");
        let err =
            apply_mdast_commands(build_hello_world(), &buf, &test_parse_markdown).unwrap_err();
        assert!(matches!(err, CommandError::PropertyValueOutOfRange { .. }));

        let mut buf = Vec::new();
        push_set_property(&mut buf, heading_id, PROP_INT, "depth", "not-a-number");
        let err =
            apply_mdast_commands(build_hello_world(), &buf, &test_parse_markdown).unwrap_err();
        assert!(matches!(err, CommandError::PropertyValueOutOfRange { .. }));

        // The boundary itself still writes.
        let mut buf = Vec::new();
        push_set_property(&mut buf, heading_id, PROP_INT, "depth", "255");
        let arena = apply_mdast_commands(build_hello_world(), &buf, &test_parse_markdown).unwrap();
        assert_eq!(
            decode_heading_data(arena.get_type_data(heading_id)).depth,
            255
        );
    }

    #[cfg(feature = "mdx")]
    #[test]
    fn mdx_jsx_set_property_replaces_named_attrs_and_keeps_explicit() {
        use satteri_ast::shared::MDX_ATTR_EXPRESSION_PROP;

        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node(HastNodeType::Root as u8);
        b.open_node(HastNodeType::MdxJsxElement as u8);
        let elem_name = b.alloc_string("Box");
        let foo = b.alloc_string("foo");
        let expr = b.alloc_string("1+1");
        let rest = b.alloc_string("rest");
        let attrs = vec![
            (MDX_ATTR_EXPRESSION_PROP, foo, expr),
            (MDX_ATTR_SPREAD, StringRef::empty(), rest),
        ];
        b.set_data_current(&encode_mdx_jsx_element_data(elem_name, &attrs, true));
        b.close_node();
        b.close_node();
        let mut arena = b.finish();

        // Replaces the expression-valued `foo` (no duplicate) and appends
        // after the spread so the write wins.
        apply_hast_mdx_jsx_attribute(&mut arena, 1, "foo", PROP_STRING, "x").unwrap();
        let data = arena.get_type_data(1).to_vec();
        assert_eq!(decode_mdx_jsx_attr_count(&data), 2);
        assert!(decode_mdx_jsx_explicit(&data));
        let (k0, _, _) = decode_mdx_jsx_attr(&data, 0);
        assert_eq!(k0, MDX_ATTR_SPREAD);
        let (k1, n1, v1) = decode_mdx_jsx_attr(&data, 1);
        assert_eq!(k1, MDX_ATTR_LITERAL_PROP);
        assert_eq!(arena.get_str(n1), "foo");
        assert_eq!(arena.get_str(v1), "x");

        // Appending a brand-new attribute must not clear the explicit flag.
        apply_hast_mdx_jsx_attribute(&mut arena, 1, "id", PROP_STRING, "intro").unwrap();
        let data = arena.get_type_data(1).to_vec();
        assert_eq!(decode_mdx_jsx_attr_count(&data), 3);
        assert!(decode_mdx_jsx_explicit(&data));
    }

    fn test_parse_markdown(source: &str) -> Arena<Mdast> {
        let mut b = ArenaBuilder::<Mdast>::new(String::new());
        b.open_node(MdastNodeType::Root as u8);
        b.open_node(MdastNodeType::Paragraph as u8);
        b.open_node(MdastNodeType::Text as u8);
        let sref = b.alloc_string(source);
        b.set_data_current(&satteri_arena::encode_string_ref_data(sref));
        b.close_node();
        b.close_node();
        b.close_node();
        b.finish()
    }

    fn push_u32(buf: &mut Vec<u8>, v: u32) {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Encode a CMD_SET_PROPERTY command into a buffer.
    fn push_set_property(buf: &mut Vec<u8>, node_id: u32, value_type: u8, name: &str, value: &str) {
        buf.push(CMD_SET_PROPERTY);
        push_u32(buf, node_id);
        buf.push(value_type);
        push_u32(buf, name.len() as u32);
        buf.extend_from_slice(name.as_bytes());
        push_u32(buf, value.len() as u32);
        buf.extend_from_slice(value.as_bytes());
    }

    fn build_hello_world() -> Arena<Mdast> {
        use satteri_ast::mdast::codec::{encode_heading_data, encode_string_ref_data};

        let source = "# Hello\n\nWorld".to_string();
        let mut b = ArenaBuilder::<Mdast>::new(source);

        b.open_node(MdastNodeType::Root as u8);
        b.set_position_current(0, 14, 1, 1, 2, 6);

        b.open_node(MdastNodeType::Heading as u8);
        b.set_position_current(0, 7, 1, 1, 1, 8);
        b.set_data_current(&encode_heading_data(1));

        b.open_node(MdastNodeType::Text as u8);
        b.set_position_current(2, 7, 1, 3, 1, 8);
        b.set_data_current(&encode_string_ref_data(StringRef::new(2, 5)));
        b.close_node();

        b.close_node();

        b.open_node(MdastNodeType::Paragraph as u8);
        b.set_position_current(9, 14, 2, 1, 2, 6);

        b.open_node(MdastNodeType::Text as u8);
        b.set_position_current(9, 14, 2, 1, 2, 6);
        b.set_data_current(&encode_string_ref_data(StringRef::new(9, 5)));
        b.close_node();

        b.close_node();
        b.close_node();

        b.finish()
    }

    #[test]
    fn empty_command_buffer() {
        let arena = build_hello_world();
        let result = apply_mdast_commands(arena.clone(), &[], &test_parse_markdown).unwrap();
        assert_eq!(result.len(), arena.len());
    }

    #[test]
    fn remove_command() {
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];
        let mut buf = Vec::new();
        buf.push(CMD_REMOVE);
        push_u32(&mut buf, heading_id);

        let result = apply_mdast_commands(arena.clone(), &buf, &test_parse_markdown).unwrap();
        assert_eq!(result.get_children(0).len(), 1);
        assert_eq!(
            result.get_node(result.get_children(0)[0]).node_type,
            MdastNodeType::Paragraph as u8
        );
    }

    #[test]
    fn set_property_heading_depth() {
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];

        let mut buf = Vec::new();
        push_set_property(&mut buf, heading_id, PROP_INT, "depth", "3");

        let result = apply_mdast_commands(arena.clone(), &buf, &test_parse_markdown).unwrap();
        let heading_data = result.get_type_data(heading_id);
        let heading = decode_heading_data(heading_data);
        assert_eq!(heading.depth, 3);
    }

    #[test]
    fn set_property_text_value() {
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];
        let text_id = arena.get_children(heading_id)[0];

        let mut buf = Vec::new();
        push_set_property(&mut buf, text_id, PROP_STRING, "value", "Goodbye");

        let result = apply_mdast_commands(arena.clone(), &buf, &test_parse_markdown).unwrap();
        let text_data = result.get_type_data(text_id);
        let sref = decode_string_ref_data(text_data);
        assert_eq!(result.get_str(sref), "Goodbye");
    }

    #[test]
    fn replace_with_raw_markdown() {
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];

        let raw_md = "## New Heading";
        let mut buf = Vec::new();
        buf.push(CMD_REPLACE);
        push_u32(&mut buf, heading_id);
        buf.push(PAYLOAD_RAW_MARKDOWN);
        push_u32(&mut buf, raw_md.len() as u32);
        buf.extend_from_slice(raw_md.as_bytes());

        let result = apply_mdast_commands(arena.clone(), &buf, &test_parse_markdown).unwrap();
        let root_children = result.get_children(0);
        assert!(root_children.len() >= 2);
    }

    #[test]
    fn multiple_commands() {
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];
        let text_id = arena.get_children(heading_id)[0];

        let mut buf = Vec::new();
        push_set_property(&mut buf, heading_id, PROP_INT, "depth", "3");
        push_set_property(&mut buf, text_id, PROP_STRING, "value", "Hi");

        let result = apply_mdast_commands(arena.clone(), &buf, &test_parse_markdown).unwrap();

        let heading_data = result.get_type_data(heading_id);
        assert_eq!(decode_heading_data(heading_data).depth, 3);

        let text_data = result.get_type_data(text_id);
        let sref = decode_string_ref_data(text_data);
        assert_eq!(result.get_str(sref), "Hi");
    }

    #[test]
    fn set_property_null() {
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];
        let text_id = arena.get_children(heading_id)[0];

        let mut buf = Vec::new();
        push_set_property(&mut buf, text_id, PROP_NULL, "value", "");

        let result = apply_mdast_commands(arena.clone(), &buf, &test_parse_markdown).unwrap();
        let text_data = result.get_type_data(text_id);
        let sref = decode_string_ref_data(text_data);
        assert_eq!(sref.len, 0);
    }

    #[test]
    fn set_property_invalid_field_reports_property_and_node_type() {
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];

        let mut buf = Vec::new();
        push_set_property(&mut buf, heading_id, PROP_STRING, "value", "x");

        let err = apply_mdast_commands(arena, &buf, &test_parse_markdown).unwrap_err();
        assert!(matches!(
            err,
            CommandError::UnknownField { ref name, ref node_type }
                if name == "value" && node_type == "heading"
        ));
        assert_eq!(
            err.to_string(),
            "cannot set property 'value' on a 'heading' node"
        );
    }

    #[test]
    fn set_property_wrong_value_type_reports_value_mismatch() {
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];

        // `depth` is a valid heading field, but it holds an int, not a string.
        let mut buf = Vec::new();
        push_set_property(&mut buf, heading_id, PROP_STRING, "depth", "3");

        let err = apply_mdast_commands(arena, &buf, &test_parse_markdown).unwrap_err();
        assert!(matches!(
            err,
            CommandError::InvalidPropertyValue { ref name, ref node_type }
                if name == "depth" && node_type == "heading"
        ));
        assert_eq!(
            err.to_string(),
            "property 'depth' on a 'heading' node cannot hold a value of this type"
        );
    }

    /// Build root > one leaf node of `node_type` carrying `type_data`.
    fn build_single_node(node_type: MdastNodeType, type_data: &[u8]) -> Arena<Mdast> {
        let mut b = ArenaBuilder::<Mdast>::new(String::new());
        b.open_node(MdastNodeType::Root as u8);
        b.open_node(node_type as u8);
        b.set_data_current(type_data);
        b.close_node();
        b.close_node();
        b.finish()
    }

    #[test]
    fn set_property_image_reference_alt_roundtrip() {
        let mut b = ArenaBuilder::<Mdast>::new(String::new());
        b.open_node(MdastNodeType::Root as u8);
        b.open_node(MdastNodeType::ImageReference as u8);
        let identifier = b.alloc_string("img");
        let alt = b.alloc_string("old");
        b.set_data_current(&encode_image_reference_data(identifier, identifier, 0, alt));
        b.close_node();
        b.close_node();
        let arena = b.finish();
        let image_ref_id = arena.get_children(0)[0];

        let mut buf = Vec::new();
        push_set_property(&mut buf, image_ref_id, PROP_STRING, "alt", "new alt");

        let result = apply_mdast_commands(arena, &buf, &test_parse_markdown).unwrap();
        let alt = decode_image_reference_alt(result.get_type_data(image_ref_id));
        assert_eq!(result.get_str(alt), "new alt");
    }

    #[test]
    fn set_property_reference_type_valid_and_invalid() {
        let mut b = ArenaBuilder::<Mdast>::new(String::new());
        b.open_node(MdastNodeType::Root as u8);
        b.open_node(MdastNodeType::LinkReference as u8);
        let identifier = b.alloc_string("ref");
        b.set_data_current(&encode_reference_data(identifier, identifier, 0));
        b.close_node();
        b.close_node();
        let arena = b.finish();
        let link_ref_id = arena.get_children(0)[0];

        let mut buf = Vec::new();
        push_set_property(&mut buf, link_ref_id, PROP_STRING, "referenceType", "full");
        let result = apply_mdast_commands(arena.clone(), &buf, &test_parse_markdown).unwrap();
        let reference = decode_reference_data(result.get_type_data(link_ref_id));
        assert_eq!(reference.reference_kind, 2);

        // A value outside the declared list is a value error, not a silent 0.
        let mut buf = Vec::new();
        push_set_property(&mut buf, link_ref_id, PROP_STRING, "referenceType", "bogus");
        let err = apply_mdast_commands(arena, &buf, &test_parse_markdown).unwrap_err();
        assert!(matches!(
            err,
            CommandError::InvalidPropertyValue { ref name, ref node_type }
                if name == "referenceType" && node_type == "linkReference"
        ));
    }

    #[test]
    fn set_property_list_start_and_ordered() {
        let arena = build_single_node(MdastNodeType::List, &encode_list_data(false, 1, false));
        let list_id = arena.get_children(0)[0];

        let mut buf = Vec::new();
        push_set_property(&mut buf, list_id, PROP_INT, "start", "5");
        push_set_property(&mut buf, list_id, PROP_BOOL_TRUE, "ordered", "");

        let result = apply_mdast_commands(arena, &buf, &test_parse_markdown).unwrap();
        let list = decode_list_data(result.get_type_data(list_id));
        assert_eq!(list.start, 5);
        assert!(list.ordered);
        assert!(!list.spread);
    }

    #[test]
    fn escape_braces_in_html_text_basic() {
        assert_eq!(
            escape_braces_in_html_text("<span>{foo: 1}</span>"),
            "<span>{'{'}foo: 1{'}'}</span>"
        );
    }

    #[test]
    fn escape_braces_preserves_attributes() {
        let result = escape_braces_in_html_text(r#"<span data-x="{a}">{b}</span>"#);
        assert!(
            result.contains(r#"data-x="{a}""#),
            "attribute braces preserved"
        );
        assert!(result.contains("{'{'}"), "text braces escaped");
    }

    #[test]
    fn escape_braces_no_braces() {
        let html = r#"<pre class="shiki"><code><span style="color:red">hello</span></code></pre>"#;
        assert_eq!(escape_braces_in_html_text(html), html);
    }

    #[test]
    fn escape_braces_shiki_output() {
        let html = r#"<pre class="shiki"><code><span style="color:#E1E4E8">const x = </span><span style="color:#B392F0">{</span><span style="color:#E1E4E8">foo: 1</span><span style="color:#B392F0">}</span></code></pre>"#;
        let escaped = escape_braces_in_html_text(html);
        assert!(
            !escaped.contains(">{<"),
            "bare braces in text should be escaped"
        );
        assert!(
            !escaped.contains(">}<"),
            "bare braces in text should be escaped"
        );
        assert!(escaped.contains(r#"class="shiki""#));
        assert!(escaped.contains(r#"style="color:#E1E4E8""#));
    }

    #[test]
    fn hast_set_property_add_new() {
        let arena = build_hast_element(&[]);
        let element_id = arena.get_children(0)[0];

        let mut buf = Vec::new();
        push_set_property(&mut buf, element_id, PROP_STRING, "class", "test");

        let result = apply_hast_commands(arena.clone(), &buf).unwrap();
        let data = result.get_type_data(element_id);
        let prop_count = u32::from_le_bytes(data[8..12].try_into().unwrap());
        assert_eq!(prop_count, 1);
        let name_ref = StringRef::new(
            u32::from_le_bytes(data[16..20].try_into().unwrap()),
            u32::from_le_bytes(data[20..24].try_into().unwrap()),
        );
        assert_eq!(result.get_str(name_ref), "class");
        let val_ref = StringRef::new(
            u32::from_le_bytes(data[28..32].try_into().unwrap()),
            u32::from_le_bytes(data[32..36].try_into().unwrap()),
        );
        assert_eq!(result.get_str(val_ref), "test");
        assert_eq!(data[24], PROP_STRING);
    }

    #[test]
    fn hast_set_property_overwrite_existing() {
        let arena = build_hast_element(&[("class", PROP_STRING, "old")]);
        let element_id = arena.get_children(0)[0];

        let mut buf = Vec::new();
        push_set_property(&mut buf, element_id, PROP_STRING, "class", "new-value");

        let result = apply_hast_commands(arena.clone(), &buf).unwrap();
        let data = result.get_type_data(element_id);
        let prop_count = u32::from_le_bytes(data[8..12].try_into().unwrap());
        assert_eq!(prop_count, 1);
        let val_ref = StringRef::new(
            u32::from_le_bytes(data[28..32].try_into().unwrap()),
            u32::from_le_bytes(data[32..36].try_into().unwrap()),
        );
        assert_eq!(result.get_str(val_ref), "new-value");
    }

    #[test]
    fn hast_set_property_bool_true() {
        let arena = build_hast_element(&[]);
        let element_id = arena.get_children(0)[0];

        let mut buf = Vec::new();
        push_set_property(&mut buf, element_id, PROP_BOOL_TRUE, "disabled", "");

        let result = apply_hast_commands(arena.clone(), &buf).unwrap();
        let data = result.get_type_data(element_id);
        let prop_count = u32::from_le_bytes(data[8..12].try_into().unwrap());
        assert_eq!(prop_count, 1);
        assert_eq!(data[24], PROP_BOOL_TRUE);
    }

    #[test]
    fn hast_set_property_multiple_on_same_node() {
        let arena = build_hast_element(&[]);
        let element_id = arena.get_children(0)[0];

        let mut buf = Vec::new();
        push_set_property(&mut buf, element_id, PROP_STRING, "class", "foo");
        push_set_property(&mut buf, element_id, PROP_STRING, "id", "bar");

        let result = apply_hast_commands(arena.clone(), &buf).unwrap();
        let data = result.get_type_data(element_id);
        let prop_count = u32::from_le_bytes(data[8..12].try_into().unwrap());
        assert_eq!(prop_count, 2);
    }

    /// Build a minimal HAST element arena: root(type 0) → element(type 1, tag "div")
    fn build_hast_element(props: &[(&str, u8, &str)]) -> Arena<Hast> {
        use satteri_ast::hast::node::HastNodeType;

        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node_raw(HastNodeType::Root as u8);
        b.open_node_raw(HastNodeType::Element as u8);
        let tag_ref = b.alloc_string("div");
        let prop_tuples: Vec<(StringRef, u8, StringRef)> = props
            .iter()
            .map(|(name, kind, value)| {
                let n = b.alloc_string(name);
                let v = if value.is_empty() {
                    StringRef::empty()
                } else {
                    b.alloc_string(value)
                };
                (n, *kind, v)
            })
            .collect();
        let mut type_data = Vec::with_capacity(16 + prop_tuples.len() * 20);
        type_data.extend_from_slice(&tag_ref.offset.to_le_bytes());
        type_data.extend_from_slice(&tag_ref.len.to_le_bytes());
        type_data.extend_from_slice(&(prop_tuples.len() as u32).to_le_bytes());
        type_data.extend_from_slice(&0u32.to_le_bytes());
        for (n, kind, v) in &prop_tuples {
            type_data.extend_from_slice(&n.offset.to_le_bytes());
            type_data.extend_from_slice(&n.len.to_le_bytes());
            type_data.push(*kind);
            type_data.extend_from_slice(&[0u8; 3]);
            type_data.extend_from_slice(&v.offset.to_le_bytes());
            type_data.extend_from_slice(&v.len.to_le_bytes());
        }
        b.set_data_current(&type_data);
        b.close_node();
        b.close_node();
        b.finish()
    }
}
