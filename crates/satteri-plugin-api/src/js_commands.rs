//! Binary command buffer parser and mutation applicator.
//!
//! Reads a command buffer produced by the JS `CommandBuffer` class, converts
//! commands into arena mutations, and returns the rebuilt arena.
//!
//! ## Wire format
//!
//! All multi-byte integers are **little-endian**.
//!
//! Commands (first byte):
//!   0x01  REMOVE           [nodeId: u32]
//!   0x05  INSERT_BEFORE    [nodeId: u32][payloadType: u8][payload...]
//!   0x06  INSERT_AFTER     [nodeId: u32][payloadType: u8][payload...]
//!   0x07  PREPEND_CHILD    [nodeId: u32][payloadType: u8][payload...]
//!   0x08  APPEND_CHILD     [nodeId: u32][payloadType: u8][payload...]
//!   0x09  WRAP             [nodeId: u32][payloadType: u8][payload...]
//!   0x0B  REPLACE          [nodeId: u32][payloadType: u8][payload...]
//!   0x0C  SET_PROPERTY     [nodeId: u32][valueType: u8][nameLen: u32][name...][valueLen: u32][value...]
//!
//! Value types for SET_PROPERTY (must match `shared::PROP_*` and the JS
//! `command-buffer.ts` constants):
//!   0  STRING     : UTF-8 value
//!   1  BOOL_TRUE  : no value bytes
//!   2  BOOL_FALSE : no value bytes
//!   3  SPACE_SEP  : space-separated list (UTF-8)
//!   4  COMMA_SEP  : comma-separated list (UTF-8)
//!   5  INT        : value is decimal string, parsed to i64
//!   6  NULL       : no value bytes
//!
//! Payload types:
//!   0x10  RAW_MARKDOWN     [len: u32][utf8...]
//!   0x11  RAW_HTML         [len: u32][utf8...]
//!   0x12  SERDE_JSON       [len: u32][utf8...]
//!
//! The MDAST and HAST command paths are deliberately separate functions
//! (`apply_mdast_commands`, `apply_hast_commands`). Numeric `node_type`
//! values overlap between the two arenas (e.g. mdast Paragraph=1 collides
//! with HastNodeType::Element=1), so a single dispatcher trying to handle
//! both kinds would silently misroute nodes. The phantom-typed `Arena<K>`
//! signature on each entry point makes a cross-kind call a compile error.

use satteri_arena::{Arena, ArenaBuilder, ArenaKind, Hast, Mdast, StringRef};
use satteri_ast::commands::{CommandError, JsNode};
use satteri_ast::hast::HastNodeType;
use satteri_ast::mdast::codec::*;
use satteri_ast::mdast::MdastNodeType;
use satteri_ast::rebuild::{Patch, REF_NODE_TYPE};
#[cfg(feature = "mdx")]
use satteri_ast::shared::encode_js_jsx_attrs;
#[cfg(feature = "mdx")]
use satteri_ast::shared::{MDX_ATTR_BOOLEAN_PROP, MDX_ATTR_SPREAD};
use satteri_ast::shared::{
    PROP_BOOL_FALSE, PROP_BOOL_TRUE, PROP_INT, PROP_NULL, PROP_SPACE_SEP, PROP_STRING,
};

// Must match packages/satteri/src/command-buffer.ts
const CMD_REMOVE: u8 = 0x01;
const CMD_INSERT_BEFORE: u8 = 0x05;
const CMD_INSERT_AFTER: u8 = 0x06;
const CMD_PREPEND_CHILD: u8 = 0x07;
const CMD_APPEND_CHILD: u8 = 0x08;
const CMD_WRAP: u8 = 0x09;
const CMD_REPLACE: u8 = 0x0B;
const CMD_SET_PROPERTY: u8 = 0x0C;

const PAYLOAD_RAW_MARKDOWN: u8 = 0x10;
const PAYLOAD_RAW_HTML: u8 = 0x11;
const PAYLOAD_SERDE_JSON: u8 = 0x12;
/// Imperative-builder op-stream: a serialized ArenaBuilder program the JS `m`
/// builder emits, replayed directly into the arena (no `JsNode`). See
/// `replay_mdast_opstream`.
const PAYLOAD_OPSTREAM: u8 = 0x14;

// Op-stream op codes (must match the JS `MdastBuilder`).
const OP_OPEN: u8 = 0x01; // [type: u8]
const OP_CLOSE: u8 = 0x02;
const OP_REF: u8 = 0x03; // [id: u32 LE] — splice an existing node
const OP_KEEP_CHILDREN: u8 = 0x04; // splice the anchor node's original children
const OP_STR: u8 = 0x05; // [field: u8][len: u32 LE][utf8]
const OP_U8: u8 = 0x06; // [field: u8][value: u8]
const OP_U32: u8 = 0x07; // [field: u8][value: u32 LE]
const OP_BOOL: u8 = 0x08; // [field: u8][0|1]
const OP_DATA: u8 = 0x09; // [len: u32 LE][json utf8]
const OP_PROP: u8 = 0x0a; // [name str][kind: u8][value str] — HAST element property
const OP_ALIGN: u8 = 0x0b; // [len: u32 LE][ColumnAlign bytes] — table column alignment

// Op-stream field ids (single namespace across OP_STR/OP_U8/OP_U32/OP_BOOL).
const OF_VALUE: u8 = 0;
const OF_URL: u8 = 1;
const OF_TITLE: u8 = 2;
const OF_ALT: u8 = 3;
const OF_LANG: u8 = 4;
const OF_META: u8 = 5;
const OF_IDENTIFIER: u8 = 6;
const OF_LABEL: u8 = 7;
const OF_NAME: u8 = 8; // directive / MDX JSX element name
const OF_REFERENCE_TYPE: u8 = 9;
const OF_DEPTH: u8 = 10;
const OF_CHECKED: u8 = 11;
const OF_START: u8 = 12;
const OF_ORDERED: u8 = 13;
const OF_SPREAD: u8 = 14;
const OF_TAGNAME: u8 = 15; // HAST element tag name
const OF_EXPLICIT: u8 = 16; // MDX JSX `_mdxExplicitJsx` flag

// MDAST field IDs: internal to the set_string_ref / resolve_mdast_field dispatch
const FIELD_DEPTH: u16 = 0x0001;
const FIELD_URL: u16 = 0x0010;
const FIELD_TITLE: u16 = 0x0011;
const FIELD_LANG: u16 = 0x0020;
const FIELD_META: u16 = 0x0021;
const FIELD_VALUE: u16 = 0x0022;
const FIELD_ALT: u16 = 0x0030;
const FIELD_ORDERED: u16 = 0x0040;
const FIELD_START: u16 = 0x0041;
const FIELD_SPREAD: u16 = 0x0042;
const FIELD_CHECKED: u16 = 0x0050;
const FIELD_IDENTIFIER: u16 = 0x0060;
const FIELD_LABEL: u16 = 0x0061;
const FIELD_REFERENCE_TYPE: u16 = 0x0062;
const FIELD_NAME: u16 = 0x0070;

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

/// Whether a plugin-supplied `data` blob carries the `_mdxExplicitJsx: true`
/// marker — used to set the matching fast-path bit in `MdxJsxElementData`.
#[cfg(feature = "mdx")]
fn js_data_is_mdx_explicit(data: &Option<serde_json::Map<String, serde_json::Value>>) -> bool {
    data.as_ref()
        .and_then(|m| m.get("_mdxExplicitJsx"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
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

/// Resolve an MDAST property name to its field ID for a given node type.
fn resolve_mdast_field(node_type: u8, name: &str) -> Option<u16> {
    match (node_type, name) {
        (2, "depth") => Some(FIELD_DEPTH),
        (8, "lang") => Some(FIELD_LANG),
        (8, "meta") => Some(FIELD_META),
        (8, "value") => Some(FIELD_VALUE),
        (15, "url") => Some(FIELD_URL),
        (15, "title") => Some(FIELD_TITLE),
        (16, "url") => Some(FIELD_URL),
        (16, "alt") => Some(FIELD_ALT),
        (16, "title") => Some(FIELD_TITLE),
        (10 | 13 | 7 | 25 | 26 | 28, "value") => Some(FIELD_VALUE),
        (27, "meta") => Some(FIELD_META),
        (27, "value") => Some(FIELD_VALUE),
        (102..=104, "value") => Some(FIELD_VALUE),
        (9, "url") => Some(FIELD_URL),
        (9, "title") => Some(FIELD_TITLE),
        (5, "ordered") => Some(FIELD_ORDERED),
        (5, "start") => Some(FIELD_START),
        (5 | 6, "spread") => Some(FIELD_SPREAD),
        (6, "checked") => Some(FIELD_CHECKED),
        (9 | 17 | 18 | 19 | 20, "identifier") => Some(FIELD_IDENTIFIER),
        (9 | 17 | 18 | 19 | 20, "label") => Some(FIELD_LABEL),
        (17 | 18 | 20, "referenceType") => Some(FIELD_REFERENCE_TYPE),
        (100 | 101, "name") => Some(FIELD_NAME),
        _ => None,
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
    if prop_name == "data" {
        apply_data_property(arena, node_id, value_type, value_str);
        return Ok(());
    }

    let node_type = arena.get_node(node_id).node_type;
    let field_id =
        resolve_mdast_field(node_type, prop_name).ok_or(CommandError::UnknownField(0))?;

    match value_type {
        PROP_STRING | PROP_SPACE_SEP => {
            let sref = arena.alloc_string(value_str);
            set_mdast_string_ref(arena, node_id, field_id, sref)
        }
        PROP_BOOL_TRUE => apply_mdast_bool(arena, node_id, node_type, field_id, true),
        PROP_BOOL_FALSE => apply_mdast_bool(arena, node_id, node_type, field_id, false),
        PROP_INT => {
            // Integer fields (depth, list start, checked). Accept a float
            // spelling like "3" or "3.0" leniently rather than truncating to 0.
            let value = value_str
                .parse::<i64>()
                .or_else(|_| value_str.parse::<f64>().map(|f| f as i64))
                .unwrap_or(0);
            apply_mdast_int(arena, node_id, node_type, field_id, value)
        }
        PROP_NULL => apply_mdast_null(arena, node_id, node_type, field_id),
        _ => Err(CommandError::UnknownCommand(value_type)),
    }
}

fn apply_mdast_int(
    arena: &mut Arena<Mdast>,
    node_id: u32,
    node_type: u8,
    field_id: u16,
    value: i64,
) -> Result<(), CommandError> {
    let data_offset = arena.get_node(node_id).data_offset as usize;
    let data_len = arena.get_node(node_id).data_len as usize;
    match (node_type, field_id) {
        (2, FIELD_DEPTH) => {
            if data_len >= 1 {
                arena.type_data[data_offset] = value as u8;
            }
        }
        (5, FIELD_START) => {
            if data_len >= 4 {
                arena.type_data[data_offset..data_offset + 4]
                    .copy_from_slice(&(value as u32).to_ne_bytes());
            }
        }
        (6, FIELD_CHECKED) => {
            if data_len >= 1 {
                arena.type_data[data_offset] = value as u8;
            }
        }
        _ => return Err(CommandError::UnknownField(field_id)),
    }
    Ok(())
}

fn apply_mdast_bool(
    arena: &mut Arena<Mdast>,
    node_id: u32,
    node_type: u8,
    field_id: u16,
    value: bool,
) -> Result<(), CommandError> {
    let data_offset = arena.get_node(node_id).data_offset as usize;
    let data_len = arena.get_node(node_id).data_len as usize;
    match (node_type, field_id) {
        (5, FIELD_ORDERED) => {
            if data_len >= 5 {
                arena.type_data[data_offset + 4] = value as u8;
            }
        }
        (5, FIELD_SPREAD) => {
            if data_len >= 6 {
                arena.type_data[data_offset + 5] = value as u8;
            }
        }
        (6, FIELD_SPREAD) => {
            if data_len >= 2 {
                arena.type_data[data_offset + 1] = value as u8;
            }
        }
        _ => return Err(CommandError::UnknownField(field_id)),
    }
    Ok(())
}

fn apply_mdast_null(
    arena: &mut Arena<Mdast>,
    node_id: u32,
    node_type: u8,
    field_id: u16,
) -> Result<(), CommandError> {
    match (node_type, field_id) {
        (6, FIELD_CHECKED) => {
            let data_offset = arena.get_node(node_id).data_offset as usize;
            let data_len = arena.get_node(node_id).data_len as usize;
            if data_len >= 1 {
                arena.type_data[data_offset] = 2;
            }
            Ok(())
        }
        _ => set_mdast_string_ref(arena, node_id, field_id, StringRef::empty()),
    }
}

fn set_mdast_string_ref(
    arena: &mut Arena<Mdast>,
    node_id: u32,
    field_id: u16,
    sref: StringRef,
) -> Result<(), CommandError> {
    let node = arena.get_node(node_id);
    let node_type = node.node_type;
    let data_offset = node.data_offset as usize;

    let ref_offset = match (node_type, field_id) {
        // Text/InlineCode/Html/Yaml/Toml/InlineMath: StringRef at 0
        (10 | 13 | 7 | 25 | 26 | 28, FIELD_VALUE) => 0,
        // Link: LinkData { url: 0, title: 8 }
        (15, FIELD_URL) => 0,
        (15, FIELD_TITLE) => 8,
        // Image: ImageData { url: 0, alt: 8, title: 16 }
        (16, FIELD_URL) => 0,
        (16, FIELD_ALT) => 8,
        (16, FIELD_TITLE) => 16,
        // Code: CodeData { lang: 0, meta: 8, value: 16 }
        (8, FIELD_LANG) => 0,
        (8, FIELD_META) => 8,
        (8, FIELD_VALUE) => 16,
        // Math: MathData { meta: 0, value: 8 }
        (27, FIELD_META) => 0,
        (27, FIELD_VALUE) => 8,
        // Definition: DefinitionData { url: 0, title: 8, identifier: 16, label: 24 }
        (9, FIELD_URL) => 0,
        (9, FIELD_TITLE) => 8,
        (9, FIELD_IDENTIFIER) => 16,
        (9, FIELD_LABEL) => 24,
        // LinkReference/ImageReference/FootnoteReference: ReferenceData { identifier: 0, label: 8 }
        (17 | 18 | 20, FIELD_IDENTIFIER) => 0,
        (17 | 18 | 20, FIELD_LABEL) => 8,
        // FootnoteDefinition: FootnoteDefinitionData { identifier: 0, label: 8 }
        (19, FIELD_IDENTIFIER) => 0,
        (19, FIELD_LABEL) => 8,
        // MdxJsxElement: MdxJsxElementData { name: 0 }
        (100 | 101, FIELD_NAME) => 0,
        // MdxExpression/MdxjsEsm: ExpressionData { value: 0 }
        (102..=104, FIELD_VALUE) => 0,
        _ => return Err(CommandError::UnknownField(field_id)),
    };

    let abs_offset = data_offset + ref_offset;
    let bytes_offset = sref.offset.to_ne_bytes();
    let bytes_len = sref.len.to_ne_bytes();
    arena.type_data[abs_offset..abs_offset + 4].copy_from_slice(&bytes_offset);
    arena.type_data[abs_offset + 4..abs_offset + 8].copy_from_slice(&bytes_len);

    Ok(())
}

fn parse_raw_markdown(
    markdown: &str,
    parse_markdown: &dyn Fn(&str) -> Arena<Mdast>,
) -> Arena<Mdast> {
    parse_markdown(markdown)
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

fn js_node_to_mdast_arena(js_node: &JsNode) -> Result<(Arena<Mdast>, bool), CommandError> {
    if js_node.is_hast {
        return Err(CommandError::UnknownNodeType(format!(
            "expected mdast node, got hast-flagged `{}`",
            js_node.node_type
        )));
    }
    let mut builder = ArenaBuilder::<Mdast>::new(String::new());
    emit_mdast_js_node(js_node, &mut builder)?;
    Ok((builder.finish(), js_node.keep_children))
}

fn js_node_to_hast_arena(js_node: &JsNode) -> Result<(Arena<Hast>, bool), CommandError> {
    if !js_node.is_hast {
        return Err(CommandError::UnknownNodeType(format!(
            "expected hast node, got mdast-flagged `{}`",
            js_node.node_type
        )));
    }
    let mut builder = ArenaBuilder::<Hast>::new(String::new());
    emit_hast_js_node(js_node, &mut builder)?;
    Ok((builder.finish(), js_node.keep_children))
}

/// Emit a reference placeholder: a `REF_NODE_TYPE` node carrying the target
/// original id (u32 LE) in its type_data. The rebuild resolves it by splicing
/// that original subtree and applying any pending patch on it.
fn emit_ref_node<K: ArenaKind>(ref_id: u32, builder: &mut ArenaBuilder<K>) {
    builder.open_node_raw(REF_NODE_TYPE);
    builder.set_data_current(&ref_id.to_le_bytes());
    builder.close_node();
}

fn emit_mdast_js_node(
    js_node: &JsNode,
    builder: &mut ArenaBuilder<Mdast>,
) -> Result<(), CommandError> {
    if let Some(ref_id) = js_node.ref_id {
        emit_ref_node(ref_id, builder);
        return Ok(());
    }

    if js_node.is_hast {
        return Err(CommandError::UnknownNodeType(format!(
            "expected mdast node, got hast-flagged `{}`",
            js_node.node_type
        )));
    }

    let node_type = name_to_node_type(&js_node.node_type)?;
    builder.open_node(node_type as u8);

    let type_data = encode_js_node_data(js_node, node_type, builder);
    if !type_data.is_empty() {
        builder.set_data_current(&type_data);
    }

    write_js_node_data(js_node, builder)?;

    if let Some(children) = &js_node.children {
        for child in children {
            emit_mdast_js_node(child, builder)?;
        }
    }

    builder.close_node();
    Ok(())
}

fn write_js_node_data<K: ArenaKind>(
    js_node: &JsNode,
    builder: &mut ArenaBuilder<K>,
) -> Result<(), CommandError> {
    let Some(data) = &js_node.data else {
        return Ok(());
    };
    let id = builder.current_node_id();
    let json = serde_json::to_vec(data).map_err(|e| CommandError::InvalidJson(e.to_string()))?;
    builder.arena_mut().set_node_data(id, json);
    Ok(())
}

fn encode_js_node_data(
    js_node: &JsNode,
    node_type: MdastNodeType,
    builder: &mut ArenaBuilder<Mdast>,
) -> Vec<u8> {
    match node_type {
        MdastNodeType::Heading => {
            let depth = js_node.depth.unwrap_or(1);
            encode_heading_data(depth)
        }
        MdastNodeType::Text
        | MdastNodeType::InlineCode
        | MdastNodeType::Html
        | MdastNodeType::Yaml
        | MdastNodeType::Toml
        | MdastNodeType::InlineMath => {
            let value = js_node.value.as_deref().unwrap_or("");
            let sref = builder.alloc_string(value);
            encode_string_ref_data(sref)
        }
        MdastNodeType::Code => {
            let lang_ref = alloc_opt_str(builder, js_node.lang.as_deref());
            let meta_ref = alloc_opt_str(builder, js_node.meta.as_deref());
            let value_ref = alloc_opt_str(builder, js_node.value.as_deref());
            encode_code_data(lang_ref, meta_ref, value_ref, b'`')
        }
        MdastNodeType::Math => {
            let meta_ref = alloc_opt_str(builder, js_node.meta.as_deref());
            let value_ref = alloc_opt_str(builder, js_node.value.as_deref());
            encode_math_data(meta_ref, value_ref)
        }
        MdastNodeType::Link => {
            let url_ref = alloc_opt_str(builder, js_node.url.as_deref());
            let title_ref = alloc_opt_str(builder, js_node.title.as_deref());
            encode_link_data(url_ref, title_ref)
        }
        MdastNodeType::Image => {
            let url_ref = alloc_opt_str(builder, js_node.url.as_deref());
            let alt_ref = alloc_opt_str(builder, js_node.alt.as_deref());
            let title_ref = alloc_opt_str(builder, js_node.title.as_deref());
            encode_image_data(url_ref, alt_ref, title_ref)
        }
        MdastNodeType::Definition => {
            let url_ref = alloc_opt_str(builder, js_node.url.as_deref());
            let title_ref = alloc_opt_str(builder, js_node.title.as_deref());
            let id_ref = alloc_opt_str(builder, js_node.identifier.as_deref());
            let label_ref = alloc_opt_str(builder, js_node.label.as_deref());
            encode_definition_data(url_ref, title_ref, id_ref, label_ref)
        }
        MdastNodeType::List => {
            let ordered = js_node.ordered.unwrap_or(false);
            let start = js_node.start.unwrap_or(1);
            let spread = js_node.spread.unwrap_or(false);
            encode_list_data(ordered, start, spread)
        }
        MdastNodeType::ListItem => {
            let checked = match js_node.checked {
                Some(true) => 1u8,
                Some(false) => 0u8,
                None => 2u8, // not a task item
            };
            let spread = js_node.spread.unwrap_or(false);
            encode_list_item_data(checked, spread)
        }
        MdastNodeType::LinkReference | MdastNodeType::FootnoteReference => {
            let id_ref = alloc_opt_str(builder, js_node.identifier.as_deref());
            let label_ref = alloc_opt_str(builder, js_node.label.as_deref());
            let kind = reference_kind(js_node.reference_type.as_deref());
            encode_reference_data(id_ref, label_ref, kind)
        }
        MdastNodeType::ImageReference => {
            let id_ref = alloc_opt_str(builder, js_node.identifier.as_deref());
            let label_ref = alloc_opt_str(builder, js_node.label.as_deref());
            let kind = reference_kind(js_node.reference_type.as_deref());
            let alt_ref = alloc_opt_str(builder, js_node.alt.as_deref());
            encode_image_reference_data(id_ref, label_ref, kind, alt_ref)
        }
        MdastNodeType::Table => encode_table_data(&column_aligns(js_node.align.as_deref())),
        MdastNodeType::FootnoteDefinition => {
            let id_ref = alloc_opt_str(builder, js_node.identifier.as_deref());
            let label_ref = alloc_opt_str(builder, js_node.label.as_deref());
            encode_footnote_definition_data(id_ref, label_ref)
        }
        #[cfg(feature = "mdx")]
        MdastNodeType::MdxJsxFlowElement | MdastNodeType::MdxJsxTextElement => {
            let name_ref = alloc_opt_str(builder, js_node.name.as_deref());
            let attr_tuples = encode_js_jsx_attrs(
                builder,
                js_node.attributes.as_ref().and_then(|a| a.as_jsx()),
            );
            let explicit = js_data_is_mdx_explicit(&js_node.data);
            encode_mdx_jsx_element_data(name_ref, &attr_tuples, explicit)
        }
        MdastNodeType::ContainerDirective
        | MdastNodeType::LeafDirective
        | MdastNodeType::TextDirective => {
            let name = js_node.name.as_deref().unwrap_or("");
            let name_ref = builder.alloc_string(name);
            let attr_pairs = encode_js_directive_attrs(builder, js_node.attributes.as_ref());
            encode_directive_data(name_ref, &attr_pairs)
        }
        #[cfg(feature = "mdx")]
        MdastNodeType::MdxFlowExpression
        | MdastNodeType::MdxTextExpression
        | MdastNodeType::MdxjsEsm => {
            let value_ref = alloc_opt_str(builder, js_node.value.as_deref());
            encode_expression_data(value_ref)
        }
        // Nodes with no type-specific data
        _ => Vec::new(),
    }
}

fn encode_js_directive_attrs(
    builder: &mut ArenaBuilder<Mdast>,
    attrs: Option<&satteri_ast::commands::JsNodeAttributes>,
) -> Vec<(StringRef, StringRef)> {
    let Some(map) = attrs.and_then(|a| a.as_directive()) else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(k, v)| {
            let val = v.as_str()?;
            Some((builder.alloc_string(k), builder.alloc_string(val)))
        })
        .collect()
}

/// Map an mdast `referenceType` to the arena reference-kind byte (shortcut = 0).
fn reference_kind(reference_type: Option<&str>) -> u8 {
    match reference_type {
        Some("collapsed") => 1,
        Some("full") => 2,
        _ => 0,
    }
}

/// Map an mdast table `align` array to arena [`ColumnAlign`] bytes.
fn column_aligns(align: Option<&[Option<String>]>) -> Vec<ColumnAlign> {
    align
        .unwrap_or(&[])
        .iter()
        .map(|a| match a.as_deref() {
            Some("left") => ColumnAlign::Left,
            Some("right") => ColumnAlign::Right,
            Some("center") => ColumnAlign::Center,
            _ => ColumnAlign::None,
        })
        .collect()
}

fn alloc_opt_str<K: ArenaKind>(builder: &mut ArenaBuilder<K>, s: Option<&str>) -> StringRef {
    match s {
        Some(v) if !v.is_empty() => builder.alloc_string(v),
        _ => StringRef::empty(),
    }
}

fn name_to_node_type(name: &str) -> Result<MdastNodeType, CommandError> {
    match name {
        "root" => Ok(MdastNodeType::Root),
        "paragraph" => Ok(MdastNodeType::Paragraph),
        "heading" => Ok(MdastNodeType::Heading),
        "thematicBreak" => Ok(MdastNodeType::ThematicBreak),
        "blockquote" => Ok(MdastNodeType::Blockquote),
        "list" => Ok(MdastNodeType::List),
        "listItem" => Ok(MdastNodeType::ListItem),
        "html" => Ok(MdastNodeType::Html),
        "code" => Ok(MdastNodeType::Code),
        "definition" => Ok(MdastNodeType::Definition),
        "text" => Ok(MdastNodeType::Text),
        "emphasis" => Ok(MdastNodeType::Emphasis),
        "strong" => Ok(MdastNodeType::Strong),
        "inlineCode" => Ok(MdastNodeType::InlineCode),
        "break" => Ok(MdastNodeType::Break),
        "link" => Ok(MdastNodeType::Link),
        "image" => Ok(MdastNodeType::Image),
        "linkReference" => Ok(MdastNodeType::LinkReference),
        "imageReference" => Ok(MdastNodeType::ImageReference),
        "footnoteDefinition" => Ok(MdastNodeType::FootnoteDefinition),
        "footnoteReference" => Ok(MdastNodeType::FootnoteReference),
        "table" => Ok(MdastNodeType::Table),
        "tableRow" => Ok(MdastNodeType::TableRow),
        "tableCell" => Ok(MdastNodeType::TableCell),
        "delete" => Ok(MdastNodeType::Delete),
        "yaml" => Ok(MdastNodeType::Yaml),
        "toml" => Ok(MdastNodeType::Toml),
        "math" => Ok(MdastNodeType::Math),
        "inlineMath" => Ok(MdastNodeType::InlineMath),
        "containerDirective" => Ok(MdastNodeType::ContainerDirective),
        "leafDirective" => Ok(MdastNodeType::LeafDirective),
        "textDirective" => Ok(MdastNodeType::TextDirective),
        #[cfg(feature = "mdx")]
        "mdxJsxFlowElement" => Ok(MdastNodeType::MdxJsxFlowElement),
        #[cfg(feature = "mdx")]
        "mdxJsxTextElement" => Ok(MdastNodeType::MdxJsxTextElement),
        #[cfg(feature = "mdx")]
        "mdxFlowExpression" => Ok(MdastNodeType::MdxFlowExpression),
        #[cfg(feature = "mdx")]
        "mdxTextExpression" => Ok(MdastNodeType::MdxTextExpression),
        #[cfg(feature = "mdx")]
        "mdxjsEsm" => Ok(MdastNodeType::MdxjsEsm),
        other => Err(CommandError::UnknownNodeType(other.to_string())),
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
    if prop_name == "data" {
        apply_data_property(arena, node_id, value_type, value_str);
        return Ok(());
    }

    let node_type = HastNodeType::from_u8(arena.get_node(node_id).node_type)
        .ok_or(CommandError::UnknownField(0))?;

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
                arena.type_data[data_offset..data_offset + 4]
                    .copy_from_slice(&sref.offset.to_le_bytes());
                arena.type_data[data_offset + 4..data_offset + 8]
                    .copy_from_slice(&sref.len.to_le_bytes());
                Ok(())
            } else {
                Err(CommandError::UnknownField(0))
            }
        }

        #[cfg(feature = "mdx")]
        HastNodeType::MdxJsxElement | HastNodeType::MdxJsxTextElement => {
            apply_hast_mdx_jsx_attribute(arena, node_id, prop_name, value_type, value_str)
        }

        _ => Err(CommandError::UnknownField(0)),
    }
}

/// Upsert a single attribute on an MDX JSX flow/text element, mirroring the
/// in-place attribute write that HAST elements get via
/// [`apply_hast_element_property`]. Avoids re-serializing the whole node (and
/// materializing its children) just to change one attribute.
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
        return Err(CommandError::UnexpectedEof);
    }
    let old_attr_count = u32::from_le_bytes(old_data[8..12].try_into().unwrap()) as usize;

    // Map the binary value-type to a JSX attribute (kind, value).
    let (kind, val_ref) = match value_type {
        PROP_BOOL_TRUE | PROP_NULL => (0u8, StringRef::empty()),
        PROP_BOOL_FALSE => (1u8, arena.alloc_string("false")),
        _ if value_str.is_empty() => (1u8, StringRef::empty()),
        _ => (1u8, arena.alloc_string(value_str)),
    };
    let name_ref = arena.alloc_string(attr_name);

    // Find an existing boolean/literal attribute with the same name. Expression
    // and spread attributes have no comparable name, so they're skipped.
    let mut found_index: Option<usize> = None;
    for i in 0..old_attr_count {
        let base = 16 + i * 20;
        let existing_kind = old_data[base];
        if existing_kind != 0 && existing_kind != 1 {
            continue;
        }
        let name_off = u32::from_le_bytes(old_data[base + 4..base + 8].try_into().unwrap());
        let name_len = u32::from_le_bytes(old_data[base + 8..base + 12].try_into().unwrap());
        if arena.get_str(StringRef::new(name_off, name_len)) == attr_name {
            found_index = Some(i);
            break;
        }
    }

    let write_attr = |buf: &mut [u8], base: usize| {
        buf[base] = kind;
        buf[base + 1..base + 4].copy_from_slice(&[0u8; 3]);
        buf[base + 4..base + 8].copy_from_slice(&name_ref.offset.to_le_bytes());
        buf[base + 8..base + 12].copy_from_slice(&name_ref.len.to_le_bytes());
        buf[base + 12..base + 16].copy_from_slice(&val_ref.offset.to_le_bytes());
        buf[base + 16..base + 20].copy_from_slice(&val_ref.len.to_le_bytes());
    };

    if let Some(idx) = found_index {
        let mut new_data = old_data;
        write_attr(&mut new_data, 16 + idx * 20);
        arena.set_type_data(node_id, &new_data);
    } else {
        let new_count = (old_attr_count + 1) as u32;
        let mut new_data = Vec::with_capacity(16 + new_count as usize * 20);
        new_data.extend_from_slice(&old_data[0..8]); // name StringRef
        new_data.extend_from_slice(&new_count.to_le_bytes());
        new_data.extend_from_slice(&0u32.to_le_bytes()); // pad
        if old_attr_count > 0 {
            new_data.extend_from_slice(&old_data[16..16 + old_attr_count * 20]);
        }
        new_data.resize(16 + new_count as usize * 20, 0);
        write_attr(&mut new_data, 16 + old_attr_count * 20);
        arena.set_type_data(node_id, &new_data);
    }

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
        return Err(CommandError::UnexpectedEof);
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

/// Emit a HAST JS node (from plugin JSON) into an ArenaBuilder.
fn emit_hast_js_node(
    js_node: &JsNode,
    builder: &mut ArenaBuilder<Hast>,
) -> Result<(), CommandError> {
    if let Some(ref_id) = js_node.ref_id {
        emit_ref_node(ref_id, builder);
        return Ok(());
    }

    let raw_type = name_to_hast_type(&js_node.node_type)
        .ok_or_else(|| CommandError::UnknownNodeType(js_node.node_type.clone()))?;
    builder.open_node_raw(raw_type as u8);

    let type_data = encode_hast_js_node_data(js_node, raw_type, builder);
    if !type_data.is_empty() {
        builder.set_data_current(&type_data);
    }

    write_js_node_data(js_node, builder)?;

    if let Some(children) = &js_node.children {
        for child in children {
            emit_hast_js_node(child, builder)?;
        }
    }

    builder.close_node();
    Ok(())
}

fn name_to_hast_type(name: &str) -> Option<HastNodeType> {
    match name {
        "root" => Some(HastNodeType::Root),
        "element" => Some(HastNodeType::Element),
        "text" => Some(HastNodeType::Text),
        "comment" => Some(HastNodeType::Comment),
        "doctype" => Some(HastNodeType::Doctype),
        "raw" => Some(HastNodeType::Raw),
        #[cfg(feature = "mdx")]
        "mdxJsxFlowElement" => Some(HastNodeType::MdxJsxElement),
        #[cfg(feature = "mdx")]
        "mdxJsxTextElement" => Some(HastNodeType::MdxJsxTextElement),
        #[cfg(feature = "mdx")]
        "mdxFlowExpression" => Some(HastNodeType::MdxFlowExpression),
        #[cfg(feature = "mdx")]
        "mdxTextExpression" => Some(HastNodeType::MdxTextExpression),
        #[cfg(feature = "mdx")]
        "mdxjsEsm" => Some(HastNodeType::MdxEsm),
        _ => None,
    }
}

fn encode_hast_js_node_data(
    js_node: &JsNode,
    node_type: HastNodeType,
    builder: &mut ArenaBuilder<Hast>,
) -> Vec<u8> {
    match node_type {
        HastNodeType::Element => {
            let tag = js_node.tag_name.as_deref().unwrap_or("div");
            let tag_ref = builder.alloc_string(tag);

            let mut props: Vec<(StringRef, u8, StringRef)> = Vec::new();
            if let Some(properties) = &js_node.properties {
                for (key, value) in properties {
                    let name_ref = builder.alloc_string(key);
                    match value {
                        serde_json::Value::Bool(true) => {
                            props.push((name_ref, PROP_BOOL_TRUE, StringRef::empty()));
                        }
                        serde_json::Value::Bool(false) => {
                            props.push((name_ref, PROP_BOOL_FALSE, StringRef::empty()));
                        }
                        serde_json::Value::String(s) => {
                            let val_ref = builder.alloc_string(s);
                            props.push((name_ref, PROP_STRING, val_ref));
                        }
                        serde_json::Value::Number(n) => {
                            let val_ref = builder.alloc_string(&n.to_string());
                            props.push((name_ref, PROP_INT, val_ref));
                        }
                        serde_json::Value::Array(arr) => {
                            let joined: String = arr
                                .iter()
                                .filter_map(|v| v.as_str())
                                .collect::<Vec<_>>()
                                .join(" ");
                            let val_ref = builder.alloc_string(&joined);
                            props.push((name_ref, PROP_SPACE_SEP, val_ref));
                        }
                        _ => {}
                    }
                }
            }

            let mut out = Vec::with_capacity(16 + props.len() * 20);
            out.extend_from_slice(&tag_ref.offset.to_le_bytes());
            out.extend_from_slice(&tag_ref.len.to_le_bytes());
            out.extend_from_slice(&(props.len() as u32).to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            for (name_ref, kind, val_ref) in &props {
                out.extend_from_slice(&name_ref.offset.to_le_bytes());
                out.extend_from_slice(&name_ref.len.to_le_bytes());
                out.push(*kind);
                out.extend_from_slice(&[0u8; 3]);
                out.extend_from_slice(&val_ref.offset.to_le_bytes());
                out.extend_from_slice(&val_ref.len.to_le_bytes());
            }
            out
        }

        HastNodeType::Text | HastNodeType::Comment | HastNodeType::Raw => {
            let value = js_node.value.as_deref().unwrap_or("");
            let sref = builder.alloc_string(value);
            let mut out = [0u8; 8];
            out[0..4].copy_from_slice(&sref.offset.to_le_bytes());
            out[4..8].copy_from_slice(&sref.len.to_le_bytes());
            out.to_vec()
        }

        #[cfg(feature = "mdx")]
        HastNodeType::MdxJsxElement | HastNodeType::MdxJsxTextElement => {
            let name = js_node
                .name
                .as_deref()
                .or(js_node.tag_name.as_deref())
                .unwrap_or("");
            let name_ref = builder.alloc_string(name);
            let attr_tuples = encode_js_jsx_attrs(
                builder,
                js_node.attributes.as_ref().and_then(|a| a.as_jsx()),
            );
            let explicit = js_data_is_mdx_explicit(&js_node.data);
            encode_mdx_jsx_element_data(name_ref, &attr_tuples, explicit)
        }

        #[cfg(feature = "mdx")]
        HastNodeType::MdxFlowExpression
        | HastNodeType::MdxTextExpression
        | HastNodeType::MdxEsm => {
            let value = js_node.value.as_deref().unwrap_or("");
            let sref = builder.alloc_string(value);
            let mut out = [0u8; 8];
            out[0..4].copy_from_slice(&sref.offset.to_le_bytes());
            out[4..8].copy_from_slice(&sref.len.to_le_bytes());
            out.to_vec()
        }

        _ => Vec::new(),
    }
}

// ── Imperative-builder op-stream replay ──────────────────────────────────────
//
// The JS `m` builder emits an op-stream (OPEN/CLOSE/field sets/REF/KEEP_CHILDREN)
// that we replay directly into an ArenaBuilder — no `JsNode` tree, no heap
// allocation per node beyond the arena itself. A node's fields are collected
// after its OPEN and flushed into its type_data the moment the next op needs the
// node finalized (a child OPEN, a CLOSE, or a spliced REF/KEEP_CHILDREN).

/// Accumulates one node's fields between its OPEN and finalization. Strings
/// borrow the op-stream buffer; they're interned into the arena at finalize.
#[derive(Default)]
struct FieldCollector<'a> {
    node_type: u8,
    finalized: bool,
    strs: [Option<&'a str>; 10],
    depth: Option<u8>,
    checked: Option<u8>,
    start: Option<u32>,
    ordered: Option<bool>,
    spread: Option<bool>,
    /// Directive / MDX JSX attributes (`OP_PROP`): (name, kind, value).
    props: Vec<(&'a str, u8, &'a str)>,
    /// Table column-alignment bytes (`OP_ALIGN`).
    align: Option<&'a [u8]>,
    /// MDX JSX `_mdxExplicitJsx` flag (`OP_BOOL` on `OF_EXPLICIT`).
    explicit: Option<bool>,
    data: Option<&'a [u8]>,
}

/// Encode a collector's fields into the current node's type_data (and node_data).
fn finalize_collector(c: &mut FieldCollector<'_>, builder: &mut ArenaBuilder<Mdast>) {
    if c.finalized {
        return;
    }
    c.finalized = true;
    let s = |i: u8| c.strs[i as usize];
    let type_data: Vec<u8> = match c.node_type {
        // Heading
        2 => encode_heading_data(c.depth.unwrap_or(1)),
        // Text, Html, InlineCode, Yaml, Toml, InlineMath: single value StringRef
        10 | 7 | 13 | 25 | 26 | 28 => {
            let sref = builder.alloc_string(s(OF_VALUE).unwrap_or(""));
            encode_string_ref_data(sref)
        }
        // Code: lang + meta + value
        8 => {
            let lang = alloc_opt_str(builder, s(OF_LANG));
            let meta = alloc_opt_str(builder, s(OF_META));
            let value = alloc_opt_str(builder, s(OF_VALUE));
            encode_code_data(lang, meta, value, b'`')
        }
        // Math: meta + value
        27 => {
            let meta = alloc_opt_str(builder, s(OF_META));
            let value = alloc_opt_str(builder, s(OF_VALUE));
            encode_math_data(meta, value)
        }
        // Link: url + title
        15 => {
            let url = alloc_opt_str(builder, s(OF_URL));
            let title = alloc_opt_str(builder, s(OF_TITLE));
            encode_link_data(url, title)
        }
        // Image: url + alt + title
        16 => {
            let url = alloc_opt_str(builder, s(OF_URL));
            let alt = alloc_opt_str(builder, s(OF_ALT));
            let title = alloc_opt_str(builder, s(OF_TITLE));
            encode_image_data(url, alt, title)
        }
        // Definition: url + title + identifier + label
        9 => {
            let url = alloc_opt_str(builder, s(OF_URL));
            let title = alloc_opt_str(builder, s(OF_TITLE));
            let id = alloc_opt_str(builder, s(OF_IDENTIFIER));
            let label = alloc_opt_str(builder, s(OF_LABEL));
            encode_definition_data(url, title, id, label)
        }
        // List: ordered + start + spread
        5 => encode_list_data(
            c.ordered.unwrap_or(false),
            c.start.unwrap_or(1),
            c.spread.unwrap_or(false),
        ),
        // ListItem: checked (0/1/2=none) + spread
        6 => encode_list_item_data(c.checked.unwrap_or(2), c.spread.unwrap_or(false)),
        // LinkReference / FootnoteReference: identifier + label + kind
        17 | 20 => {
            let id = alloc_opt_str(builder, s(OF_IDENTIFIER));
            let label = alloc_opt_str(builder, s(OF_LABEL));
            encode_reference_data(id, label, reference_kind(s(OF_REFERENCE_TYPE)))
        }
        // ImageReference: identifier + label + kind + alt
        18 => {
            let id = alloc_opt_str(builder, s(OF_IDENTIFIER));
            let label = alloc_opt_str(builder, s(OF_LABEL));
            let alt = alloc_opt_str(builder, s(OF_ALT));
            encode_image_reference_data(id, label, reference_kind(s(OF_REFERENCE_TYPE)), alt)
        }
        // FootnoteDefinition: identifier + label
        19 => {
            let id = alloc_opt_str(builder, s(OF_IDENTIFIER));
            let label = alloc_opt_str(builder, s(OF_LABEL));
            encode_footnote_definition_data(id, label)
        }
        // Table: column alignment bytes
        21 => {
            let aligns: Vec<ColumnAlign> = c
                .align
                .unwrap_or(&[])
                .iter()
                .map(|&b| ColumnAlign::from_u8(b).unwrap_or(ColumnAlign::None))
                .collect();
            encode_table_data(&aligns)
        }
        // MdxJsxFlowElement / MdxJsxTextElement: name + typed attributes + explicit
        #[cfg(feature = "mdx")]
        100 | 101 => {
            let name_ref = builder.alloc_string(s(OF_NAME).unwrap_or(""));
            let mut attrs: Vec<(u8, StringRef, StringRef)> = Vec::with_capacity(c.props.len());
            for &(name, kind, value) in &c.props {
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
            encode_mdx_jsx_element_data(name_ref, &attrs, c.explicit.unwrap_or(false))
        }
        // Container/Leaf/TextDirective: name + string attributes
        30..=32 => {
            let name_ref = builder.alloc_string(s(OF_NAME).unwrap_or(""));
            let mut attrs: Vec<(StringRef, StringRef)> = Vec::with_capacity(c.props.len());
            for &(key, _kind, value) in &c.props {
                let kr = builder.alloc_string(key);
                let vr = builder.alloc_string(value);
                attrs.push((kr, vr));
            }
            encode_directive_data(name_ref, &attrs)
        }
        // MdxFlowExpression, MdxTextExpression, MdxjsEsm: single value StringRef
        #[cfg(feature = "mdx")]
        102..=104 => encode_expression_data(alloc_opt_str(builder, s(OF_VALUE))),
        // Root, Paragraph, ThematicBreak, Blockquote, Emphasis, Strong, Break,
        // TableRow, TableCell, Delete: no type_data.
        _ => Vec::new(),
    };
    if !type_data.is_empty() {
        builder.set_data_current(&type_data);
    }
    if let Some(data) = c.data {
        let id = builder.current_node_id();
        builder.arena_mut().set_node_data(id, data.to_vec());
    }
}

/// Replay an op-stream into a fresh sub-arena. `orig`/`anchor` resolve
/// `KEEP_CHILDREN` (splice the replaced node's original children, as refs).
fn replay_mdast_opstream(
    ops: &[u8],
    orig: &Arena<Mdast>,
    anchor: u32,
) -> Result<Arena<Mdast>, CommandError> {
    let mut builder = ArenaBuilder::<Mdast>::new(String::new());
    let mut reader = BufReader::new(ops);
    let mut stack: Vec<FieldCollector<'_>> = Vec::new();

    while reader.remaining() > 0 {
        match reader.read_u8()? {
            OP_OPEN => {
                if let Some(c) = stack.last_mut() {
                    finalize_collector(c, &mut builder);
                }
                let node_type = reader.read_u8()?;
                builder.open_node(node_type);
                stack.push(FieldCollector {
                    node_type,
                    ..Default::default()
                });
            }
            OP_CLOSE => {
                if let Some(mut c) = stack.pop() {
                    finalize_collector(&mut c, &mut builder);
                }
                builder.close_node();
            }
            OP_REF => {
                if let Some(c) = stack.last_mut() {
                    finalize_collector(c, &mut builder);
                }
                let id = reader.read_u32()?;
                emit_ref_node(id, &mut builder);
            }
            OP_KEEP_CHILDREN => {
                if let Some(c) = stack.last_mut() {
                    finalize_collector(c, &mut builder);
                }
                for &child in orig.get_children(anchor) {
                    emit_ref_node(child, &mut builder);
                }
            }
            OP_STR => {
                let field = reader.read_u8()? as usize;
                let len = reader.read_u32()? as usize;
                let value = reader.read_str(len)?;
                if let Some(c) = stack.last_mut() {
                    if field < c.strs.len() {
                        c.strs[field] = Some(value);
                    }
                }
            }
            OP_U8 => {
                let field = reader.read_u8()?;
                let value = reader.read_u8()?;
                if let Some(c) = stack.last_mut() {
                    match field {
                        OF_DEPTH => c.depth = Some(value),
                        OF_CHECKED => c.checked = Some(value),
                        _ => {}
                    }
                }
            }
            OP_U32 => {
                let field = reader.read_u8()?;
                let value = reader.read_u32()?;
                if let (Some(c), OF_START) = (stack.last_mut(), field) {
                    c.start = Some(value);
                }
            }
            OP_BOOL => {
                let field = reader.read_u8()?;
                let value = reader.read_u8()? != 0;
                if let Some(c) = stack.last_mut() {
                    match field {
                        OF_ORDERED => c.ordered = Some(value),
                        OF_SPREAD => c.spread = Some(value),
                        OF_EXPLICIT => c.explicit = Some(value),
                        _ => {}
                    }
                }
            }
            OP_PROP => {
                let name_len = reader.read_u32()? as usize;
                let name = reader.read_str(name_len)?;
                let kind = reader.read_u8()?;
                let val_len = reader.read_u32()? as usize;
                let value = reader.read_str(val_len)?;
                if let Some(c) = stack.last_mut() {
                    c.props.push((name, kind, value));
                }
            }
            OP_ALIGN => {
                let len = reader.read_u32()? as usize;
                let bytes = reader.read_bytes(len)?;
                if let Some(c) = stack.last_mut() {
                    c.align = Some(bytes);
                }
            }
            OP_DATA => {
                let len = reader.read_u32()? as usize;
                let bytes = reader.read_bytes(len)?;
                if let Some(c) = stack.last_mut() {
                    c.data = Some(bytes);
                }
            }
            other => return Err(CommandError::UnknownCommand(other)),
        }
    }

    Ok(builder.finish())
}

/// Returns (arena, keep_children) for an MDAST sub-tree payload. `orig`/`anchor`
/// are the arena and the command's target node, used by an op-stream's
/// `KEEP_CHILDREN`.
fn read_mdast_payload(
    reader: &mut BufReader<'_>,
    parse_markdown: &dyn Fn(&str) -> Arena<Mdast>,
    orig: &Arena<Mdast>,
    anchor: u32,
) -> Result<(Arena<Mdast>, bool), CommandError> {
    let payload_type = reader.read_u8()?;
    let len = reader.read_u32()? as usize;

    match payload_type {
        PAYLOAD_RAW_MARKDOWN => {
            let md = reader.read_str(len)?;
            Ok((parse_raw_markdown(md, parse_markdown), false))
        }
        PAYLOAD_RAW_HTML => {
            let html = reader.read_str(len)?;
            let escaped = escape_braces_in_html_text(html);
            Ok((parse_raw_markdown(&escaped, parse_markdown), false))
        }
        PAYLOAD_SERDE_JSON => {
            let json_str = reader.read_str(len)?;
            let js_node: JsNode = serde_json::from_str(json_str)
                .map_err(|e| CommandError::InvalidJson(e.to_string()))?;
            js_node_to_mdast_arena(&js_node)
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
struct HastFieldCollector<'a> {
    node_type: u8,
    finalized: bool,
    /// HAST element `tagName` or MDX JSX element `name`.
    tag: Option<&'a str>,
    value: Option<&'a str>,
    props: Vec<(&'a str, u8, &'a str)>,
    /// MDX JSX `_mdxExplicitJsx` flag (`OP_BOOL` on `OF_EXPLICIT`).
    explicit: Option<bool>,
    data: Option<&'a [u8]>,
}

fn finalize_hast_collector(c: &mut HastFieldCollector<'_>, builder: &mut ArenaBuilder<Hast>) {
    if c.finalized {
        return;
    }
    c.finalized = true;
    let type_data: Vec<u8> = match c.node_type {
        // Element: [tag StringRef][prop_count u32][pad u32] then prop_count × 20B.
        1 => {
            let tag_ref = builder.alloc_string(c.tag.unwrap_or("div"));
            let mut prop_refs: Vec<(StringRef, u8, StringRef)> = Vec::with_capacity(c.props.len());
            for &(name, kind, value) in &c.props {
                let n = builder.alloc_string(name);
                let v = if kind == PROP_BOOL_TRUE || kind == PROP_BOOL_FALSE {
                    StringRef::empty()
                } else {
                    builder.alloc_string(value)
                };
                prop_refs.push((n, kind, v));
            }
            let mut td = Vec::with_capacity(16 + prop_refs.len() * 20);
            td.extend_from_slice(&tag_ref.offset.to_le_bytes());
            td.extend_from_slice(&tag_ref.len.to_le_bytes());
            td.extend_from_slice(&(prop_refs.len() as u32).to_le_bytes());
            td.extend_from_slice(&0u32.to_le_bytes());
            for (n, kind, v) in &prop_refs {
                td.extend_from_slice(&n.offset.to_le_bytes());
                td.extend_from_slice(&n.len.to_le_bytes());
                td.push(*kind);
                td.extend_from_slice(&[0u8; 3]);
                td.extend_from_slice(&v.offset.to_le_bytes());
                td.extend_from_slice(&v.len.to_le_bytes());
            }
            td
        }
        // Text, Comment, Raw, and the MDX expression/ESM nodes (which store the
        // same single value StringRef).
        2 | 3 | 5 | 12 | 13 | 14 => {
            let sref = builder.alloc_string(c.value.unwrap_or(""));
            encode_string_ref_data(sref)
        }
        // MdxJsxElement / MdxJsxTextElement: name + typed attributes + explicit
        #[cfg(feature = "mdx")]
        10 | 11 => {
            let name_ref = builder.alloc_string(c.tag.unwrap_or(""));
            let mut attrs: Vec<(u8, StringRef, StringRef)> = Vec::with_capacity(c.props.len());
            for &(name, kind, value) in &c.props {
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
            encode_mdx_jsx_element_data(name_ref, &attrs, c.explicit.unwrap_or(false))
        }
        // Root, Doctype: no type_data
        _ => Vec::new(),
    };
    if !type_data.is_empty() {
        builder.set_data_current(&type_data);
    }
    if let Some(data) = c.data {
        let id = builder.current_node_id();
        builder.arena_mut().set_node_data(id, data.to_vec());
    }
}

fn replay_hast_opstream(
    ops: &[u8],
    orig: &Arena<Hast>,
    anchor: u32,
) -> Result<Arena<Hast>, CommandError> {
    let mut builder = ArenaBuilder::<Hast>::new(String::new());
    let mut reader = BufReader::new(ops);
    let mut stack: Vec<HastFieldCollector<'_>> = Vec::new();

    while reader.remaining() > 0 {
        match reader.read_u8()? {
            OP_OPEN => {
                if let Some(c) = stack.last_mut() {
                    finalize_hast_collector(c, &mut builder);
                }
                let node_type = reader.read_u8()?;
                builder.open_node_raw(node_type);
                stack.push(HastFieldCollector {
                    node_type,
                    ..Default::default()
                });
            }
            OP_CLOSE => {
                if let Some(mut c) = stack.pop() {
                    finalize_hast_collector(&mut c, &mut builder);
                }
                builder.close_node();
            }
            OP_REF => {
                if let Some(c) = stack.last_mut() {
                    finalize_hast_collector(c, &mut builder);
                }
                let id = reader.read_u32()?;
                emit_ref_node(id, &mut builder);
            }
            OP_KEEP_CHILDREN => {
                if let Some(c) = stack.last_mut() {
                    finalize_hast_collector(c, &mut builder);
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
                    match field {
                        OF_TAGNAME | OF_NAME => c.tag = Some(value),
                        OF_VALUE => c.value = Some(value),
                        _ => {}
                    }
                }
            }
            OP_BOOL => {
                let field = reader.read_u8()?;
                let value = reader.read_u8()? != 0;
                if let Some(c) = stack.last_mut() {
                    if field == OF_EXPLICIT {
                        c.explicit = Some(value);
                    }
                }
            }
            OP_PROP => {
                let name_len = reader.read_u32()? as usize;
                let name = reader.read_str(name_len)?;
                let kind = reader.read_u8()?;
                let val_len = reader.read_u32()? as usize;
                let value = reader.read_str(val_len)?;
                if let Some(c) = stack.last_mut() {
                    c.props.push((name, kind, value));
                }
            }
            OP_DATA => {
                let len = reader.read_u32()? as usize;
                let bytes = reader.read_bytes(len)?;
                if let Some(c) = stack.last_mut() {
                    c.data = Some(bytes);
                }
            }
            other => return Err(CommandError::UnknownCommand(other)),
        }
    }

    Ok(builder.finish())
}

/// Returns (arena, keep_children) for a HAST sub-tree payload. `PAYLOAD_OPSTREAM`
/// (declarative-compiled) and `PAYLOAD_SERDE_JSON` (declarative JSON fallback) are
/// accepted — HAST has no source grammar, so raw markdown / HTML are not.
fn read_hast_payload(
    reader: &mut BufReader<'_>,
    orig: &Arena<Hast>,
    anchor: u32,
) -> Result<(Arena<Hast>, bool), CommandError> {
    let payload_type = reader.read_u8()?;
    let len = reader.read_u32()? as usize;

    match payload_type {
        PAYLOAD_SERDE_JSON => {
            let json_str = reader.read_str(len)?;
            let js_node: JsNode = serde_json::from_str(json_str)
                .map_err(|e| CommandError::InvalidJson(e.to_string()))?;
            js_node_to_hast_arena(&js_node)
        }
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
    let (arena, dropped) = apply_mdast_commands_lenient(arena, command_buf, parse_markdown)?;
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
    mut arena: Arena<Mdast>,
    command_buf: &[u8],
    parse_markdown: &dyn Fn(&str) -> Arena<Mdast>,
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
                    read_mdast_payload(&mut reader, parse_markdown, &arena, node_id)?;
                patches.push(Patch::InsertBefore { node_id, new_tree });
            }

            CMD_INSERT_AFTER => {
                let node_id = reader.read_u32()?;
                let (new_tree, _) =
                    read_mdast_payload(&mut reader, parse_markdown, &arena, node_id)?;
                patches.push(Patch::InsertAfter { node_id, new_tree });
            }

            CMD_PREPEND_CHILD => {
                let node_id = reader.read_u32()?;
                let (child_tree, _) =
                    read_mdast_payload(&mut reader, parse_markdown, &arena, node_id)?;
                patches.push(Patch::PrependChild {
                    node_id,
                    child_tree,
                });
            }

            CMD_APPEND_CHILD => {
                let node_id = reader.read_u32()?;
                let (child_tree, _) =
                    read_mdast_payload(&mut reader, parse_markdown, &arena, node_id)?;
                patches.push(Patch::AppendChild {
                    node_id,
                    child_tree,
                });
            }

            CMD_WRAP => {
                let node_id = reader.read_u32()?;
                let (parent_tree, _) =
                    read_mdast_payload(&mut reader, parse_markdown, &arena, node_id)?;
                patches.push(Patch::Wrap {
                    node_id,
                    parent_tree,
                });
            }

            CMD_REPLACE => {
                let node_id = reader.read_u32()?;
                let (new_tree, keep_children) =
                    read_mdast_payload(&mut reader, parse_markdown, &arena, node_id)?;
                patches.push(Patch::Replace {
                    node_id,
                    new_tree,
                    keep_children,
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
/// HAST plugins inject sub-trees via `PAYLOAD_SERDE_JSON` only — there is
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

    // Op-stream builder helpers for tests.
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
    fn replace_with_serde_json() {
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];

        let json =
            r#"{"type":"heading","depth":2,"children":[{"type":"text","value":"Replaced"}]}"#;
        let mut buf = Vec::new();
        buf.push(CMD_REPLACE);
        push_u32(&mut buf, heading_id);
        buf.push(PAYLOAD_SERDE_JSON);
        push_u32(&mut buf, json.len() as u32);
        buf.extend_from_slice(json.as_bytes());

        let result = apply_mdast_commands(arena.clone(), &buf, &test_parse_markdown).unwrap();
        let root_children = result.get_children(0);
        assert_eq!(root_children.len(), 2);
        let new_heading = root_children[0];
        assert_eq!(
            result.get_node(new_heading).node_type,
            MdastNodeType::Heading as u8
        );
        let heading_data = result.get_type_data(new_heading);
        assert_eq!(decode_heading_data(heading_data).depth, 2);
    }

    #[test]
    fn replace_with_directive_child() {
        // Directives serialize `attributes` as a map (`{}`), not the array form
        // used by MDX JSX. The deserializer must accept both shapes; without
        // that, any plugin returning a tree containing a directive child fails
        // with "invalid type: map, expected a sequence".
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];

        let json = r#"{"type":"paragraph","children":[{"type":"text","value":"hi "},{"type":"textDirective","name":"inline","attributes":{},"children":[]}]}"#;
        let mut buf = Vec::new();
        buf.push(CMD_REPLACE);
        push_u32(&mut buf, heading_id);
        buf.push(PAYLOAD_SERDE_JSON);
        push_u32(&mut buf, json.len() as u32);
        buf.extend_from_slice(json.as_bytes());

        let result = apply_mdast_commands(arena.clone(), &buf, &test_parse_markdown).unwrap();
        let root_children = result.get_children(0);
        let new_para = root_children[0];
        assert_eq!(
            result.get_node(new_para).node_type,
            MdastNodeType::Paragraph as u8
        );
        let para_children = result.get_children(new_para);
        assert_eq!(para_children.len(), 2);
        let directive = para_children[1];
        assert_eq!(
            result.get_node(directive).node_type,
            MdastNodeType::TextDirective as u8
        );
        let dir_data = result.get_type_data(directive);
        assert_eq!(decode_directive_attr_count(dir_data), 0);
    }

    #[test]
    fn replace_with_directive_attrs() {
        // Same as above but with non-empty directive attrs to confirm the map
        // shape round-trips into the arena's directive type_data.
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];

        let json = r#"{"type":"containerDirective","name":"tip","attributes":{"id":"foo","class":"bar"},"children":[]}"#;
        let mut buf = Vec::new();
        buf.push(CMD_REPLACE);
        push_u32(&mut buf, heading_id);
        buf.push(PAYLOAD_SERDE_JSON);
        push_u32(&mut buf, json.len() as u32);
        buf.extend_from_slice(json.as_bytes());

        let result = apply_mdast_commands(arena.clone(), &buf, &test_parse_markdown).unwrap();
        let directive = result.get_children(0)[0];
        assert_eq!(
            result.get_node(directive).node_type,
            MdastNodeType::ContainerDirective as u8
        );
        let dir_data = result.get_type_data(directive);
        assert_eq!(decode_directive_attr_count(dir_data), 2);
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
    fn js_node_to_arena_basic() {
        let js = JsNode {
            node_type: "heading".to_string(),
            children: Some(vec![JsNode {
                node_type: "text".to_string(),
                children: None,
                value: Some("Hello".to_string()),
                depth: None,
                url: None,
                title: None,
                alt: None,
                lang: None,
                meta: None,
                ordered: None,
                start: None,
                spread: None,
                checked: None,
                identifier: None,
                label: None,
                reference_type: None,
                name: None,
                align: None,
                attributes: None,
                tag_name: None,
                properties: None,
                is_hast: false,
                keep_children: false,
                ref_id: None,
                data: None,
            }]),
            depth: Some(2),
            value: None,
            url: None,
            title: None,
            alt: None,
            lang: None,
            meta: None,
            ordered: None,
            start: None,
            spread: None,
            checked: None,
            identifier: None,
            label: None,
            reference_type: None,
            name: None,
            align: None,
            attributes: None,
            tag_name: None,
            properties: None,
            is_hast: false,
            keep_children: false,
            ref_id: None,
            data: None,
        };

        let (arena, _keep) = js_node_to_mdast_arena(&js).unwrap();
        assert_eq!(arena.len(), 2);
        assert_eq!(arena.get_node(0).node_type, MdastNodeType::Heading as u8);
        assert_eq!(arena.get_children(0).len(), 1);
        let text_id = arena.get_children(0)[0];
        assert_eq!(arena.get_node(text_id).node_type, MdastNodeType::Text as u8);
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
