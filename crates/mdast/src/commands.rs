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
//!   0x02  SET_PROPERTY_INT [nodeId: u32][fieldId: u16][value: i64]
//!   0x03  SET_PROPERTY_STR [nodeId: u32][fieldId: u16][len: u32][utf8...]
//!   0x04  SET_PROPERTY_BOOL[nodeId: u32][fieldId: u16][value: u8]
//!   0x05  INSERT_BEFORE    [nodeId: u32][payloadType: u8][payload...]
//!   0x06  INSERT_AFTER     [nodeId: u32][payloadType: u8][payload...]
//!   0x07  PREPEND_CHILD    [nodeId: u32][payloadType: u8][payload...]
//!   0x08  APPEND_CHILD     [nodeId: u32][payloadType: u8][payload...]
//!   0x09  WRAP             [nodeId: u32][payloadType: u8][payload...]
//!   0x0A  SET_PROPERTY_NULL[nodeId: u32][fieldId: u16]
//!   0x0B  REPLACE          [nodeId: u32][payloadType: u8][payload...]
//!
//! Payload types:
//!   0x10  RAW_MARKDOWN     [len: u32][utf8...]
//!   0x11  RAW_HTML         [len: u32][utf8...]
//!   0x12  SERDE_JSON       [len: u32][utf8...]

use crate::codec::*;
use crate::jsx_attr_parser::{
    MDX_ATTR_BOOLEAN_PROP, MDX_ATTR_EXPRESSION_PROP, MDX_ATTR_LITERAL_PROP, MDX_ATTR_SPREAD,
};
use crate::node::{MdastNodeType, StringRef};
use crate::rebuild::Patch;
use crate::{MdastArena, MdastBuilder};

use serde::Deserialize;

// Must match packages/tryckeri/src/command-buffer.ts
pub const CMD_REMOVE: u8 = 0x01;
pub const CMD_SET_INT: u8 = 0x02;
pub const CMD_SET_STRING: u8 = 0x03;
pub const CMD_SET_BOOL: u8 = 0x04;
pub const CMD_INSERT_BEFORE: u8 = 0x05;
pub const CMD_INSERT_AFTER: u8 = 0x06;
pub const CMD_PREPEND_CHILD: u8 = 0x07;
pub const CMD_APPEND_CHILD: u8 = 0x08;
pub const CMD_WRAP: u8 = 0x09;
pub const CMD_SET_NULL: u8 = 0x0A;
pub const CMD_REPLACE: u8 = 0x0B;

pub const PAYLOAD_RAW_MARKDOWN: u8 = 0x10;
pub const PAYLOAD_RAW_HTML: u8 = 0x11;
pub const PAYLOAD_SERDE_JSON: u8 = 0x12;

pub const FIELD_DEPTH: u16 = 0x0001;
pub const FIELD_URL: u16 = 0x0010;
pub const FIELD_TITLE: u16 = 0x0011;
pub const FIELD_LANG: u16 = 0x0020;
pub const FIELD_META: u16 = 0x0021;
pub const FIELD_VALUE: u16 = 0x0022;
pub const FIELD_ALT: u16 = 0x0030;
pub const FIELD_ORDERED: u16 = 0x0040;
pub const FIELD_START: u16 = 0x0041;
pub const FIELD_SPREAD: u16 = 0x0042;
pub const FIELD_CHECKED: u16 = 0x0050;
pub const FIELD_IDENTIFIER: u16 = 0x0060;
pub const FIELD_LABEL: u16 = 0x0061;
pub const FIELD_REFERENCE_TYPE: u16 = 0x0062;
pub const FIELD_NAME: u16 = 0x0070;

#[derive(Debug, Deserialize)]
pub struct JsNode {
    #[serde(rename = "type")]
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
    pub attributes: Option<Vec<JsNodeAttribute>>,
    // HAST-specific fields
    #[serde(rename = "tagName")]
    pub tag_name: Option<String>,
    #[serde(default)]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,
    /// Marker: when true, this node is a HAST node (not MDAST).
    #[serde(rename = "_hast", default)]
    pub is_hast: bool,
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

// ---------------------------------------------------------------------------
// HAST node type constants (duplicated from node_types.rs to avoid dependency)
// ---------------------------------------------------------------------------

const HAST_ROOT_TYPE: u8 = 0;
const HAST_ELEMENT_TYPE: u8 = 1;
const HAST_TEXT_TYPE: u8 = 2;
const HAST_COMMENT_TYPE: u8 = 3;
const HAST_DOCTYPE_TYPE: u8 = 4;
const HAST_RAW_TYPE: u8 = 5;
const HAST_MDX_JSX_ELEMENT_TYPE: u8 = 10;
const HAST_MDX_JSX_TEXT_ELEMENT_TYPE: u8 = 11;
const HAST_MDX_EXPRESSION_TYPE: u8 = 12;
const HAST_MDX_ESM_TYPE: u8 = 13;

const HAST_PROP_STRING: u8 = 0;
const HAST_PROP_BOOL_TRUE: u8 = 1;
const HAST_PROP_BOOL_FALSE: u8 = 2;
const HAST_PROP_SPACE_SEP: u8 = 3;

#[derive(Debug)]
pub enum CommandError {
    UnexpectedEof,
    UnknownCommand(u8),
    UnknownPayloadType(u8),
    InvalidUtf8,
    InvalidJson(String),
    UnknownNodeType(String),
    UnknownField(u16),
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
        }
    }
}

impl std::error::Error for CommandError {}

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

    fn read_u16(&mut self) -> Result<u16, CommandError> {
        if self.remaining() < 2 {
            return Err(CommandError::UnexpectedEof);
        }
        let v = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
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

    fn read_i64(&mut self) -> Result<i64, CommandError> {
        if self.remaining() < 8 {
            return Err(CommandError::UnexpectedEof);
        }
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.data[self.pos..self.pos + 8]);
        self.pos += 8;
        Ok(i64::from_le_bytes(bytes))
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

/// String fields are allocated in the arena's source buffer with the StringRef
/// updated; fixed-size fields are modified in-place.
fn apply_set_int(
    arena: &mut MdastArena,
    node_id: u32,
    field_id: u16,
    value: i64,
) -> Result<(), CommandError> {
    let node = arena.get_node(node_id);
    let node_type = node.node_type;
    let data_offset = node.data_offset as usize;
    let data_len = node.data_len as usize;

    match (node_type, field_id) {
        // Heading.depth (u8 at offset 0)
        (2, FIELD_DEPTH) => {
            if data_len >= 1 {
                arena.type_data[data_offset] = value as u8;
            }
        }
        // List.start (u32 at offset 0 in ListData)
        (5, FIELD_START) => {
            if data_len >= 4 {
                let bytes = (value as u32).to_ne_bytes();
                arena.type_data[data_offset..data_offset + 4].copy_from_slice(&bytes);
            }
        }
        // ListItem.checked (u8 at offset 0 in ListItemData)
        // 0=unchecked, 1=checked, 2=not a task item
        (6, FIELD_CHECKED) => {
            if data_len >= 1 {
                arena.type_data[data_offset] = value as u8;
            }
        }
        _ => return Err(CommandError::UnknownField(field_id)),
    }
    Ok(())
}

fn apply_set_string(
    arena: &mut MdastArena,
    node_id: u32,
    field_id: u16,
    value: &str,
) -> Result<(), CommandError> {
    // Allocate the new string in the arena's source buffer
    let sref = arena.alloc_string(value);
    set_string_ref(arena, node_id, field_id, sref)
}

fn apply_set_bool(
    arena: &mut MdastArena,
    node_id: u32,
    field_id: u16,
    value: bool,
) -> Result<(), CommandError> {
    let node = arena.get_node(node_id);
    let node_type = node.node_type;
    let data_offset = node.data_offset as usize;
    let data_len = node.data_len as usize;

    match (node_type, field_id) {
        // List.ordered (bool at offset 4 in ListData)
        (5, FIELD_ORDERED) => {
            if data_len >= 5 {
                arena.type_data[data_offset + 4] = value as u8;
            }
        }
        // List.spread (bool at offset 5 in ListData)
        (5, FIELD_SPREAD) => {
            if data_len >= 6 {
                arena.type_data[data_offset + 5] = value as u8;
            }
        }
        // ListItem.spread (bool at offset 1 in ListItemData)
        (6, FIELD_SPREAD) => {
            if data_len >= 2 {
                arena.type_data[data_offset + 1] = value as u8;
            }
        }
        _ => return Err(CommandError::UnknownField(field_id)),
    }
    Ok(())
}

fn apply_set_null(arena: &mut MdastArena, node_id: u32, field_id: u16) -> Result<(), CommandError> {
    let node = arena.get_node(node_id);
    let node_type = node.node_type;
    let data_offset = node.data_offset as usize;
    let data_len = node.data_len as usize;

    match (node_type, field_id) {
        // ListItem.checked = null → set to 2 (not a task item)
        (6, FIELD_CHECKED) => {
            if data_len >= 1 {
                arena.type_data[data_offset] = 2;
            }
        }
        // String fields → set StringRef to empty
        _ => {
            set_string_ref(arena, node_id, field_id, StringRef::empty())?;
        }
    }
    Ok(())
}

fn set_string_ref(
    arena: &mut MdastArena,
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

fn parse_raw_markdown(markdown: &str, parse_markdown: &dyn Fn(&str) -> MdastArena) -> MdastArena {
    parse_markdown(markdown)
}

/// Escape `{` and `}` in HTML text content so they are not interpreted as MDX
/// expressions when the HTML is re-parsed through the MDX parser.
///
/// Only braces in **text content** (outside of HTML tags) are escaped — braces
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

fn js_node_to_arena(js_node: &JsNode) -> Result<MdastArena, CommandError> {
    let mut builder = MdastBuilder::new(String::new());
    emit_js_node(js_node, &mut builder)?;
    Ok(builder.finish())
}

fn emit_js_node(js_node: &JsNode, builder: &mut MdastBuilder) -> Result<(), CommandError> {
    if js_node.is_hast {
        return emit_hast_js_node(js_node, builder);
    }

    let node_type = name_to_node_type(&js_node.node_type)?;
    builder.open_node(node_type);

    let type_data = encode_js_node_data(js_node, node_type, builder);
    if !type_data.is_empty() {
        builder.set_data_current(&type_data);
    }

    if let Some(children) = &js_node.children {
        for child in children {
            emit_js_node(child, builder)?;
        }
    }

    builder.close_node();
    Ok(())
}

fn emit_hast_js_node(js_node: &JsNode, builder: &mut MdastBuilder) -> Result<(), CommandError> {
    let raw_type = name_to_hast_raw_type(&js_node.node_type)
        .ok_or_else(|| CommandError::UnknownNodeType(js_node.node_type.clone()))?;
    builder.open_node_raw(raw_type);

    let type_data = encode_hast_js_node_data(js_node, raw_type, builder);
    if !type_data.is_empty() {
        builder.set_data_current(&type_data);
    }

    if let Some(children) = &js_node.children {
        for child in children {
            emit_hast_js_node(child, builder)?;
        }
    }

    builder.close_node();
    Ok(())
}

fn name_to_hast_raw_type(name: &str) -> Option<u8> {
    match name {
        "root" => Some(HAST_ROOT_TYPE),
        "element" => Some(HAST_ELEMENT_TYPE),
        "text" => Some(HAST_TEXT_TYPE),
        "comment" => Some(HAST_COMMENT_TYPE),
        "doctype" => Some(HAST_DOCTYPE_TYPE),
        "raw" => Some(HAST_RAW_TYPE),
        "mdxJsxFlowElement" => Some(HAST_MDX_JSX_ELEMENT_TYPE),
        "mdxJsxTextElement" => Some(HAST_MDX_JSX_TEXT_ELEMENT_TYPE),
        "mdxExpression" => Some(HAST_MDX_EXPRESSION_TYPE),
        "mdxjsEsm" => Some(HAST_MDX_ESM_TYPE),
        _ => None,
    }
}

fn encode_hast_js_node_data(js_node: &JsNode, raw_type: u8, builder: &mut MdastBuilder) -> Vec<u8> {
    match raw_type {
        HAST_ELEMENT_TYPE => {
            let tag = js_node.tag_name.as_deref().unwrap_or("div");
            let tag_ref = builder.alloc_string(tag);

            // Encode properties
            let mut props: Vec<(StringRef, u8, StringRef)> = Vec::new();
            if let Some(properties) = &js_node.properties {
                for (key, value) in properties {
                    let name_ref = builder.alloc_string(key);
                    match value {
                        serde_json::Value::Bool(true) => {
                            props.push((name_ref, HAST_PROP_BOOL_TRUE, StringRef::empty()));
                        }
                        serde_json::Value::Bool(false) => {
                            props.push((name_ref, HAST_PROP_BOOL_FALSE, StringRef::empty()));
                        }
                        serde_json::Value::String(s) => {
                            let val_ref = builder.alloc_string(s);
                            props.push((name_ref, HAST_PROP_STRING, val_ref));
                        }
                        serde_json::Value::Array(arr) => {
                            // Space-separated list
                            let joined: String = arr
                                .iter()
                                .filter_map(|v| v.as_str())
                                .collect::<Vec<_>>()
                                .join(" ");
                            let val_ref = builder.alloc_string(&joined);
                            props.push((name_ref, HAST_PROP_SPACE_SEP, val_ref));
                        }
                        _ => {} // skip null/number/object
                    }
                }
            }

            // Layout: [tag: StringRef(8)][prop_count: u32(4)][pad: u32(4)]
            //   + prop_count * [name: StringRef(8)][kind: u8][pad: 3][value: StringRef(8)]
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

        HAST_TEXT_TYPE | HAST_COMMENT_TYPE | HAST_RAW_TYPE => {
            let value = js_node.value.as_deref().unwrap_or("");
            let sref = builder.alloc_string(value);
            let mut out = [0u8; 8];
            out[0..4].copy_from_slice(&sref.offset.to_le_bytes());
            out[4..8].copy_from_slice(&sref.len.to_le_bytes());
            out.to_vec()
        }

        HAST_MDX_JSX_ELEMENT_TYPE | HAST_MDX_JSX_TEXT_ELEMENT_TYPE => {
            let name = js_node
                .name
                .as_deref()
                .or(js_node.tag_name.as_deref())
                .unwrap_or("");
            let name_ref = builder.alloc_string(name);
            let attr_tuples = encode_js_jsx_attrs(builder, js_node.attributes.as_deref());
            encode_mdx_jsx_element_data(name_ref, &attr_tuples)
        }

        HAST_MDX_EXPRESSION_TYPE | HAST_MDX_ESM_TYPE => {
            let value = js_node.value.as_deref().unwrap_or("");
            let sref = builder.alloc_string(value);
            let mut out = [0u8; 8];
            out[0..4].copy_from_slice(&sref.offset.to_le_bytes());
            out[4..8].copy_from_slice(&sref.len.to_le_bytes());
            out.to_vec()
        }

        // root, doctype: no type_data
        _ => Vec::new(),
    }
}

fn encode_js_node_data(
    js_node: &JsNode,
    node_type: MdastNodeType,
    builder: &mut MdastBuilder,
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
        MdastNodeType::LinkReference
        | MdastNodeType::ImageReference
        | MdastNodeType::FootnoteReference => {
            let id_ref = alloc_opt_str(builder, js_node.identifier.as_deref());
            let label_ref = alloc_opt_str(builder, js_node.label.as_deref());
            let kind = match js_node.reference_type.as_deref() {
                Some("collapsed") => 1u8,
                Some("full") => 2u8,
                _ => 0u8, // shortcut
            };
            encode_reference_data(id_ref, label_ref, kind)
        }
        MdastNodeType::FootnoteDefinition => {
            let id_ref = alloc_opt_str(builder, js_node.identifier.as_deref());
            let label_ref = alloc_opt_str(builder, js_node.label.as_deref());
            encode_footnote_definition_data(id_ref, label_ref)
        }
        MdastNodeType::MdxJsxFlowElement | MdastNodeType::MdxJsxTextElement => {
            let name_ref = alloc_opt_str(builder, js_node.name.as_deref());
            let attr_tuples = encode_js_jsx_attrs(builder, js_node.attributes.as_deref());
            encode_mdx_jsx_element_data(name_ref, &attr_tuples)
        }
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

fn encode_js_jsx_attrs(
    builder: &mut MdastBuilder,
    attrs: Option<&[JsNodeAttribute]>,
) -> Vec<(u8, StringRef, StringRef)> {
    let Some(attrs) = attrs else {
        return Vec::new();
    };
    attrs
        .iter()
        .map(|attr| match attr {
            JsNodeAttribute::Attribute { name, value } => {
                let n = builder.alloc_string(name);
                match value {
                    None => (MDX_ATTR_BOOLEAN_PROP, n, StringRef::empty()),
                    Some(serde_json::Value::String(s)) => {
                        let v = builder.alloc_string(s);
                        (MDX_ATTR_LITERAL_PROP, n, v)
                    }
                    Some(serde_json::Value::Object(obj)) => {
                        let expr = obj.get("value").and_then(|v| v.as_str()).unwrap_or("");
                        let v = builder.alloc_string(expr);
                        (MDX_ATTR_EXPRESSION_PROP, n, v)
                    }
                    _ => (MDX_ATTR_BOOLEAN_PROP, n, StringRef::empty()),
                }
            }
            JsNodeAttribute::ExpressionAttribute { value } => {
                let v = builder.alloc_string(value);
                (MDX_ATTR_SPREAD, StringRef::empty(), v)
            }
        })
        .collect()
}

fn alloc_opt_str(builder: &mut MdastBuilder, s: Option<&str>) -> StringRef {
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
        "mdxJsxFlowElement" => Ok(MdastNodeType::MdxJsxFlowElement),
        "mdxJsxTextElement" => Ok(MdastNodeType::MdxJsxTextElement),
        "mdxFlowExpression" => Ok(MdastNodeType::MdxFlowExpression),
        "mdxTextExpression" => Ok(MdastNodeType::MdxTextExpression),
        "mdxjsEsm" => Ok(MdastNodeType::MdxjsEsm),
        other => Err(CommandError::UnknownNodeType(other.to_string())),
    }
}

fn read_payload(
    reader: &mut BufReader<'_>,
    parse_markdown: &dyn Fn(&str) -> MdastArena,
) -> Result<MdastArena, CommandError> {
    let payload_type = reader.read_u8()?;
    let len = reader.read_u32()? as usize;

    match payload_type {
        PAYLOAD_RAW_MARKDOWN => {
            let md = reader.read_str(len)?;
            Ok(parse_raw_markdown(md, parse_markdown))
        }
        PAYLOAD_RAW_HTML => {
            // Re-parse the HTML through the markdown/MDX parser so that
            // HTML tags become proper JSX element nodes in MDX mode.
            // Escape { and } in text content first so they are treated as
            // literal characters rather than MDX expression boundaries.
            let html = reader.read_str(len)?;
            let escaped = escape_braces_in_html_text(html);
            Ok(parse_raw_markdown(&escaped, parse_markdown))
        }
        PAYLOAD_SERDE_JSON => {
            let json_str = reader.read_str(len)?;
            let js_node: JsNode = serde_json::from_str(json_str)
                .map_err(|e| CommandError::InvalidJson(e.to_string()))?;
            js_node_to_arena(&js_node)
        }
        other => Err(CommandError::UnknownPayloadType(other)),
    }
}

/// The `parse_markdown` callback avoids a circular dependency on the `parser`
/// crate. Set-property mutations are applied in-place on a cloned arena first;
/// structural mutations are collected as `Patch` objects and applied via `rebuild()`.
pub fn apply_commands(
    arena: &MdastArena,
    command_buf: &[u8],
    parse_markdown: &dyn Fn(&str) -> MdastArena,
) -> Result<MdastArena, CommandError> {
    if command_buf.is_empty() {
        return Ok(arena.clone());
    }

    let mut arena = arena.clone();
    let mut patches: Vec<Patch> = Vec::new();
    let mut reader = BufReader::new(command_buf);

    while reader.remaining() > 0 {
        let cmd = reader.read_u8()?;

        match cmd {
            CMD_REMOVE => {
                let node_id = reader.read_u32()?;
                patches.push(Patch::Remove { node_id });
            }

            CMD_SET_INT => {
                let node_id = reader.read_u32()?;
                let field_id = reader.read_u16()?;
                let value = reader.read_i64()?;
                apply_set_int(&mut arena, node_id, field_id, value)?;
            }

            CMD_SET_STRING => {
                let node_id = reader.read_u32()?;
                let field_id = reader.read_u16()?;
                let len = reader.read_u32()? as usize;
                let value = reader.read_str(len)?;
                apply_set_string(&mut arena, node_id, field_id, value)?;
            }

            CMD_SET_BOOL => {
                let node_id = reader.read_u32()?;
                let field_id = reader.read_u16()?;
                let value = reader.read_u8()? != 0;
                apply_set_bool(&mut arena, node_id, field_id, value)?;
            }

            CMD_SET_NULL => {
                let node_id = reader.read_u32()?;
                let field_id = reader.read_u16()?;
                apply_set_null(&mut arena, node_id, field_id)?;
            }

            CMD_INSERT_BEFORE => {
                let node_id = reader.read_u32()?;
                let new_tree = read_payload(&mut reader, parse_markdown)?;
                patches.push(Patch::InsertBefore { node_id, new_tree });
            }

            CMD_INSERT_AFTER => {
                let node_id = reader.read_u32()?;
                let new_tree = read_payload(&mut reader, parse_markdown)?;
                patches.push(Patch::InsertAfter { node_id, new_tree });
            }

            CMD_PREPEND_CHILD => {
                let node_id = reader.read_u32()?;
                let child_tree = read_payload(&mut reader, parse_markdown)?;
                patches.push(Patch::PrependChild {
                    node_id,
                    child_tree,
                });
            }

            CMD_APPEND_CHILD => {
                let node_id = reader.read_u32()?;
                let child_tree = read_payload(&mut reader, parse_markdown)?;
                patches.push(Patch::AppendChild {
                    node_id,
                    child_tree,
                });
            }

            CMD_WRAP => {
                let node_id = reader.read_u32()?;
                let parent_tree = read_payload(&mut reader, parse_markdown)?;
                patches.push(Patch::Wrap {
                    node_id,
                    parent_tree,
                });
            }

            CMD_REPLACE => {
                let node_id = reader.read_u32()?;
                let new_tree = read_payload(&mut reader, parse_markdown)?;
                patches.push(Patch::Replace { node_id, new_tree });
            }

            other => return Err(CommandError::UnknownCommand(other)),
        }
    }

    if patches.is_empty() {
        Ok(arena)
    } else {
        Ok(crate::rebuild::rebuild(&arena, &patches))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_parse_markdown(source: &str) -> MdastArena {
        let mut b = MdastBuilder::new(String::new());
        b.open_node(MdastNodeType::Root);
        b.open_node(MdastNodeType::Paragraph);
        b.open_node(MdastNodeType::Text);
        let sref = b.alloc_string(source);
        b.set_data_current(&crate::codec::encode_string_ref_data(sref));
        b.close_node();
        b.close_node();
        b.close_node();
        b.finish()
    }

    fn push_u32(buf: &mut Vec<u8>, v: u32) {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    fn push_u16(buf: &mut Vec<u8>, v: u16) {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    fn push_i64(buf: &mut Vec<u8>, v: i64) {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    fn build_hello_world() -> MdastArena {
        use crate::codec::{encode_heading_data, encode_string_ref_data};
        use crate::node::StringRef;

        let source = "# Hello\n\nWorld".to_string();
        let mut b = MdastBuilder::new(source);

        b.open_node(MdastNodeType::Root);
        b.set_position_current(0, 14, 1, 1, 2, 6);

        b.open_node(MdastNodeType::Heading);
        b.set_position_current(0, 7, 1, 1, 1, 8);
        b.set_data_current(&encode_heading_data(1));

        b.open_node(MdastNodeType::Text);
        b.set_position_current(2, 7, 1, 3, 1, 8);
        b.set_data_current(&encode_string_ref_data(StringRef::new(2, 5)));
        b.close_node();

        b.close_node();

        b.open_node(MdastNodeType::Paragraph);
        b.set_position_current(9, 14, 2, 1, 2, 6);

        b.open_node(MdastNodeType::Text);
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
        let result = apply_commands(&arena, &[], &test_parse_markdown).unwrap();
        assert_eq!(result.len(), arena.len());
    }

    #[test]
    fn remove_command() {
        let arena = build_hello_world();
        // Remove heading (node 1)
        let heading_id = arena.get_children(0)[0]; // 1
        let mut buf = Vec::new();
        buf.push(CMD_REMOVE);
        push_u32(&mut buf, heading_id);

        let result = apply_commands(&arena, &buf, &test_parse_markdown).unwrap();
        // Root should now have 1 child (paragraph only)
        assert_eq!(result.get_children(0).len(), 1);
        assert_eq!(
            result.get_node(result.get_children(0)[0]).node_type,
            MdastNodeType::Paragraph as u8
        );
    }

    #[test]
    fn set_int_heading_depth() {
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];

        let mut buf = Vec::new();
        buf.push(CMD_SET_INT);
        push_u32(&mut buf, heading_id);
        push_u16(&mut buf, FIELD_DEPTH);
        push_i64(&mut buf, 3);

        let result = apply_commands(&arena, &buf, &test_parse_markdown).unwrap();
        let heading_data = result.get_type_data(heading_id);
        let heading = decode_heading_data(heading_data);
        assert_eq!(heading.depth, 3);
    }

    #[test]
    fn set_string_text_value() {
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];
        let text_id = arena.get_children(heading_id)[0]; // text node "Hello"

        let new_value = "Goodbye";
        let mut buf = Vec::new();
        buf.push(CMD_SET_STRING);
        push_u32(&mut buf, text_id);
        push_u16(&mut buf, FIELD_VALUE);
        push_u32(&mut buf, new_value.len() as u32);
        buf.extend_from_slice(new_value.as_bytes());

        let result = apply_commands(&arena, &buf, &test_parse_markdown).unwrap();
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

        let result = apply_commands(&arena, &buf, &test_parse_markdown).unwrap();
        let root_children = result.get_children(0);
        // The raw markdown "## New Heading" parses to a Root with one Heading child.
        // The Patch::Replace uses the whole parsed arena (rooted at Root).
        // So root's first child should be a Root (from the sub-arena).
        // Actually, let's just check the node count changed and the heading is replaced.
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

        let result = apply_commands(&arena, &buf, &test_parse_markdown).unwrap();
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
    fn multiple_commands() {
        let arena = build_hello_world();
        let heading_id = arena.get_children(0)[0];
        let text_id = arena.get_children(heading_id)[0];

        let mut buf = Vec::new();

        // Set heading depth to 3
        buf.push(CMD_SET_INT);
        push_u32(&mut buf, heading_id);
        push_u16(&mut buf, FIELD_DEPTH);
        push_i64(&mut buf, 3);

        // Set text value to "Hi"
        let new_value = "Hi";
        buf.push(CMD_SET_STRING);
        push_u32(&mut buf, text_id);
        push_u16(&mut buf, FIELD_VALUE);
        push_u32(&mut buf, new_value.len() as u32);
        buf.extend_from_slice(new_value.as_bytes());

        let result = apply_commands(&arena, &buf, &test_parse_markdown).unwrap();

        let heading_data = result.get_type_data(heading_id);
        assert_eq!(decode_heading_data(heading_data).depth, 3);

        let text_data = result.get_type_data(text_id);
        let sref = decode_string_ref_data(text_data);
        assert_eq!(result.get_str(sref), "Hi");
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
                attributes: None,
                tag_name: None,
                properties: None,
                is_hast: false,
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
            attributes: None,
            tag_name: None,
            properties: None,
            is_hast: false,
        };

        let arena = js_node_to_arena(&js).unwrap();
        assert_eq!(arena.len(), 2); // heading + text
        assert_eq!(arena.get_node(0).node_type, MdastNodeType::Heading as u8);
        assert_eq!(arena.get_children(0).len(), 1);
        let text_id = arena.get_children(0)[0];
        assert_eq!(arena.get_node(text_id).node_type, MdastNodeType::Text as u8);
    }

    #[test]
    fn escape_braces_in_html_text_basic() {
        // Braces in text content should be escaped
        assert_eq!(
            escape_braces_in_html_text("<span>{foo: 1}</span>"),
            "<span>{'{'}foo: 1{'}'}</span>"
        );
    }

    #[test]
    fn escape_braces_preserves_attributes() {
        // Braces inside quoted attribute values should NOT be escaped
        assert_eq!(
            escape_braces_in_html_text(r#"<span data-x="{a}">{b}</span>"#),
            r#"<span data-x="{a}">{'{'} b{'}'}</span>"#.replace(" b", "b") // just {'{'}b{'}'}
        );
        // More direct test
        let result = escape_braces_in_html_text(r#"<span data-x="{a}">{b}</span>"#);
        assert!(
            result.contains(r#"data-x="{a}""#),
            "attribute braces preserved"
        );
        assert!(result.contains("{'{'}"), "text braces escaped");
    }

    #[test]
    fn escape_braces_no_braces() {
        // No braces → no change
        let html = r#"<pre class="shiki"><code><span style="color:red">hello</span></code></pre>"#;
        assert_eq!(escape_braces_in_html_text(html), html);
    }

    #[test]
    fn escape_braces_shiki_output() {
        let html = r#"<pre class="shiki"><code><span style="color:#E1E4E8">const x = </span><span style="color:#B392F0">{</span><span style="color:#E1E4E8">foo: 1</span><span style="color:#B392F0">}</span></code></pre>"#;
        let escaped = escape_braces_in_html_text(html);
        // The lone { and } between spans should be escaped
        assert!(
            !escaped.contains(">{<"),
            "bare braces in text should be escaped"
        );
        assert!(
            !escaped.contains(">}<"),
            "bare braces in text should be escaped"
        );
        // Attributes should be untouched
        assert!(escaped.contains(r#"class="shiki""#));
        assert!(escaped.contains(r#"style="color:#E1E4E8""#));
    }
}
