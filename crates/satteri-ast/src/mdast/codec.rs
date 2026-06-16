//! Type-specific data structs for `Arena::type_data`, serialized as raw
//! bytes via `#[repr(C)]` layout.
//!
//! Invariant: in-arena multi-byte scalars are little-endian — the TS readers
//! decode the raw buffer as LE, and every supported target is LE, so the
//! explicit spelling also matches the native `#[repr(C)]` views.

use satteri_arena::StringRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct HeadingData {
    pub depth: u8,
}

/// `title.len == 0` means no title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct LinkData {
    pub url: StringRef,
    pub title: StringRef,
}

impl LinkData {
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.url.as_bytes());
        buf[8..16].copy_from_slice(&self.title.as_bytes());
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            url: StringRef::from_bytes(&bytes[0..8]),
            title: StringRef::from_bytes(&bytes[8..16]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ImageData {
    pub url: StringRef,
    pub alt: StringRef,
    pub title: StringRef,
}

impl ImageData {
    pub fn to_bytes(&self) -> [u8; 24] {
        let mut buf = [0u8; 24];
        buf[0..8].copy_from_slice(&self.url.as_bytes());
        buf[8..16].copy_from_slice(&self.alt.as_bytes());
        buf[16..24].copy_from_slice(&self.title.as_bytes());
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            url: StringRef::from_bytes(&bytes[0..8]),
            alt: StringRef::from_bytes(&bytes[8..16]),
            title: StringRef::from_bytes(&bytes[16..24]),
        }
    }
}

/// `fence_char`: e.g. b'`' or b'~'.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct CodeData {
    pub lang: StringRef,
    pub meta: StringRef,
    pub value: StringRef,
    pub fence_char: u8,
    pub _pad: [u8; 3],
}

impl CodeData {
    pub fn to_bytes(&self) -> [u8; 28] {
        let mut buf = [0u8; 28];
        buf[0..8].copy_from_slice(&self.lang.as_bytes());
        buf[8..16].copy_from_slice(&self.meta.as_bytes());
        buf[16..24].copy_from_slice(&self.value.as_bytes());
        buf[24] = self.fence_char;
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            lang: StringRef::from_bytes(&bytes[0..8]),
            meta: StringRef::from_bytes(&bytes[8..16]),
            value: StringRef::from_bytes(&bytes[16..24]),
            fence_char: bytes[24],
            _pad: [0; 3],
        }
    }
}

/// `start` is the starting number for ordered lists (ignored for unordered).
///
/// Field order chosen to avoid implicit padding:
///   start(u32) @ 0, ordered(bool) @ 4, spread(bool) @ 5, _pad(2) @ 6 → 8 bytes total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ListData {
    pub start: u32,
    pub ordered: bool,
    pub spread: bool,
    pub _pad: [u8; 2],
}

impl ListData {
    pub fn to_bytes(&self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        buf[0..4].copy_from_slice(&self.start.to_le_bytes());
        buf[4] = self.ordered as u8;
        buf[5] = self.spread as u8;
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            start: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            ordered: bytes[4] != 0,
            spread: bytes[5] != 0,
            _pad: [0; 2],
        }
    }
}

/// `checked`: 0 = unchecked, 1 = checked, 2 = not a task item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ListItemData {
    pub checked: u8,
    pub spread: bool,
}

impl ListItemData {
    pub fn to_bytes(&self) -> [u8; 2] {
        [self.checked, self.spread as u8]
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            checked: bytes[0],
            spread: bytes[1] != 0,
        }
    }
}

/// Immediately followed in type_data by `align_count` [`ColumnAlign`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct TableData {
    pub align_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ColumnAlign {
    None = 0,
    Left = 1,
    Right = 2,
    Center = 3,
}

impl ColumnAlign {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(ColumnAlign::None),
            1 => Some(ColumnAlign::Left),
            2 => Some(ColumnAlign::Right),
            3 => Some(ColumnAlign::Center),
            _ => None,
        }
    }
}

/// Data for LinkReference, ImageReference, FootnoteReference nodes.
/// `reference_kind`: 0=Shortcut, 1=Collapsed, 2=Full.
///
/// Explicit _pad avoids implicit trailing bytes: 8+8+1+3 = 20 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ReferenceData {
    pub identifier: StringRef,
    pub label: StringRef,
    pub reference_kind: u8,
    pub _pad: [u8; 3],
}

impl ReferenceData {
    pub fn to_bytes(&self) -> [u8; 20] {
        let mut buf = [0u8; 20];
        buf[0..8].copy_from_slice(&self.identifier.as_bytes());
        buf[8..16].copy_from_slice(&self.label.as_bytes());
        buf[16] = self.reference_kind;
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            identifier: StringRef::from_bytes(&bytes[0..8]),
            label: StringRef::from_bytes(&bytes[8..16]),
            reference_kind: bytes[16],
            _pad: [0; 3],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct FootnoteDefinitionData {
    pub identifier: StringRef,
    pub label: StringRef,
}

impl FootnoteDefinitionData {
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.identifier.as_bytes());
        buf[8..16].copy_from_slice(&self.label.as_bytes());
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            identifier: StringRef::from_bytes(&bytes[0..8]),
            label: StringRef::from_bytes(&bytes[8..16]),
        }
    }
}

/// `title.len == 0` means no title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DefinitionData {
    pub url: StringRef,
    pub title: StringRef,
    pub identifier: StringRef,
    pub label: StringRef,
}

impl DefinitionData {
    pub fn to_bytes(&self) -> [u8; 32] {
        let mut buf = [0u8; 32];
        buf[0..8].copy_from_slice(&self.url.as_bytes());
        buf[8..16].copy_from_slice(&self.title.as_bytes());
        buf[16..24].copy_from_slice(&self.identifier.as_bytes());
        buf[24..32].copy_from_slice(&self.label.as_bytes());
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            url: StringRef::from_bytes(&bytes[0..8]),
            title: StringRef::from_bytes(&bytes[8..16]),
            identifier: StringRef::from_bytes(&bytes[16..24]),
            label: StringRef::from_bytes(&bytes[24..32]),
        }
    }
}

/// `meta.len == 0` means no meta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct MathData {
    pub meta: StringRef,
    pub value: StringRef,
}

impl MathData {
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.meta.as_bytes());
        buf[8..16].copy_from_slice(&self.value.as_bytes());
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            meta: StringRef::from_bytes(&bytes[0..8]),
            value: StringRef::from_bytes(&bytes[8..16]),
        }
    }
}

/// Header for directive type_data (ContainerDirective, LeafDirective, TextDirective).
///
/// Full layout (variable-length): this 16-byte header, then `attr_count`
/// `DirectiveAttributeEntry` items (16 bytes each) starting at 16. Pinned by
/// the generated layout asserts so the registry's tail header offsets can't
/// drift from the codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DirectiveData {
    pub name: StringRef,
    pub attr_count: u32,
    pub _pad: u32,
}

/// One stored directive attribute entry (the `encode_directive_data` items);
/// pinned by the generated layout asserts so the registry's tail offsets
/// can't drift from the codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DirectiveAttributeEntry {
    pub key: StringRef,
    pub value: StringRef,
}

pub fn encode_directive_data(name: StringRef, attrs: &[(StringRef, StringRef)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + attrs.len() * 16);
    out.extend_from_slice(&name.as_bytes());
    out.extend_from_slice(&(attrs.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // _pad
    for &(key, value) in attrs {
        out.extend_from_slice(&key.as_bytes());
        out.extend_from_slice(&value.as_bytes());
    }
    out
}

pub fn decode_directive_name(bytes: &[u8]) -> StringRef {
    StringRef::from_bytes(bytes)
}

pub fn decode_directive_attr_count(bytes: &[u8]) -> u32 {
    assert!(bytes.len() >= 12);
    u32::from_le_bytes(bytes[8..12].try_into().unwrap())
}

pub fn decode_directive_attr(bytes: &[u8], index: u32) -> (StringRef, StringRef) {
    let base = 16 + index as usize * 16;
    let key = StringRef::from_bytes(&bytes[base..base + 8]);
    let value = StringRef::from_bytes(&bytes[base + 8..base + 16]);
    (key, value)
}

/// Header for MdxJsxFlowElement and MdxJsxTextElement type_data.
/// `name.len == 0` means a fragment.
///
/// Full layout (variable-length): this 16-byte header, then `attr_count`
/// `MdxJsxAttributeEntry` items (20 bytes each) starting at 16. Pinned by the
/// generated layout asserts; not `mdx`-gated, the asserts reference it
/// unconditionally.
///
/// `explicit_jsx` mirrors `_mdxExplicitJsx` from `@mdx-js/mdx`. It's a fast
/// read path for the hast→recma transform — the same bit is *also* mirrored
/// into the node's `data` blob (`{"_mdxExplicitJsx":true}`) so plugins can
/// read/write it as `node.data._mdxExplicitJsx`. The JSON side is the
/// plugin-visible source of truth; this byte exists so the transform
/// doesn't have to parse JSON per node.
///
/// Attribute kinds (from jsx_attr_parser):
///   0 = BooleanProp (name only, no value)
///   1 = LiteralProp (name="literal")
///   2 = ExpressionProp (name={expr})
///   3 = Spread ({...expr})
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct MdxJsxElementData {
    pub name: StringRef,
    pub attr_count: u32,
    pub explicit_jsx: u8,
    pub _pad: [u8; 3],
}

/// One stored MDX JSX attribute entry (the `encode_mdx_jsx_element_data`
/// items, MDAST and HAST alike); pinned by the generated layout asserts.
/// Not `mdx`-gated: the asserts reference it unconditionally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct MdxJsxAttributeEntry {
    pub kind: u8,
    pub _pad: [u8; 3],
    pub name: StringRef,
    pub value: StringRef,
}

/// Data for MdxFlowExpression, MdxTextExpression, MdxjsEsm.
#[cfg(feature = "mdx")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ExpressionData {
    pub value: StringRef,
}

#[cfg(feature = "mdx")]
impl ExpressionData {
    pub fn to_bytes(&self) -> [u8; 8] {
        self.value.as_bytes()
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            value: StringRef::from_bytes(bytes),
        }
    }
}

// MDX_ATTR_* constants are in crate::shared, re-export for backwards compat
#[cfg(feature = "mdx")]
pub use crate::shared::{
    MDX_ATTR_BOOLEAN_PROP, MDX_ATTR_EXPRESSION_PROP, MDX_ATTR_LITERAL_PROP, MDX_ATTR_SPREAD,
};

pub fn encode_heading_data(depth: u8) -> Vec<u8> {
    vec![depth]
}

pub fn decode_heading_data(bytes: &[u8]) -> HeadingData {
    HeadingData { depth: bytes[0] }
}

pub fn encode_link_data(url: StringRef, title: StringRef) -> Vec<u8> {
    LinkData { url, title }.to_bytes().to_vec()
}

pub fn decode_link_data(bytes: &[u8]) -> LinkData {
    LinkData::from_bytes(bytes)
}

pub fn encode_image_data(url: StringRef, alt: StringRef, title: StringRef) -> Vec<u8> {
    ImageData { url, alt, title }.to_bytes().to_vec()
}

pub fn decode_image_data(bytes: &[u8]) -> ImageData {
    ImageData::from_bytes(bytes)
}

pub fn encode_code_data(
    lang: StringRef,
    meta: StringRef,
    value: StringRef,
    fence_char: u8,
) -> Vec<u8> {
    CodeData {
        lang,
        meta,
        value,
        fence_char,
        _pad: [0; 3],
    }
    .to_bytes()
    .to_vec()
}

pub fn decode_code_data(bytes: &[u8]) -> CodeData {
    CodeData::from_bytes(bytes)
}

pub fn encode_list_data(ordered: bool, start: u32, spread: bool) -> Vec<u8> {
    ListData {
        start,
        ordered,
        spread,
        _pad: [0; 2],
    }
    .to_bytes()
    .to_vec()
}

pub fn decode_list_data(bytes: &[u8]) -> ListData {
    ListData::from_bytes(bytes)
}

pub fn encode_list_item_data(checked: u8, spread: bool) -> Vec<u8> {
    ListItemData { checked, spread }.to_bytes().to_vec()
}

pub fn decode_list_item_data(bytes: &[u8]) -> ListItemData {
    ListItemData::from_bytes(bytes)
}

pub fn encode_table_data(alignments: &[ColumnAlign]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + alignments.len());
    bytes.extend_from_slice(&(alignments.len() as u32).to_le_bytes());
    for a in alignments {
        bytes.push(*a as u8);
    }
    bytes
}

pub fn decode_table_alignments(bytes: &[u8]) -> Vec<ColumnAlign> {
    if bytes.len() < 4 {
        return Vec::new();
    }
    let count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let end = 4 + count;
    if bytes.len() < end {
        return Vec::new();
    }
    bytes[4..end]
        .iter()
        .map(|&b| ColumnAlign::from_u8(b).unwrap_or(ColumnAlign::None))
        .collect()
}

pub fn encode_reference_data(
    identifier: StringRef,
    label: StringRef,
    reference_kind: u8,
) -> Vec<u8> {
    ReferenceData {
        identifier,
        label,
        reference_kind,
        _pad: [0; 3],
    }
    .to_bytes()
    .to_vec()
}

pub fn decode_reference_data(bytes: &[u8]) -> ReferenceData {
    ReferenceData::from_bytes(bytes)
}

/// `imageReference` layout: 20-byte [`ReferenceData`] header followed by an
/// 8-byte [`StringRef`] for `alt`. When bytes aren't present (data.len() < 28),
/// `alt` falls back to empty — callers can then derive it from children.
pub fn encode_image_reference_data(
    identifier: StringRef,
    label: StringRef,
    reference_kind: u8,
    alt: StringRef,
) -> Vec<u8> {
    let mut bytes = encode_reference_data(identifier, label, reference_kind);
    bytes.extend_from_slice(&alt.as_bytes());
    bytes
}

pub fn decode_image_reference_alt(bytes: &[u8]) -> StringRef {
    if bytes.len() >= 28 {
        StringRef::from_bytes(&bytes[20..28])
    } else {
        StringRef::empty()
    }
}

pub fn decode_footnote_definition_data(bytes: &[u8]) -> FootnoteDefinitionData {
    FootnoteDefinitionData::from_bytes(bytes)
}

pub fn encode_footnote_definition_data(identifier: StringRef, label: StringRef) -> Vec<u8> {
    FootnoteDefinitionData { identifier, label }
        .to_bytes()
        .to_vec()
}

#[cfg(feature = "mdx")]
pub fn encode_mdx_jsx_element_data(
    name: StringRef,
    attrs: &[(u8, StringRef, StringRef)],
    explicit_jsx: bool,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + attrs.len() * 20);

    out.extend_from_slice(&name.as_bytes());
    out.extend_from_slice(&(attrs.len() as u32).to_le_bytes());
    out.push(if explicit_jsx { 1 } else { 0 });
    out.extend_from_slice(&[0u8; 3]);

    // Attribute entries: 20 bytes each
    for &(kind, attr_name, attr_value) in attrs {
        out.push(kind);
        out.extend_from_slice(&[0u8; 3]); // _pad
        out.extend_from_slice(&attr_name.as_bytes());
        out.extend_from_slice(&attr_value.as_bytes());
    }

    out
}

#[cfg(feature = "mdx")]
pub fn decode_mdx_jsx_element_name(bytes: &[u8]) -> StringRef {
    StringRef::from_bytes(bytes)
}

#[cfg(feature = "mdx")]
pub fn decode_mdx_jsx_attr_count(bytes: &[u8]) -> u32 {
    assert!(bytes.len() >= 12);
    u32::from_le_bytes(bytes[8..12].try_into().unwrap())
}

/// Whether the JSX element was authored in MDX source (mirrors
/// `_mdxExplicitJsx` in `node.data`). Defaults to `false` on short buffers.
#[cfg(feature = "mdx")]
pub fn decode_mdx_jsx_explicit(bytes: &[u8]) -> bool {
    bytes.get(12).is_some_and(|&b| b != 0)
}

#[cfg(feature = "mdx")]
pub fn decode_mdx_jsx_attr(bytes: &[u8], index: u32) -> (u8, StringRef, StringRef) {
    let base = 16 + index as usize * 20;
    let kind = bytes[base];
    let attr_name = StringRef::from_bytes(&bytes[base + 4..base + 12]);
    let attr_value = StringRef::from_bytes(&bytes[base + 12..base + 20]);
    (kind, attr_name, attr_value)
}

#[cfg(feature = "mdx")]
pub fn encode_expression_data(value: StringRef) -> Vec<u8> {
    ExpressionData { value }.to_bytes().to_vec()
}

#[cfg(feature = "mdx")]
pub fn decode_expression_data(bytes: &[u8]) -> ExpressionData {
    ExpressionData::from_bytes(bytes)
}

pub fn encode_definition_data(
    url: StringRef,
    title: StringRef,
    identifier: StringRef,
    label: StringRef,
) -> Vec<u8> {
    DefinitionData {
        url,
        title,
        identifier,
        label,
    }
    .to_bytes()
    .to_vec()
}

pub fn decode_definition_data(bytes: &[u8]) -> DefinitionData {
    DefinitionData::from_bytes(bytes)
}

pub fn encode_math_data(meta: StringRef, value: StringRef) -> Vec<u8> {
    MathData { meta, value }.to_bytes().to_vec()
}

pub fn decode_math_data(bytes: &[u8]) -> MathData {
    MathData::from_bytes(bytes)
}

pub fn encode_string_ref_data(sr: StringRef) -> Vec<u8> {
    sr.as_bytes().to_vec()
}

pub fn decode_string_ref_data(bytes: &[u8]) -> StringRef {
    StringRef::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_round_trip() {
        let bytes = encode_heading_data(3);
        let d = decode_heading_data(&bytes);
        assert_eq!(d.depth, 3);
    }

    #[test]
    fn link_round_trip() {
        let url = StringRef::new(0, 10);
        let title = StringRef::new(11, 5);
        let bytes = encode_link_data(url, title);
        let d = decode_link_data(&bytes);
        assert_eq!(d.url, url);
        assert_eq!(d.title, title);
    }
}
