//! Type-specific data structs for `MdastArena::type_data`, serialized as raw
//! bytes via `#[repr(C)]` layout.

use crate::node::StringRef;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ImageData {
    pub url: StringRef,
    pub alt: StringRef,
    pub title: StringRef,
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

/// `checked`: 0 = unchecked, 1 = checked, 2 = not a task item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ListItemData {
    pub checked: u8,
    pub spread: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct FootnoteDefinitionData {
    pub identifier: StringRef,
    pub label: StringRef,
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

/// `meta.len == 0` means no meta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct MathData {
    pub meta: StringRef,
    pub value: StringRef,
}

/// Header for MdxJsxFlowElement and MdxJsxTextElement type_data.
/// `name.len == 0` means a fragment.
///
/// Full layout (variable-length):
///   [name: StringRef(8B)][attr_count: u32(4B)][_pad: u32(4B)] = 16-byte header
///   then attr_count * 20 bytes each:
///     [kind: u8(1B)][_pad: [u8;3](3B)][attr_name: StringRef(8B)][attr_value: StringRef(8B)]
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
}

/// Data for MdxFlowExpression, MdxTextExpression, MdxjsEsm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ExpressionData {
    pub value: StringRef,
}

pub const MDX_ATTR_BOOLEAN_PROP: u8 = 0;
pub const MDX_ATTR_LITERAL_PROP: u8 = 1;
pub const MDX_ATTR_EXPRESSION_PROP: u8 = 2;
pub const MDX_ATTR_SPREAD: u8 = 3;

/// Safety: T must be #[repr(C)] and contain no padding with undefined bytes.
unsafe fn struct_to_bytes<T: Copy>(val: &T) -> &[u8] {
    std::slice::from_raw_parts(val as *const T as *const u8, std::mem::size_of::<T>())
}

/// Safety: bytes must be at least size_of::<T>() bytes, properly aligned data
/// for type T (we copy so alignment doesn't matter).
unsafe fn bytes_to_struct<T: Copy>(bytes: &[u8]) -> T {
    assert!(
        bytes.len() >= std::mem::size_of::<T>(),
        "buffer too small: need {} bytes, got {}",
        std::mem::size_of::<T>(),
        bytes.len()
    );
    let mut val = std::mem::MaybeUninit::<T>::uninit();
    std::ptr::copy_nonoverlapping(
        bytes.as_ptr(),
        val.as_mut_ptr() as *mut u8,
        std::mem::size_of::<T>(),
    );
    val.assume_init()
}

pub fn encode_heading_data(depth: u8) -> Vec<u8> {
    let d = HeadingData { depth };
    unsafe { struct_to_bytes(&d) }.to_vec()
}

pub fn decode_heading_data(bytes: &[u8]) -> HeadingData {
    unsafe { bytes_to_struct(bytes) }
}

pub fn encode_link_data(url: StringRef, title: StringRef) -> Vec<u8> {
    let d = LinkData { url, title };
    unsafe { struct_to_bytes(&d) }.to_vec()
}

pub fn decode_link_data(bytes: &[u8]) -> LinkData {
    unsafe { bytes_to_struct(bytes) }
}

pub fn encode_image_data(url: StringRef, alt: StringRef, title: StringRef) -> Vec<u8> {
    let d = ImageData { url, alt, title };
    unsafe { struct_to_bytes(&d) }.to_vec()
}

pub fn decode_image_data(bytes: &[u8]) -> ImageData {
    unsafe { bytes_to_struct(bytes) }
}

pub fn encode_code_data(
    lang: StringRef,
    meta: StringRef,
    value: StringRef,
    fence_char: u8,
) -> Vec<u8> {
    let d = CodeData {
        lang,
        meta,
        value,
        fence_char,
        _pad: [0u8; 3],
    };
    unsafe { struct_to_bytes(&d) }.to_vec()
}

pub fn decode_code_data(bytes: &[u8]) -> CodeData {
    unsafe { bytes_to_struct(bytes) }
}

pub fn encode_list_data(ordered: bool, start: u32, spread: bool) -> Vec<u8> {
    let d = ListData {
        start,
        ordered,
        spread,
        _pad: [0; 2],
    };
    unsafe { struct_to_bytes(&d) }.to_vec()
}

pub fn decode_list_data(bytes: &[u8]) -> ListData {
    unsafe { bytes_to_struct(bytes) }
}

pub fn encode_list_item_data(checked: u8, spread: bool) -> Vec<u8> {
    let d = ListItemData { checked, spread };
    unsafe { struct_to_bytes(&d) }.to_vec()
}

pub fn decode_list_item_data(bytes: &[u8]) -> ListItemData {
    unsafe { bytes_to_struct(bytes) }
}

pub fn encode_table_data(alignments: &[ColumnAlign]) -> Vec<u8> {
    let header = TableData {
        align_count: alignments.len() as u32,
    };
    let mut bytes = unsafe { struct_to_bytes(&header) }.to_vec();
    for a in alignments {
        bytes.push(*a as u8);
    }
    bytes
}

pub fn decode_table_data(bytes: &[u8]) -> (TableData, Vec<ColumnAlign>) {
    let header: TableData = unsafe { bytes_to_struct(bytes) };
    let count = header.align_count as usize;
    let struct_size = std::mem::size_of::<TableData>();
    let align_bytes = &bytes[struct_size..struct_size + count];
    let alignments = align_bytes
        .iter()
        .map(|&b| ColumnAlign::from_u8(b).unwrap_or(ColumnAlign::None))
        .collect();
    (header, alignments)
}

pub fn encode_reference_data(
    identifier: StringRef,
    label: StringRef,
    reference_kind: u8,
) -> Vec<u8> {
    let d = ReferenceData {
        identifier,
        label,
        reference_kind,
        _pad: [0; 3],
    };
    unsafe { struct_to_bytes(&d) }.to_vec()
}

pub fn decode_reference_data(bytes: &[u8]) -> ReferenceData {
    unsafe { bytes_to_struct(bytes) }
}

pub fn encode_footnote_definition_data(identifier: StringRef, label: StringRef) -> Vec<u8> {
    let d = FootnoteDefinitionData { identifier, label };
    unsafe { struct_to_bytes(&d) }.to_vec()
}

pub fn decode_footnote_definition_data(bytes: &[u8]) -> FootnoteDefinitionData {
    unsafe { bytes_to_struct(bytes) }
}

pub fn encode_mdx_jsx_element_data(
    name: StringRef,
    attrs: &[(u8, StringRef, StringRef)],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + attrs.len() * 20);

    // 16-byte header: name(8) + attr_count(4) + _pad(4)
    out.extend_from_slice(unsafe { struct_to_bytes(&name) });
    out.extend_from_slice(&(attrs.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // _pad

    // Attribute entries: 20 bytes each
    for &(kind, attr_name, attr_value) in attrs {
        out.push(kind);
        out.extend_from_slice(&[0u8; 3]); // _pad
        out.extend_from_slice(unsafe { struct_to_bytes(&attr_name) });
        out.extend_from_slice(unsafe { struct_to_bytes(&attr_value) });
    }

    out
}

pub fn decode_mdx_jsx_element_name(bytes: &[u8]) -> StringRef {
    unsafe { bytes_to_struct(bytes) }
}

pub fn decode_mdx_jsx_attr_count(bytes: &[u8]) -> u32 {
    assert!(bytes.len() >= 12);
    u32::from_le_bytes(bytes[8..12].try_into().unwrap())
}

pub fn decode_mdx_jsx_attr(bytes: &[u8], index: u32) -> (u8, StringRef, StringRef) {
    let base = 16 + index as usize * 20;
    let kind = bytes[base];
    let attr_name: StringRef = unsafe { bytes_to_struct(&bytes[base + 4..]) };
    let attr_value: StringRef = unsafe { bytes_to_struct(&bytes[base + 12..]) };
    (kind, attr_name, attr_value)
}

pub fn encode_expression_data(value: StringRef) -> Vec<u8> {
    let d = ExpressionData { value };
    unsafe { struct_to_bytes(&d) }.to_vec()
}

pub fn decode_expression_data(bytes: &[u8]) -> ExpressionData {
    unsafe { bytes_to_struct(bytes) }
}

pub fn encode_definition_data(
    url: StringRef,
    title: StringRef,
    identifier: StringRef,
    label: StringRef,
) -> Vec<u8> {
    let d = DefinitionData {
        url,
        title,
        identifier,
        label,
    };
    unsafe { struct_to_bytes(&d) }.to_vec()
}

pub fn decode_definition_data(bytes: &[u8]) -> DefinitionData {
    unsafe { bytes_to_struct(bytes) }
}

pub fn encode_math_data(meta: StringRef, value: StringRef) -> Vec<u8> {
    let d = MathData { meta, value };
    unsafe { struct_to_bytes(&d) }.to_vec()
}

pub fn decode_math_data(bytes: &[u8]) -> MathData {
    unsafe { bytes_to_struct(bytes) }
}

/// Used by Text, InlineCode, Html, Yaml, Toml, and InlineMath nodes.
pub fn encode_string_ref_data(sr: StringRef) -> Vec<u8> {
    unsafe { struct_to_bytes(&sr) }.to_vec()
}

pub fn decode_string_ref_data(bytes: &[u8]) -> StringRef {
    unsafe { bytes_to_struct(bytes) }
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

    #[test]
    fn table_round_trip() {
        let aligns = vec![ColumnAlign::Left, ColumnAlign::Right, ColumnAlign::Center];
        let bytes = encode_table_data(&aligns);
        let (hdr, decoded) = decode_table_data(&bytes);
        assert_eq!(hdr.align_count, 3);
        assert_eq!(decoded, aligns);
    }
}
