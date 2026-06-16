//! Codec helpers for HAST type-specific data encoding/decoding.
//!
//! Element type_data layout:
//!   [tag_name: StringRef(8B)][prop_count: u32(4B)][_pad: u32(4B)] = 16-byte header
//!   then prop_count * PropertyEntry (20 bytes each):
//!     [name: StringRef(8B)][value_type: u8(1B)][_pad: [u8;3](3B)][value: StringRef(8B)]
//!
//! Text/Comment/Raw type_data: just StringRef (8 bytes).

use satteri_arena::StringRef;

/// Stored element header (`encode_element_data`): tag name, then the property
/// count, then `prop_count` `PropertyEntry` items (20 bytes each) starting at
/// 16. Pinned by the generated layout asserts so the registry's tail header
/// offsets can't drift from the codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ElementData {
    pub tag_name: StringRef,
    pub prop_count: u32,
    pub _pad: u32,
}

/// One stored element property entry (the `encode_element_data` items);
/// pinned by the generated layout asserts so the registry's tail offsets
/// can't drift from the codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PropertyEntry {
    pub name: StringRef,
    pub value_type: u8,
    pub _pad: [u8; 3],
    pub value: StringRef,
}

/// Props tuple: (name, value_type, value).
pub fn encode_element_data(tag_name: StringRef, props: &[(StringRef, u8, StringRef)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + props.len() * 20);
    encode_element_data_into(tag_name, props, &mut out);
    out
}

/// Write element data directly into a target buffer (zero-alloc).
pub fn encode_element_data_into(
    tag_name: StringRef,
    props: &[(StringRef, u8, StringRef)],
    out: &mut Vec<u8>,
) {
    out.extend_from_slice(&tag_name.as_bytes());
    out.extend_from_slice(&(props.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    for &(name, value_type, value) in props {
        out.extend_from_slice(&name.as_bytes());
        out.push(value_type);
        out.extend_from_slice(&[0u8; 3]);
        out.extend_from_slice(&value.as_bytes());
    }
}

pub fn decode_element_tag(data: &[u8]) -> StringRef {
    StringRef::from_bytes(&data[0..8])
}

pub fn decode_element_prop_count(data: &[u8]) -> u32 {
    u32::from_le_bytes(data[8..12].try_into().unwrap())
}

/// Returns (name, value_type, value).
pub fn decode_element_prop(data: &[u8], index: u32) -> (StringRef, u8, StringRef) {
    let base = 16 + index as usize * 20;
    let name = StringRef::from_bytes(&data[base..base + 8]);
    let value_type = data[base + 8];
    let value = StringRef::from_bytes(&data[base + 12..base + 20]);
    (name, value_type, value)
}

pub fn encode_text_data(sr: StringRef) -> Vec<u8> {
    sr.as_bytes().to_vec()
}

pub fn decode_text_data(data: &[u8]) -> StringRef {
    StringRef::from_bytes(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_ref_round_trip() {
        let sr = StringRef::new(42, 10);
        let encoded = encode_text_data(sr);
        let decoded = decode_text_data(&encoded);
        assert_eq!(decoded.offset, 42);
        assert_eq!(decoded.len, 10);
    }

    #[test]
    fn element_no_props() {
        let tag = StringRef::new(0, 3);
        let data = encode_element_data(tag, &[]);
        assert_eq!(data.len(), 16);
        assert_eq!(decode_element_tag(&data).offset, 0);
        assert_eq!(decode_element_tag(&data).len, 3);
        assert_eq!(decode_element_prop_count(&data), 0);
    }

    #[test]
    fn element_with_props() {
        let tag = StringRef::new(0, 1);
        let name = StringRef::new(5, 4);
        let value = StringRef::new(10, 6);
        let props = vec![(name, crate::shared::PROP_STRING, value)];
        let data = encode_element_data(tag, &props);
        assert_eq!(data.len(), 36); // 16 + 20
        assert_eq!(decode_element_prop_count(&data), 1);
        let (n, kind, v) = decode_element_prop(&data, 0);
        assert_eq!(n.offset, 5);
        assert_eq!(n.len, 4);
        assert_eq!(kind, crate::shared::PROP_STRING);
        assert_eq!(v.offset, 10);
        assert_eq!(v.len, 6);
    }

    #[test]
    fn text_data_round_trip() {
        let sr = StringRef::new(100, 20);
        let data = encode_text_data(sr);
        let decoded = decode_text_data(&data);
        assert_eq!(decoded.offset, 100);
        assert_eq!(decoded.len, 20);
    }
}
