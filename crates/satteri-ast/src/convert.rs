//! Convert an MDAST arena to a HAST arena.

use rustc_hash::FxHashMap;
use satteri_arena::{decode_string_ref_data, Arena, ArenaBuilder, Hast, Mdast, StringRef};

use crate::hast::codec::encode_element_data_into;
use crate::hast::HastNodeType;
use crate::mdast::{
    decode_code_data, decode_definition_data, decode_expression_data,
    decode_footnote_definition_data, decode_heading_data, decode_image_data,
    decode_image_reference_alt, decode_link_data, decode_list_data, decode_list_item_data,
    decode_math_data, decode_mdx_jsx_attr, decode_mdx_jsx_attr_count, decode_mdx_jsx_element_name,
    decode_mdx_jsx_explicit, decode_reference_data, decode_table_alignments,
    encode_mdx_jsx_element_data, ColumnAlign, ListItemData, MdastNodeType,
};
use crate::shared::{PROP_BOOL_FALSE, PROP_BOOL_TRUE, PROP_INT, PROP_SPACE_SEP, PROP_STRING};

/// Owned view over `data.hName` / `data.hProperties` / `data.hChildren` for a
/// single mdast node. Mirrors mdast-util-to-hast's `applyData` semantics: a JS
/// plugin sets these fields and the converter honours them when emitting hast.
struct HData {
    root: Option<serde_json::Value>,
}

impl HData {
    fn read(view: &Arena<Mdast>, node_id: u32) -> Self {
        let bytes = match view.get_node_data(node_id) {
            Some(b) if !b.is_empty() => b,
            _ => return HData { root: None },
        };
        // Most node_data blobs are unrelated to hast emission (e.g. code
        // language/meta JSON, plugin-private metadata). Bail out before paying
        // for `serde_json::from_slice` if none of the three keys are present
        // as quoted JSON keys. The substrings include the leading `"` so they
        // can't match an `hName` *value* embedded in user data.
        if !contains_h_key(bytes) {
            return HData { root: None };
        }
        let parsed: serde_json::Value = match serde_json::from_slice(bytes) {
            Ok(v) => v,
            Err(_) => return HData { root: None },
        };
        if !matches!(parsed, serde_json::Value::Object(_)) {
            return HData { root: None };
        }
        HData { root: Some(parsed) }
    }

    fn h_name(&self) -> Option<&str> {
        self.root.as_ref()?.as_object()?.get("hName")?.as_str()
    }

    fn h_properties(&self) -> Option<&serde_json::Map<String, serde_json::Value>> {
        self.root
            .as_ref()?
            .as_object()?
            .get("hProperties")?
            .as_object()
    }

    fn h_children(&self) -> Option<&[serde_json::Value]> {
        self.root
            .as_ref()?
            .as_object()?
            .get("hChildren")?
            .as_array()
            .map(|v| v.as_slice())
    }

    fn is_empty(&self) -> bool {
        self.h_name().is_none() && self.h_properties().is_none() && self.h_children().is_none()
    }
}

/// Quick byte scan for any of the three quoted h-keys. False positives just
/// fall through to a real JSON parse, so this needs to be cheap, not perfect.
fn contains_h_key(bytes: &[u8]) -> bool {
    // The shortest key is `"hName"` (7 bytes including quotes). Anything
    // smaller can't match.
    if bytes.len() < 7 {
        return false;
    }
    // Walk the buffer once; whenever we see `"h`, peek at the next byte to
    // route to the right candidate. Avoids three full passes.
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'"' && bytes[i + 1] == b'h' && i + 2 < bytes.len() {
            let rest = &bytes[i + 2..];
            let matched = match rest.first() {
                Some(b'N') => rest.starts_with(b"Name\""),
                Some(b'P') => rest.starts_with(b"Properties\""),
                Some(b'C') => rest.starts_with(b"Children\""),
                _ => false,
            };
            if matched {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Convert a JSON value to an h-property entry.
/// Returns `None` for `null`/`undefined` (property is stripped) and for
/// unsupported value shapes (e.g. nested objects).
fn json_value_to_prop(
    builder: &mut ArenaBuilder<Hast>,
    value: &serde_json::Value,
) -> Option<(u8, StringRef)> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(true) => Some((PROP_BOOL_TRUE, StringRef::empty())),
        serde_json::Value::Bool(false) => Some((PROP_BOOL_FALSE, StringRef::empty())),
        serde_json::Value::String(s) => Some((PROP_STRING, builder.alloc_string(s))),
        serde_json::Value::Number(n) => {
            let s = n.to_string();
            Some((PROP_INT, builder.alloc_string(&s)))
        }
        serde_json::Value::Array(arr) => {
            let joined: String = arr
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            Some((PROP_SPACE_SEP, builder.alloc_string(&joined)))
        }
        serde_json::Value::Object(_) => None,
    }
}

/// Merge default specs with `hProperties` overrides. Later wins; `null` strips.
/// Returns a list of `PropData` ready to be passed to `open_element_with_props`.
fn merged_h_props(
    builder: &mut ArenaBuilder<Hast>,
    defaults: &[(&str, u8, StringRef)],
    overrides: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Vec<PropData> {
    let mut entries: Vec<(String, u8, StringRef)> = defaults
        .iter()
        .map(|(n, k, v)| ((*n).to_string(), *k, *v))
        .collect();
    if let Some(overrides) = overrides {
        for (name, value) in overrides {
            let idx = entries.iter().position(|(n, _, _)| n == name);
            let entry = json_value_to_prop(builder, value);
            match (entry, idx) {
                (None, Some(i)) => {
                    entries.remove(i);
                }
                (None, None) => {}
                (Some((kind, val)), Some(i)) => entries[i] = (name.clone(), kind, val),
                (Some((kind, val)), None) => entries.push((name.clone(), kind, val)),
            }
        }
    }
    entries
        .into_iter()
        .map(|(name, kind, value)| PropData {
            name_ref: builder.alloc_string(&name),
            value_kind: kind,
            value_ref: value,
        })
        .collect()
}

/// Whether the caller still needs to emit the mdast node's children, or
/// whether `hChildren` already replaced them.
enum ChildrenAction {
    Recurse,
    Replaced,
}

/// Open an HTML element honouring `hName` / `hProperties` / `hChildren` on the
/// source mdast node. The caller is responsible for `close_node` and
/// `copy_position` afterwards.
fn open_h_element(
    builder: &mut ArenaBuilder<Hast>,
    view: &Arena<Mdast>,
    src_id: u32,
    default_tag: &str,
    default_specs: &[(&str, u8, StringRef)],
) -> ChildrenAction {
    let h = HData::read(view, src_id);
    if h.is_empty() {
        if default_specs.is_empty() {
            open_element(builder, default_tag);
        } else {
            open_element_with_specs(builder, default_tag, default_specs);
        }
        return ChildrenAction::Recurse;
    }
    let tag = h.h_name().unwrap_or(default_tag);
    let props = merged_h_props(builder, default_specs, h.h_properties());
    open_element_with_props(builder, tag, &props);
    if let Some(children) = h.h_children() {
        emit_h_children(builder, children);
        ChildrenAction::Replaced
    } else {
        ChildrenAction::Recurse
    }
}

/// Same as `open_h_element` but for void elements (no children, builder closes
/// the node automatically). `hChildren` is ignored on void elements.
fn add_h_void_element(
    builder: &mut ArenaBuilder<Hast>,
    view: &Arena<Mdast>,
    src_id: u32,
    default_tag: &str,
    default_specs: &[(&str, u8, StringRef)],
) -> u32 {
    let h = HData::read(view, src_id);
    if h.is_empty() {
        return if default_specs.is_empty() {
            add_void_element(builder, default_tag)
        } else {
            add_void_element_with_specs(builder, default_tag, default_specs)
        };
    }
    let tag = h.h_name().unwrap_or(default_tag);
    let props = merged_h_props(builder, default_specs, h.h_properties());
    add_void_element_with_props(builder, tag, &props)
}

/// Emit a list of hast nodes (from `data.hChildren`) into the builder. The
/// children are JSON-encoded hast nodes — `element` / `text` / `comment` /
/// `raw` are supported; anything else is silently skipped.
fn emit_h_children(builder: &mut ArenaBuilder<Hast>, children: &[serde_json::Value]) {
    for child in children {
        emit_h_child(builder, child);
    }
}

fn emit_h_child(builder: &mut ArenaBuilder<Hast>, child: &serde_json::Value) {
    let Some(obj) = child.as_object() else {
        return;
    };
    let ty = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match ty {
        "element" => {
            let tag = obj.get("tagName").and_then(|v| v.as_str()).unwrap_or("div");
            let mut props: Vec<PropData> = Vec::new();
            if let Some(properties) = obj.get("properties").and_then(|v| v.as_object()) {
                for (name, value) in properties {
                    if let Some((kind, val)) = json_value_to_prop(builder, value) {
                        props.push(PropData {
                            name_ref: builder.alloc_string(name),
                            value_kind: kind,
                            value_ref: val,
                        });
                    }
                }
            }
            open_element_with_props(builder, tag, &props);
            if let Some(grand) = obj.get("children").and_then(|v| v.as_array()) {
                emit_h_children(builder, grand);
            }
            builder.close_node();
        }
        "text" => {
            let value = obj.get("value").and_then(|v| v.as_str()).unwrap_or("");
            add_text_node(builder, value);
        }
        "comment" => {
            let value = obj.get("value").and_then(|v| v.as_str()).unwrap_or("");
            let value_ref = builder.alloc_string(value);
            let leaf_id = builder.add_leaf_raw(HastNodeType::Comment as u8);
            builder
                .arena_mut()
                .set_type_data(leaf_id, &value_ref.as_bytes());
        }
        "raw" => {
            let value = obj.get("value").and_then(|v| v.as_str()).unwrap_or("");
            add_raw_node(builder, value);
        }
        _ => {}
    }
}

fn encode_url(builder: &mut ArenaBuilder<Hast>, url: &str) -> StringRef {
    let bytes = url.as_bytes();
    // A `%` is only "safe" when it's the start of a valid percent-encoding
    // (followed by two hex digits). An invalid `%2X` (X not hex) means the
    // `%` itself should be encoded as `%25` — matches remark's behavior.
    let pct_safe = |i: usize| -> bool {
        i + 2 < bytes.len() && is_hex_digit(bytes[i + 1]) && is_hex_digit(bytes[i + 2])
    };
    let needs_encode = bytes.iter().enumerate().any(|(i, &b)| {
        if b == b'%' {
            !pct_safe(i)
        } else {
            !is_url_safe(b)
        }
    });
    if !needs_encode {
        return builder.alloc_string(url);
    }
    let mut encoded = String::with_capacity(url.len() * 2);
    for (i, &byte) in bytes.iter().enumerate() {
        let safe = if byte == b'%' {
            pct_safe(i)
        } else {
            is_url_safe(byte)
        };
        if safe {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(hex_digit(byte >> 4));
            encoded.push(hex_digit(byte & 0xf));
        }
    }
    builder.alloc_string(&encoded)
}

fn is_hex_digit(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

fn is_url_safe(b: u8) -> bool {
    matches!(b,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
        | b'-' | b'.' | b'_' | b'~'
        | b':' | b'/' | b'?' | b'#' | b'@'
        | b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
    )
}

fn hex_digit(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'A' + n - 10) as char,
        _ => unreachable!(),
    }
}

/// Convert an MDAST arena directly to a HAST arena.
pub fn mdast_arena_to_hast_arena(source: &Arena<Mdast>) -> Arena<Hast> {
    let src = source.source();
    let mut builder: ArenaBuilder<Hast> = ArenaBuilder::new(src.to_string());
    let n = source.len();
    builder.arena_mut().nodes.reserve(n);
    builder.arena_mut().children.reserve(n);
    builder.arena_mut().type_data.reserve(n * 20);
    let newline_ref = builder.alloc_string("\n");
    let refs = collect_refs(source);
    let ctx = ConvertCtx {
        defs: &refs.defs,
        footnotes: refs.footnotes.as_ref(),
        footnote_defs: &refs.footnote_defs,
        footnote_ref_occurrence: &refs.footnote_ref_occurrence,
        footnote_ref_totals: &refs.footnote_ref_totals,
        newline_ref,
    };
    convert_node(0, source, &mut builder, &ctx);
    builder.finish()
}

/// Shared read-only context threaded through the conversion.
///
/// `footnotes` is only materialized when the document actually contains a
/// FootnoteReference or FootnoteDefinition, so documents without footnotes
/// don't allocate a HashMap at all.
struct ConvertCtx<'a, 'src> {
    defs: &'a FxHashMap<&'src str, Definition>,
    footnotes: Option<&'a FxHashMap<&'src str, usize>>,
    /// Pre-allocated `"\n"` StringRef shared by every block separator inserted
    /// during conversion; avoids re-pushing a single byte into the source pool.
    newline_ref: StringRef,
    /// Node IDs of FootnoteDefinition nodes in the order their identifiers
    /// are first referenced. This matches remark-gfm's `state.footnoteOrder`
    /// and drives the order of `<li>`s in the emitted `<section>`.
    footnote_defs: &'a [u32],
    /// For each FootnoteReference node id: the 1-based occurrence index of
    /// that reference within its identifier. First ref = 1, second = 2, etc.
    /// Used to emit `id="user-content-fnref-ID[-K]"` on the anchor.
    footnote_ref_occurrence: &'a FxHashMap<u32, usize>,
    /// Per-identifier total number of references (for the backref links in
    /// the section).
    footnote_ref_totals: &'a FxHashMap<&'src str, usize>,
}

/// Definition data stored as StringRefs into the MDAST source, avoids cloning strings.
struct Definition {
    url: StringRef,
    title: StringRef, // empty = no title
}

/// Single-pass collection of everything later arms need to cross-reference:
/// link/image reference definitions, plus source-order numbering for
/// footnote references and definitions.
struct CollectedRefs<'src> {
    defs: FxHashMap<&'src str, Definition>,
    /// `None` when the document contains no footnotes — saves the HashMap
    /// allocation on the common path.
    footnotes: Option<FxHashMap<&'src str, usize>>,
    /// FootnoteDefinition node ids in the order their identifiers are first
    /// referenced (main flow first, then inside definitions that got queued).
    footnote_defs: Vec<u32>,
    /// 1-based occurrence index for each FootnoteReference node id.
    footnote_ref_occurrence: FxHashMap<u32, usize>,
    /// Total reference count per identifier.
    footnote_ref_totals: FxHashMap<&'src str, usize>,
}

fn collect_refs(view: &Arena<Mdast>) -> CollectedRefs<'_> {
    let mut defs: FxHashMap<&str, Definition> = FxHashMap::default();
    let mut fn_def_nodes: FxHashMap<&str, u32> = FxHashMap::default();

    // First-wins for duplicate identifiers means *source order*, not
    // node-id order: top-level refdefs are appended at the end of the
    // root after blockquote-nested defs have already been allocated, so
    // their IDs come later even though they appear earlier in the
    // document. Collect Definition node ids first, then sort by source
    // position before inserting.
    let mut def_nodes: Vec<u32> = Vec::new();
    for id in 0..view.len() as u32 {
        let node = view.get_node(id);
        let data = view.get_type_data(id);
        match MdastNodeType::from_u8(node.node_type) {
            Some(MdastNodeType::Definition) if data.len() >= 32 => {
                def_nodes.push(id);
            }
            Some(MdastNodeType::FootnoteDefinition) if data.len() >= 16 => {
                let fd = decode_footnote_definition_data(data);
                let identifier = view.get_str(fd.identifier);
                fn_def_nodes.entry(identifier).or_insert(id);
            }
            _ => {}
        }
    }
    def_nodes.sort_by_key(|&id| view.get_node(id).start_offset);
    for id in &def_nodes {
        let data = view.get_type_data(*id);
        let dd = decode_definition_data(data);
        let identifier = view.get_str(dd.identifier);
        defs.entry(identifier).or_insert_with(|| Definition {
            url: dd.url,
            title: dd.title,
        });
    }

    // No footnote definitions ⇒ no references can resolve, so the rest of
    // this function's footnote bookkeeping is guaranteed to produce empty
    // results. Skip the two arena walks and the HashMap allocations.
    if fn_def_nodes.is_empty() {
        return CollectedRefs {
            defs,
            footnotes: None,
            footnote_defs: Vec::new(),
            footnote_ref_occurrence: FxHashMap::default(),
            footnote_ref_totals: FxHashMap::default(),
        };
    }

    // Pass 2: mirror remark-gfm's rendering-time footnoteOrder. Main-flow
    // refs come first (skip into definition bodies), then each referenced
    // definition's body is scanned in the order it was first referenced —
    // which can itself add more entries as nested refs are discovered.
    //
    // Queueing is done in terms of node ids so nothing needs to outlive
    // `view`. Identifier lookups use the shared `fn_def_nodes` map.
    let mut fn_numbers: FxHashMap<&str, usize> = FxHashMap::default();
    let mut fn_def_order: Vec<u32> = Vec::new();

    // Collect refs encountered in the main document (everything except
    // footnote definition bodies), in DFS order. Store as node ids.
    //
    // Skip directive subtrees too: our HAST conversion drops directives, so
    // any footnote reference inside a directive never appears in the output.
    // Counting it here would incorrectly force the footnote `<section>` to
    // appear even when no live reference remains.
    fn walk_main_refs(view: &Arena<Mdast>, node_id: u32, refs: &mut Vec<u32>) {
        let node = view.get_node(node_id);
        let ty = MdastNodeType::from_u8(node.node_type);
        if ty == Some(MdastNodeType::FootnoteDefinition) {
            return;
        }
        if matches!(
            ty,
            Some(
                MdastNodeType::ContainerDirective
                    | MdastNodeType::LeafDirective
                    | MdastNodeType::TextDirective
            )
        ) {
            return;
        }
        if ty == Some(MdastNodeType::FootnoteReference) {
            refs.push(node_id);
        }
        for &child_id in view.get_children(node_id) {
            walk_main_refs(view, child_id, refs);
        }
    }
    let mut main_refs: Vec<u32> = Vec::new();
    walk_main_refs(view, 0, &mut main_refs);

    for ref_id in main_refs {
        let data = view.get_type_data(ref_id);
        if data.len() < 20 {
            continue;
        }
        let rd = decode_reference_data(data);
        let identifier = view.get_str(rd.identifier);
        if fn_numbers.contains_key(identifier) {
            continue;
        }
        let Some(&def_id) = fn_def_nodes.get(identifier) else {
            continue;
        };
        let def_data = view.get_type_data(def_id);
        let fd = decode_footnote_definition_data(def_data);
        let id_view: &str = view.get_str(fd.identifier);
        fn_numbers.insert(id_view, fn_numbers.len() + 1);
        fn_def_order.push(def_id);
    }

    // Walk each queued def body to pick up nested refs. Because defs can
    // reference each other, the queue may grow while we iterate — index into
    // it by position rather than borrowing an iterator.
    fn walk_body_refs(view: &Arena<Mdast>, node_id: u32, refs: &mut Vec<u32>) {
        let node = view.get_node(node_id);
        if MdastNodeType::from_u8(node.node_type) == Some(MdastNodeType::FootnoteReference) {
            refs.push(node_id);
        }
        for &child_id in view.get_children(node_id) {
            walk_body_refs(view, child_id, refs);
        }
    }
    let mut cursor = 0;
    while cursor < fn_def_order.len() {
        let def_id = fn_def_order[cursor];
        let mut body_refs: Vec<u32> = Vec::new();
        walk_body_refs(view, def_id, &mut body_refs);
        for ref_id in body_refs {
            let data = view.get_type_data(ref_id);
            if data.len() < 20 {
                continue;
            }
            let rd = decode_reference_data(data);
            let identifier = view.get_str(rd.identifier);
            if fn_numbers.contains_key(identifier) {
                continue;
            }
            let Some(&d_id) = fn_def_nodes.get(identifier) else {
                continue;
            };
            let dd = view.get_type_data(d_id);
            let fd = decode_footnote_definition_data(dd);
            let id_view: &str = view.get_str(fd.identifier);
            fn_numbers.insert(id_view, fn_numbers.len() + 1);
            fn_def_order.push(d_id);
        }
        cursor += 1;
    }

    // Pass 3: assign 1-based occurrence indices to every reference that
    // resolves to a numbered definition, matching remark's rendering order
    // (main flow first, then each queued def body in `fn_def_order` order).
    let mut fn_ref_occurrence: FxHashMap<u32, usize> = FxHashMap::default();
    let mut fn_ref_totals: FxHashMap<&str, usize> = FxHashMap::default();
    let mut main_refs2: Vec<u32> = Vec::new();
    walk_main_refs(view, 0, &mut main_refs2);
    for ref_id in main_refs2 {
        let data = view.get_type_data(ref_id);
        if data.len() < 20 {
            continue;
        }
        let rd = decode_reference_data(data);
        let identifier = view.get_str(rd.identifier);
        if !fn_numbers.contains_key(identifier) {
            continue;
        }
        // Resolve identifier to the definition's view-owned string so it
        // outlives the caller chain.
        let &def_id = fn_def_nodes.get(identifier).unwrap();
        let fd = decode_footnote_definition_data(view.get_type_data(def_id));
        let id_view: &str = view.get_str(fd.identifier);
        let entry = fn_ref_totals.entry(id_view).or_insert(0);
        *entry += 1;
        fn_ref_occurrence.insert(ref_id, *entry);
    }
    for &def_id in &fn_def_order {
        let mut body_refs: Vec<u32> = Vec::new();
        walk_body_refs(view, def_id, &mut body_refs);
        for ref_id in body_refs {
            let data = view.get_type_data(ref_id);
            if data.len() < 20 {
                continue;
            }
            let rd = decode_reference_data(data);
            let identifier = view.get_str(rd.identifier);
            if !fn_numbers.contains_key(identifier) {
                continue;
            }
            let &d_id = fn_def_nodes.get(identifier).unwrap();
            let fd = decode_footnote_definition_data(view.get_type_data(d_id));
            let id_view: &str = view.get_str(fd.identifier);
            let entry = fn_ref_totals.entry(id_view).or_insert(0);
            *entry += 1;
            fn_ref_occurrence.insert(ref_id, *entry);
        }
    }

    let footnotes = if fn_numbers.is_empty() {
        None
    } else {
        Some(fn_numbers)
    };

    CollectedRefs {
        defs,
        footnotes,
        footnote_defs: fn_def_order,
        footnote_ref_occurrence: fn_ref_occurrence,
        footnote_ref_totals: fn_ref_totals,
    }
}

fn find_def<'a>(
    defs: &'a FxHashMap<&str, Definition>,
    _view: &Arena<Mdast>,
    identifier: &str,
) -> Option<&'a Definition> {
    defs.get(identifier)
}

/// Pre-built property data: refs already interned in the builder's string pool.
struct PropData {
    name_ref: StringRef,
    value_kind: u8,
    value_ref: StringRef,
}

fn build_props(builder: &mut ArenaBuilder<Hast>, specs: &[(&str, u8, StringRef)]) -> Vec<PropData> {
    specs
        .iter()
        .map(|&(name, kind, value_ref)| {
            let name_ref = builder.alloc_string(name);
            PropData {
                name_ref,
                value_kind: kind,
                value_ref,
            }
        })
        .collect()
}

/// Open an element and emit its props without an intermediate `Vec<PropData>`.
fn open_element_with_specs(
    builder: &mut ArenaBuilder<Hast>,
    tag: &str,
    specs: &[(&str, u8, StringRef)],
) -> u32 {
    let tag_ref = builder.alloc_string(tag);
    let id = builder.open_node_raw(HastNodeType::Element as u8);
    write_element_data_specs(builder, tag_ref, specs);
    id
}

fn add_void_element_with_specs(
    builder: &mut ArenaBuilder<Hast>,
    tag: &str,
    specs: &[(&str, u8, StringRef)],
) -> u32 {
    let id = open_element_with_specs(builder, tag, specs);
    builder.close_node();
    id
}

fn write_element_data_specs(
    builder: &mut ArenaBuilder<Hast>,
    tag_ref: StringRef,
    specs: &[(&str, u8, StringRef)],
) {
    let name_refs: [StringRef; 8] = {
        let mut arr = [StringRef::empty(); 8];
        debug_assert!(specs.len() <= arr.len(), "too many props for inline buffer");
        for (i, &(name, _, _)) in specs.iter().enumerate() {
            arr[i] = builder.alloc_string(name);
        }
        arr
    };
    let writer = builder.begin_data_current();
    let out = &mut builder.arena_mut().type_data;
    out.extend_from_slice(&tag_ref.as_bytes());
    out.extend_from_slice(&(specs.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for (i, &(_, kind, value_ref)) in specs.iter().enumerate() {
        out.extend_from_slice(&name_refs[i].as_bytes());
        out.push(kind);
        out.extend_from_slice(&[0u8; 3]);
        out.extend_from_slice(&value_ref.as_bytes());
    }
    builder.finish_data_current(writer);
}

fn list_contains_task_item(list_id: u32, view: &Arena<Mdast>) -> bool {
    for &child_id in view.get_children(list_id) {
        let child = view.get_node(child_id);
        if MdastNodeType::from_u8(child.node_type) != Some(MdastNodeType::ListItem) {
            continue;
        }
        let data = view.get_type_data(child_id);
        if data.is_empty() {
            continue;
        }
        if decode_list_item_data(data).checked != 2 {
            return true;
        }
    }
    false
}

fn write_element_data(builder: &mut ArenaBuilder<Hast>, tag_ref: StringRef, props: &[PropData]) {
    let writer = builder.begin_data_current();
    let out = &mut builder.arena_mut().type_data;
    out.extend_from_slice(&tag_ref.as_bytes());
    out.extend_from_slice(&(props.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for p in props {
        out.extend_from_slice(&p.name_ref.as_bytes());
        out.push(p.value_kind);
        out.extend_from_slice(&[0u8; 3]);
        out.extend_from_slice(&p.value_ref.as_bytes());
    }
    builder.finish_data_current(writer);
}

fn open_element(builder: &mut ArenaBuilder<Hast>, tag: &str) -> u32 {
    let id = builder.open_node_raw(HastNodeType::Element as u8);
    let tag_ref = builder.alloc_string(tag);
    let writer = builder.begin_data_current();
    encode_element_data_into(tag_ref, &[], &mut builder.arena_mut().type_data);
    builder.finish_data_current(writer);
    id
}

fn open_element_with_props(builder: &mut ArenaBuilder<Hast>, tag: &str, props: &[PropData]) -> u32 {
    let id = builder.open_node_raw(HastNodeType::Element as u8);
    let tag_ref = builder.alloc_string(tag);
    write_element_data(builder, tag_ref, props);
    id
}

fn add_void_element(builder: &mut ArenaBuilder<Hast>, tag: &str) -> u32 {
    let id = builder.open_node_raw(HastNodeType::Element as u8);
    let tag_ref = builder.alloc_string(tag);
    let writer = builder.begin_data_current();
    encode_element_data_into(tag_ref, &[], &mut builder.arena_mut().type_data);
    builder.finish_data_current(writer);
    builder.close_node();
    id
}

fn add_void_element_with_props(
    builder: &mut ArenaBuilder<Hast>,
    tag: &str,
    props: &[PropData],
) -> u32 {
    let id = builder.open_node_raw(HastNodeType::Element as u8);
    let tag_ref = builder.alloc_string(tag);
    write_element_data(builder, tag_ref, props);
    builder.close_node();
    id
}

fn add_text_node(builder: &mut ArenaBuilder<Hast>, text: &str) -> u32 {
    let text_ref = builder.alloc_string(text);
    add_text_node_with_ref(builder, text_ref)
}

/// Mirror `trim-lines`: strip spaces/tabs adjacent to line breaks inside the
/// value. The very first character and the very last character are preserved
/// (only line ENDS for non-final lines and line STARTS for non-first lines
/// get trimmed). Returns `Cow::Borrowed` when the value is unchanged so the
/// caller can reuse the original `StringRef`.
fn trim_lines_for_hast(value: &str) -> std::borrow::Cow<'_, str> {
    let bytes = value.as_bytes();
    // Quick scan: any line break with adjacent ws? If not, nothing to trim.
    let mut needs_trim = false;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' || b == b'\r' {
            if i > 0 && (bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
                needs_trim = true;
                break;
            }
            let after = if b == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                i + 2
            } else {
                i + 1
            };
            if after < bytes.len() && (bytes[after] == b' ' || bytes[after] == b'\t') {
                needs_trim = true;
                break;
            }
            i = after;
            continue;
        }
        i += 1;
    }
    if !needs_trim {
        return std::borrow::Cow::Borrowed(value);
    }
    let mut out = String::with_capacity(value.len());
    let mut last = 0;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' || b == b'\r' {
            // Trim trailing ws on the line ending here (interior line end).
            let mut line_end = i;
            while line_end > last && (bytes[line_end - 1] == b' ' || bytes[line_end - 1] == b'\t') {
                line_end -= 1;
            }
            out.push_str(&value[last..line_end]);
            // Append the line break itself.
            let lb_end = if b == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                i + 2
            } else {
                i + 1
            };
            out.push_str(&value[i..lb_end]);
            // Skip leading ws on the next line (interior line start).
            let mut next_start = lb_end;
            while next_start < bytes.len()
                && (bytes[next_start] == b' ' || bytes[next_start] == b'\t')
            {
                next_start += 1;
            }
            last = next_start;
            i = next_start;
            continue;
        }
        i += 1;
    }
    out.push_str(&value[last..]);
    std::borrow::Cow::Owned(out)
}

/// Add a text leaf reusing a StringRef from the source arena that seeded the
/// builder; only valid because the builder's source pool starts as a clone of
/// the view's source, so source-derived offsets address the same bytes.
fn add_text_node_with_ref(builder: &mut ArenaBuilder<Hast>, text_ref: StringRef) -> u32 {
    let leaf_id = builder.add_leaf_raw(HastNodeType::Text as u8);
    builder
        .arena_mut()
        .set_type_data(leaf_id, &text_ref.as_bytes());
    leaf_id
}

fn add_raw_node(builder: &mut ArenaBuilder<Hast>, html: &str) -> u32 {
    let html_ref = builder.alloc_string(html);
    let leaf_id = builder.add_leaf_raw(HastNodeType::Raw as u8);
    builder
        .arena_mut()
        .set_type_data(leaf_id, &html_ref.as_bytes());
    leaf_id
}

/// Set position on a node by id, copying from the given source mdast node.
/// Used for leaf nodes (void elements, text, raw) which can't use `set_position_current`.
fn copy_position_to(
    target_id: u32,
    src_node_id: u32,
    view: &Arena<Mdast>,
    builder: &mut ArenaBuilder<Hast>,
) {
    let node = view.get_node(src_node_id);
    if node.start_line > 0 || node.start_offset > 0 {
        builder.arena_mut().set_position(
            target_id,
            node.start_offset,
            node.end_offset,
            node.start_line,
            node.start_column,
            node.end_line,
            node.end_column,
        );
    }
}

/// Encode lang and meta as a JSON object for the code element's node_data.
fn encode_code_node_data(lang: &str, meta: &str) -> Vec<u8> {
    // Manual JSON construction, avoids serde_json dep.
    // Both lang and meta come from markdown source, so we need to escape
    // backslashes, double quotes, and control characters.
    fn json_escape(s: &str, out: &mut Vec<u8>) {
        for ch in s.bytes() {
            match ch {
                b'"' => out.extend_from_slice(b"\\\""),
                b'\\' => out.extend_from_slice(b"\\\\"),
                b'\n' => out.extend_from_slice(b"\\n"),
                b'\r' => out.extend_from_slice(b"\\r"),
                b'\t' => out.extend_from_slice(b"\\t"),
                c if c < 0x20 => {
                    // Other control characters: \u00XX
                    out.extend_from_slice(b"\\u00");
                    out.push(b"0123456789abcdef"[(c >> 4) as usize]);
                    out.push(b"0123456789abcdef"[(c & 0xf) as usize]);
                }
                _ => out.push(ch),
            }
        }
    }

    // Emit only the keys that have content so plugin-set `data.meta = ""`
    // can round-trip independently of the converter's behaviour.
    let mut buf = Vec::with_capacity(32 + lang.len() + meta.len());
    buf.push(b'{');
    let mut first = true;
    if !lang.is_empty() {
        buf.extend_from_slice(b"\"lang\":\"");
        json_escape(lang, &mut buf);
        buf.push(b'"');
        first = false;
    }
    if !meta.is_empty() {
        if !first {
            buf.push(b',');
        }
        buf.extend_from_slice(b"\"meta\":\"");
        json_escape(meta, &mut buf);
        buf.push(b'"');
    }
    buf.push(b'}');
    buf
}

fn copy_position(node_id: u32, view: &Arena<Mdast>, builder: &mut ArenaBuilder<Hast>) {
    let node = view.get_node(node_id);
    if node.start_line > 0 || node.start_offset > 0 {
        builder.set_position_current(
            node.start_offset,
            node.end_offset,
            node.start_line,
            node.start_column,
            node.end_line,
            node.end_column,
        );
    }
}

fn convert_node(
    node_id: u32,
    view: &Arena<Mdast>,
    builder: &mut ArenaBuilder<Hast>,
    ctx: &ConvertCtx<'_, '_>,
) {
    let node = view.get_node(node_id);
    let raw_type = node.node_type;

    match MdastNodeType::from_u8(raw_type) {
        Some(MdastNodeType::Root) => {
            builder.open_node_raw(HastNodeType::Root as u8);
            copy_position(node_id, view, builder);
            convert_children_wrapped(node_id, view, builder, ctx);
            emit_gfm_footnotes_section(view, builder, ctx);
            builder.close_node();
        }

        Some(MdastNodeType::Paragraph) => {
            // Note: MDX paragraph unraveling is handled by convert_children_wrapped
            // at the parent level, so by the time we get here it's a normal <p>.
            let action = open_h_element(builder, view, node_id, "p", &[]);
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Recurse) {
                convert_children(node_id, view, builder, ctx);
            }
            builder.close_node();
        }

        Some(MdastNodeType::Heading) => {
            let data = view.get_type_data(node_id);
            let depth = if data.is_empty() {
                1
            } else {
                decode_heading_data(data).depth
            };
            let tag = match depth {
                1 => "h1",
                2 => "h2",
                3 => "h3",
                4 => "h4",
                5 => "h5",
                _ => "h6",
            };
            let action = open_h_element(builder, view, node_id, tag, &[]);
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Recurse) {
                convert_children(node_id, view, builder, ctx);
            }
            builder.close_node();
        }

        Some(MdastNodeType::ThematicBreak) => {
            let id = add_h_void_element(builder, view, node_id, "hr", &[]);
            copy_position_to(id, node_id, view, builder);
        }

        Some(MdastNodeType::Blockquote) => {
            let action = open_h_element(builder, view, node_id, "blockquote", &[]);
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Recurse) {
                convert_children_with_newlines(node_id, view, builder, ctx);
            }
            builder.close_node();
        }

        Some(MdastNodeType::List) => {
            let data = view.get_type_data(node_id);
            let list_data = decode_list_data(data);
            let tag = if list_data.ordered { "ol" } else { "ul" };
            let has_task_items = list_contains_task_item(node_id, view);

            let mut prop_specs: Vec<(&str, u8, StringRef)> = Vec::new();
            let start_str;
            let start_ref;
            let class_ref;
            if list_data.ordered && list_data.start != 1 {
                start_str = list_data.start.to_string();
                start_ref = builder.alloc_string(&start_str);
                prop_specs.push(("start", PROP_INT, start_ref));
            }
            if has_task_items {
                class_ref = builder.alloc_string("contains-task-list");
                prop_specs.push(("className", PROP_SPACE_SEP, class_ref));
            }

            let action = open_h_element(builder, view, node_id, tag, &prop_specs);
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Recurse) {
                convert_children_with_newlines(node_id, view, builder, ctx);
            }
            builder.close_node();
        }

        Some(MdastNodeType::ListItem) => {
            let data = view.get_type_data(node_id);
            let item_data = if data.is_empty() {
                None
            } else {
                Some(decode_list_item_data(data))
            };
            let is_task = item_data.is_some_and(|d| d.checked != 2);

            let task_class_ref;
            let task_specs: [(&str, u8, StringRef); 1];
            let li_specs: &[(&str, u8, StringRef)] = if is_task {
                task_class_ref = builder.alloc_string("task-list-item");
                task_specs = [("className", PROP_SPACE_SEP, task_class_ref)];
                &task_specs
            } else {
                &[]
            };
            // hChildren replaces the rendered children; skip task-checkbox /
            // paragraph-unwrap behavior in that case.
            let action = open_h_element(builder, view, node_id, "li", li_specs);
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Replaced) {
                builder.close_node();
                return;
            }

            let parent_id = view.get_node(node_id).parent;
            let loose = {
                let pd = view.get_type_data(parent_id);
                if !pd.is_empty() && decode_list_data(pd).spread {
                    true
                } else {
                    view.get_children(parent_id).iter().any(|&sibling_id| {
                        let sd = view.get_type_data(sibling_id);
                        !sd.is_empty() && decode_list_item_data(sd).spread
                    })
                }
            };
            if loose {
                convert_children_with_newlines_task(
                    node_id,
                    view,
                    builder,
                    ctx,
                    is_task.then(|| item_data.unwrap()),
                );
            } else {
                convert_children_unwrap_paragraphs_task(
                    node_id,
                    view,
                    builder,
                    ctx,
                    is_task.then(|| item_data.unwrap()),
                );
            }
            builder.close_node();
        }

        Some(MdastNodeType::Html) => {
            let data = view.get_type_data(node_id);
            let string_ref = decode_string_ref_data(data);
            let value = view.get_str(string_ref);
            let id = add_raw_node(builder, value);
            copy_position_to(id, node_id, view, builder);
        }

        Some(MdastNodeType::Code) => {
            let data = view.get_type_data(node_id);
            let code_data = decode_code_data(data);
            let value = view.get_str(code_data.value);

            // hName/hProperties on the code mdast node override the outer
            // <pre>; the inner <code> remains as remark-rehype emits it.
            // hChildren on a code node replaces the pre's children entirely
            // (no <code>, just whatever the user provided).
            let action = open_h_element(builder, view, node_id, "pre", &[]);
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Replaced) {
                builder.close_node();
                return;
            }
            let code_id = if code_data.lang.len > 0 {
                let lang = view.get_str(code_data.lang);
                let class_val = format!("language-{}", lang);
                let class_ref = builder.alloc_string(&class_val);
                let props = build_props(builder, &[("className", PROP_SPACE_SEP, class_ref)]);
                open_element_with_props(builder, "code", &props)
            } else {
                open_element(builder, "code")
            };

            copy_position(node_id, view, builder);

            let lang = view.get_str(code_data.lang);
            let meta = view.get_str(code_data.meta);
            if !lang.is_empty() || !meta.is_empty() {
                let json = encode_code_node_data(lang, meta);
                builder.arena_mut().set_node_data(code_id, json);
            }

            if value.is_empty() {
                add_text_node(builder, value);
            } else {
                let mut buf = String::with_capacity(value.len() + 1);
                buf.push_str(value);
                buf.push('\n');
                add_text_node(builder, &buf);
            }
            builder.close_node(); // code
            builder.close_node(); // pre
        }

        Some(MdastNodeType::Text) => {
            let data = view.get_type_data(node_id);
            let string_ref = decode_string_ref_data(data);
            // mdast-util-to-hast applies `trim-lines`: strip spaces/tabs at
            // line-break boundaries inside the text value (interior line
            // ends and starts), but preserve leading whitespace of the first
            // line and trailing whitespace of the last line. Handles soft-
            // wraps where a continuation line starts with `&#9;` (decoded
            // to a tab in mdast) — the tab vanishes in hast.
            let raw_value = view.get_str(string_ref);
            let id = match trim_lines_for_hast(raw_value) {
                std::borrow::Cow::Borrowed(_) => add_text_node_with_ref(builder, string_ref),
                std::borrow::Cow::Owned(s) => add_text_node(builder, &s),
            };
            copy_position_to(id, node_id, view, builder);
        }

        Some(MdastNodeType::Emphasis) => {
            let action = open_h_element(builder, view, node_id, "em", &[]);
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Recurse) {
                convert_children(node_id, view, builder, ctx);
            }
            builder.close_node();
        }

        Some(MdastNodeType::Strong) => {
            let action = open_h_element(builder, view, node_id, "strong", &[]);
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Recurse) {
                convert_children(node_id, view, builder, ctx);
            }
            builder.close_node();
        }

        Some(MdastNodeType::InlineCode) => {
            let data = view.get_type_data(node_id);
            let string_ref = decode_string_ref_data(data);
            let action = open_h_element(builder, view, node_id, "code", &[]);
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Recurse) {
                let value = view.get_str(string_ref);
                if value.contains('\n') {
                    let normalized = value.replace('\n', " ");
                    let text_id = add_text_node(builder, &normalized);
                    copy_position_to(text_id, node_id, view, builder);
                } else {
                    let text_id = add_text_node(builder, value);
                    copy_position_to(text_id, node_id, view, builder);
                }
            }
            builder.close_node();
        }

        Some(MdastNodeType::Break) => {
            let id = add_h_void_element(builder, view, node_id, "br", &[]);
            copy_position_to(id, node_id, view, builder);
            add_text_node_with_ref(builder, ctx.newline_ref);
        }

        Some(MdastNodeType::Link) => {
            let data = view.get_type_data(node_id);
            let link_data = decode_link_data(data);
            let url_ref = encode_url(builder, view.get_str(link_data.url));
            let specs: Vec<(&str, u8, StringRef)> = if link_data.title.len > 0 {
                vec![
                    ("href", PROP_STRING, url_ref),
                    ("title", PROP_STRING, link_data.title),
                ]
            } else {
                vec![("href", PROP_STRING, url_ref)]
            };
            let action = open_h_element(builder, view, node_id, "a", &specs);
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Recurse) {
                convert_children(node_id, view, builder, ctx);
            }
            builder.close_node();
        }

        Some(MdastNodeType::Image) => {
            let data = view.get_type_data(node_id);
            let img_data = decode_image_data(data);
            let url_ref = encode_url(builder, view.get_str(img_data.url));
            let specs: Vec<(&str, u8, StringRef)> = if img_data.title.len > 0 {
                vec![
                    ("src", PROP_STRING, url_ref),
                    ("alt", PROP_STRING, img_data.alt),
                    ("title", PROP_STRING, img_data.title),
                ]
            } else {
                vec![
                    ("src", PROP_STRING, url_ref),
                    ("alt", PROP_STRING, img_data.alt),
                ]
            };
            let id = add_h_void_element(builder, view, node_id, "img", &specs);
            copy_position_to(id, node_id, view, builder);
        }

        Some(MdastNodeType::Delete) => {
            let action = open_h_element(builder, view, node_id, "del", &[]);
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Recurse) {
                convert_children(node_id, view, builder, ctx);
            }
            builder.close_node();
        }

        Some(MdastNodeType::Table) => {
            let alignments = decode_table_alignments(view.get_type_data(node_id));
            let action = open_h_element(builder, view, node_id, "table", &[]);
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Replaced) {
                builder.close_node();
                return;
            }
            let child_ids = view.get_children(node_id);
            if !child_ids.is_empty() {
                add_text_node_with_ref(builder, ctx.newline_ref);
                open_element(builder, "thead");
                copy_position(child_ids[0], view, builder);
                add_text_node_with_ref(builder, ctx.newline_ref);
                convert_table_row(child_ids[0], view, builder, ctx, true, &alignments);
                add_text_node_with_ref(builder, ctx.newline_ref);
                builder.close_node(); // thead
                add_text_node_with_ref(builder, ctx.newline_ref);

                if child_ids.len() > 1 {
                    open_element(builder, "tbody");
                    // tbody spans from first body row to last body row
                    let first_body = child_ids[1];
                    let last_body = *child_ids.last().unwrap();
                    let fb = view.get_node(first_body);
                    let lb = view.get_node(last_body);
                    builder.set_position_current(
                        fb.start_offset,
                        lb.end_offset,
                        fb.start_line,
                        fb.start_column,
                        lb.end_line,
                        lb.end_column,
                    );
                    add_text_node_with_ref(builder, ctx.newline_ref);
                    for &row_id in &child_ids[1..] {
                        convert_table_row(row_id, view, builder, ctx, false, &alignments);
                        add_text_node_with_ref(builder, ctx.newline_ref);
                    }
                    builder.close_node(); // tbody
                    add_text_node_with_ref(builder, ctx.newline_ref);
                }
            }
            builder.close_node(); // table
        }

        Some(MdastNodeType::Math) => {
            let data = view.get_type_data(node_id);
            let math_data = decode_math_data(data);
            let value = view.get_str(math_data.value);
            // hName/hProperties on math affect the outer <pre>; the inner
            // <code> is left as-is. hChildren replaces all children.
            let action = open_h_element(builder, view, node_id, "pre", &[]);
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Replaced) {
                builder.close_node();
                return;
            }
            let class_ref = builder.alloc_string("language-math math-display");
            let props = build_props(builder, &[("className", PROP_SPACE_SEP, class_ref)]);
            open_element_with_props(builder, "code", &props);
            add_text_node(builder, value);
            builder.close_node(); // code
            builder.close_node(); // pre
        }

        Some(MdastNodeType::InlineMath) => {
            let data = view.get_type_data(node_id);
            let math_data = decode_math_data(data);
            let value = view.get_str(math_data.value);
            let class_ref = builder.alloc_string("language-math math-inline");
            let action = open_h_element(
                builder,
                view,
                node_id,
                "code",
                &[("className", PROP_SPACE_SEP, class_ref)],
            );
            copy_position(node_id, view, builder);
            if matches!(action, ChildrenAction::Recurse) {
                add_text_node(builder, value);
            }
            builder.close_node();
        }

        Some(MdastNodeType::Definition) | Some(MdastNodeType::Yaml) | Some(MdastNodeType::Toml) => {
            // No HAST output
        }

        Some(MdastNodeType::LinkReference) => {
            let data = view.get_type_data(node_id);
            if data.len() >= 20 {
                let rd = decode_reference_data(data);
                let identifier = view.get_str(rd.identifier);
                if let Some(def) = find_def(ctx.defs, view, identifier) {
                    let url_ref = encode_url(builder, view.get_str(def.url));
                    let specs: Vec<(&str, u8, StringRef)> = if !def.title.is_empty() {
                        vec![
                            ("href", PROP_STRING, url_ref),
                            ("title", PROP_STRING, def.title),
                        ]
                    } else {
                        vec![("href", PROP_STRING, url_ref)]
                    };
                    let action = open_h_element(builder, view, node_id, "a", &specs);
                    copy_position(node_id, view, builder);
                    if matches!(action, ChildrenAction::Recurse) {
                        convert_children(node_id, view, builder, ctx);
                    }
                    builder.close_node();
                } else {
                    // Unresolved: output children as-is
                    convert_children(node_id, view, builder, ctx);
                }
            }
        }

        Some(MdastNodeType::ImageReference) => {
            let data = view.get_type_data(node_id);
            if data.len() >= 20 {
                let rd = decode_reference_data(data);
                let identifier = view.get_str(rd.identifier);
                if let Some(def) = find_def(ctx.defs, view, identifier) {
                    // Parser-emitted ImageReferences store alt inline (the
                    // node has no children — alt is accumulated from the
                    // bracket text at parse time). Plugin-synthesized ones
                    // may lack this byte range and carry the text as
                    // children, so fall back to extracting from those.
                    let stored_alt = decode_image_reference_alt(data);
                    let alt = if !stored_alt.is_empty() {
                        view.get_str(stored_alt).to_string()
                    } else {
                        extract_text_content(node_id, view)
                    };
                    let url_ref = encode_url(builder, view.get_str(def.url));
                    let alt_ref = builder.alloc_string(&alt);
                    let specs: Vec<(&str, u8, StringRef)> = if !def.title.is_empty() {
                        vec![
                            ("src", PROP_STRING, url_ref),
                            ("alt", PROP_STRING, alt_ref),
                            ("title", PROP_STRING, def.title),
                        ]
                    } else {
                        vec![("src", PROP_STRING, url_ref), ("alt", PROP_STRING, alt_ref)]
                    };
                    let id = add_h_void_element(builder, view, node_id, "img", &specs);
                    copy_position_to(id, node_id, view, builder);
                }
            }
        }

        Some(MdastNodeType::FootnoteReference) => {
            let data = view.get_type_data(node_id);
            if data.len() >= 20 {
                let rd = decode_reference_data(data);
                let identifier = view.get_str(rd.identifier);
                let Some(number) = ctx.footnotes.and_then(|m| m.get(identifier).copied()) else {
                    // Orphan reference (no matching definition): remark keeps
                    // these as literal `[^id]` text.
                    let literal = format!("[^{}]", identifier);
                    add_text_node(builder, &literal);
                    return;
                };
                // remark lowercases the identifier when building URL/id
                // attributes so fragment targets collide-resist regardless of
                // how the author cased the source label.
                let safe_id = identifier.to_ascii_lowercase();
                let occurrence = ctx
                    .footnote_ref_occurrence
                    .get(&node_id)
                    .copied()
                    .unwrap_or(1);
                open_element(builder, "sup");
                copy_position(node_id, view, builder);
                let href = format!("#user-content-fn-{}", safe_id);
                // Reuses of the same footnote get a `-K` suffix on the id so
                // backrefs can target the specific call site.
                let id_attr = if occurrence > 1 {
                    format!("user-content-fnref-{}-{}", safe_id, occurrence)
                } else {
                    format!("user-content-fnref-{}", safe_id)
                };
                let href_ref = builder.alloc_string(&href);
                let id_ref = builder.alloc_string(&id_attr);
                let empty_ref = StringRef::empty();
                let aria_ref = builder.alloc_string("footnote-label");
                let a_props = build_props(
                    builder,
                    &[
                        ("href", PROP_STRING, href_ref),
                        ("id", PROP_STRING, id_ref),
                        ("dataFootnoteRef", PROP_BOOL_TRUE, empty_ref),
                        ("ariaDescribedBy", PROP_SPACE_SEP, aria_ref),
                    ],
                );
                open_element_with_props(builder, "a", &a_props);
                copy_position(node_id, view, builder);
                add_text_node(builder, &number.to_string());
                builder.close_node(); // a
                builder.close_node(); // sup
            }
        }

        Some(MdastNodeType::FootnoteDefinition) => {
            // Skipped inline — GFM renders all definitions together in the
            // trailing `<section class="footnotes">` block that's emitted by
            // `emit_gfm_footnotes_section` after the root's other children.
        }

        Some(MdastNodeType::MdxJsxFlowElement) => {
            convert_mdx_jsx_element(
                node_id,
                view,
                builder,
                ctx,
                HastNodeType::MdxJsxElement as u8,
            );
        }
        Some(MdastNodeType::MdxJsxTextElement) => {
            convert_mdx_jsx_element(
                node_id,
                view,
                builder,
                ctx,
                HastNodeType::MdxJsxTextElement as u8,
            );
        }

        Some(MdastNodeType::MdxFlowExpression) => {
            let data = view.get_type_data(node_id);
            let value = if data.is_empty() {
                ""
            } else {
                let d = decode_expression_data(data);
                view.get_str(d.value)
            };
            let value_ref = builder.alloc_string(value);
            let leaf_id = builder.add_leaf_raw(HastNodeType::MdxFlowExpression as u8);
            builder
                .arena_mut()
                .set_type_data(leaf_id, &value_ref.as_bytes());
            let mdast_node = view.get_node(node_id);
            builder.arena_mut().set_position(
                leaf_id,
                mdast_node.start_offset,
                mdast_node.end_offset,
                mdast_node.start_line,
                mdast_node.start_column,
                mdast_node.end_line,
                mdast_node.end_column,
            );
        }

        Some(MdastNodeType::MdxTextExpression) => {
            let data = view.get_type_data(node_id);
            let value = if data.is_empty() {
                ""
            } else {
                let d = decode_expression_data(data);
                view.get_str(d.value)
            };
            let value_ref = builder.alloc_string(value);
            let leaf_id = builder.add_leaf_raw(HastNodeType::MdxTextExpression as u8);
            builder
                .arena_mut()
                .set_type_data(leaf_id, &value_ref.as_bytes());
            let mdast_node = view.get_node(node_id);
            builder.arena_mut().set_position(
                leaf_id,
                mdast_node.start_offset,
                mdast_node.end_offset,
                mdast_node.start_line,
                mdast_node.start_column,
                mdast_node.end_line,
                mdast_node.end_column,
            );
        }

        Some(MdastNodeType::MdxjsEsm) => {
            let data = view.get_type_data(node_id);
            let value = if data.is_empty() {
                ""
            } else {
                let d = decode_expression_data(data);
                view.get_str(d.value)
            };
            let value_ref = builder.alloc_string(value);
            let leaf_id = builder.add_leaf_raw(HastNodeType::MdxEsm as u8);
            builder
                .arena_mut()
                .set_type_data(leaf_id, &value_ref.as_bytes());
            let mdast_node = view.get_node(node_id);
            builder.arena_mut().set_position(
                leaf_id,
                mdast_node.start_offset,
                mdast_node.end_offset,
                mdast_node.start_line,
                mdast_node.start_column,
                mdast_node.end_line,
                mdast_node.end_column,
            );
        }

        Some(MdastNodeType::ContainerDirective)
        | Some(MdastNodeType::LeafDirective)
        | Some(MdastNodeType::TextDirective) => {
            // Directives have no built-in HAST representation; the only way
            // to render one is to set `data.hName` (and optionally
            // `data.hProperties` / `data.hChildren`) on the mdast node from a
            // plugin. Without `hName`, drop the node — matching the empty
            // `containerDirective` handler we install on the reference side.
            let h = HData::read(view, node_id);
            if let Some(name) = h.h_name() {
                let props = merged_h_props(builder, &[], h.h_properties());
                open_element_with_props(builder, name, &props);
                copy_position(node_id, view, builder);
                if let Some(children) = h.h_children() {
                    emit_h_children(builder, children);
                } else {
                    convert_children(node_id, view, builder, ctx);
                }
                builder.close_node();
            }
        }

        _ => {
            // Unknown: recurse into children
            convert_children(node_id, view, builder, ctx);
        }
    }
}

fn convert_children(
    node_id: u32,
    view: &Arena<Mdast>,
    builder: &mut ArenaBuilder<Hast>,
    ctx: &ConvertCtx<'_, '_>,
) {
    let children = view.get_children(node_id);
    let mut prev_was_break = false;
    let break_ty = MdastNodeType::Break as u8;
    for &child_id in children {
        let before_count = builder.current_pending_children().len();
        convert_node(child_id, view, builder, ctx);
        if prev_was_break {
            let pending = builder.current_pending_children();
            if pending.len() > before_count {
                let first_new = pending[before_count];
                trim_leading_ws_after_break(builder, first_new);
            }
        }
        prev_was_break = view.get_node(child_id).node_type == break_ty;
    }
}

/// After a `Break` mdast sibling, trim leading spaces and tabs from the text
/// content of the next sibling's hast output. Matches mdast-util-to-hast's
/// post-break `trimMarkdownSpaceStart` pass: only the directly-emitted text
/// node (for text mdast nodes) or the first text child of the emitted element
/// is touched. No deeper recursion.
fn trim_leading_ws_after_break(builder: &mut ArenaBuilder<Hast>, node_id: u32) {
    let arena = builder.arena_mut();
    let target_id = {
        let node = arena.get_node(node_id);
        if node.node_type == HastNodeType::Text as u8 {
            Some(node_id)
        } else if node.node_type == HastNodeType::Element as u8 {
            let children = arena.get_children(node_id);
            children
                .first()
                .copied()
                .filter(|&id| arena.get_node(id).node_type == HastNodeType::Text as u8)
        } else {
            None
        }
    };
    let Some(text_id) = target_id else {
        return;
    };
    let (data_off, data_len) = {
        let node = arena.get_node(text_id);
        (node.data_offset as usize, node.data_len as usize)
    };
    if data_len < 8 {
        return;
    }
    let sref = StringRef::from_bytes(&arena.type_data[data_off..data_off + 8]);
    let s_off = sref.offset as usize;
    let s_len = sref.len as usize;
    let source_bytes = arena.source.as_bytes();
    if s_off + s_len > source_bytes.len() {
        return;
    }
    let slice = &source_bytes[s_off..s_off + s_len];
    let mut i = 0;
    while i < slice.len() && (slice[i] == b' ' || slice[i] == b'\t') {
        i += 1;
    }
    if i == 0 {
        return;
    }
    let new_ref = StringRef::new((s_off + i) as u32, (s_len - i) as u32);
    arena.type_data[data_off..data_off + 8].copy_from_slice(&new_ref.as_bytes());
}

/// Convert children with `\n` text nodes between them.
/// Matches `remark-rehype`'s behavior for block containers.
fn convert_children_with_newlines(
    node_id: u32,
    view: &Arena<Mdast>,
    builder: &mut ArenaBuilder<Hast>,
    ctx: &ConvertCtx<'_, '_>,
) {
    let children = view.get_children(node_id);
    add_text_node_with_ref(builder, ctx.newline_ref);
    if children.is_empty() {
        return;
    }
    for &child_id in children {
        if !produces_hast_output(child_id, view) {
            continue;
        }
        convert_node(child_id, view, builder, ctx);
        add_text_node_with_ref(builder, ctx.newline_ref);
    }
}

fn emit_checkbox(builder: &mut ArenaBuilder<Hast>, item_data: ListItemData) {
    let type_ref = builder.alloc_string("checkbox");
    if item_data.checked == 1 {
        let props = build_props(
            builder,
            &[
                ("type", PROP_STRING, type_ref),
                ("checked", PROP_BOOL_TRUE, StringRef::empty()),
                ("disabled", PROP_BOOL_TRUE, StringRef::empty()),
            ],
        );
        add_void_element_with_props(builder, "input", &props);
    } else {
        let props = build_props(
            builder,
            &[
                ("type", PROP_STRING, type_ref),
                ("checked", PROP_BOOL_FALSE, StringRef::empty()),
                ("disabled", PROP_BOOL_TRUE, StringRef::empty()),
            ],
        );
        add_void_element_with_props(builder, "input", &props);
    }
    add_text_node(builder, " ");
}

fn convert_children_with_newlines_task(
    node_id: u32,
    view: &Arena<Mdast>,
    builder: &mut ArenaBuilder<Hast>,
    ctx: &ConvertCtx<'_, '_>,
    task: Option<ListItemData>,
) {
    let children = view.get_children(node_id);
    if children.is_empty() {
        if let Some(td) = task {
            add_text_node_with_ref(builder, ctx.newline_ref);
            open_element(builder, "p");
            copy_position(node_id, view, builder);
            emit_checkbox(builder, td);
            builder.close_node();
            add_text_node_with_ref(builder, ctx.newline_ref);
        }
        return;
    }
    add_text_node_with_ref(builder, ctx.newline_ref);
    let mut first = true;
    for &child_id in children {
        let child_node = view.get_node(child_id);
        let is_para =
            MdastNodeType::from_u8(child_node.node_type) == Some(MdastNodeType::Paragraph);
        if let (true, true, Some(td)) = (first, is_para, task) {
            open_element(builder, "p");
            copy_position(child_id, view, builder);
            emit_checkbox(builder, td);
            convert_children(child_id, view, builder, ctx);
            builder.close_node();
        } else if let (true, Some(td)) = (first, task) {
            emit_checkbox(builder, td);
            convert_node(child_id, view, builder, ctx);
        } else {
            convert_node(child_id, view, builder, ctx);
        }
        // Skip the trailing separator for children that render to nothing
        // (e.g. directives without a handler) so we don't leave stray `\n`s.
        if produces_hast_output(child_id, view) {
            add_text_node_with_ref(builder, ctx.newline_ref);
        }
        first = false;
    }
}

fn convert_children_unwrap_paragraphs_task(
    node_id: u32,
    view: &Arena<Mdast>,
    builder: &mut ArenaBuilder<Hast>,
    ctx: &ConvertCtx<'_, '_>,
    task: Option<ListItemData>,
) {
    let children = view.get_children(node_id);
    let mut first = true;
    let mut prev_was_block = false;
    for &child_id in children {
        let child = view.get_node(child_id);
        // Children that render to nothing (e.g. directives) shouldn't
        // introduce extra `\n` separators.
        if !produces_hast_output(child_id, view) {
            continue;
        }
        if MdastNodeType::from_u8(child.node_type) == Some(MdastNodeType::Paragraph) {
            if let (true, Some(td)) = (first, task) {
                emit_checkbox(builder, td);
            }
            convert_children(child_id, view, builder, ctx);
            prev_was_block = false;
        } else {
            if !prev_was_block {
                add_text_node_with_ref(builder, ctx.newline_ref);
            }
            if let (true, Some(td)) = (first, task) {
                emit_checkbox(builder, td);
            }
            convert_node(child_id, view, builder, ctx);
            add_text_node_with_ref(builder, ctx.newline_ref);
            prev_was_block = true;
        }
        first = false;
    }
}

/// Convert children with `\n` text nodes inserted between them.
/// These are needed by the MDX compilation path (JSX children spacing).
/// The HTML renderer skips whitespace-only text nodes between block elements.
/// Emit the `<section class="footnotes">` block that remark-gfm appends to
/// the end of a document whenever it contains any footnote definitions.
fn emit_gfm_footnotes_section(
    view: &Arena<Mdast>,
    builder: &mut ArenaBuilder<Hast>,
    ctx: &ConvertCtx<'_, '_>,
) {
    if ctx.footnote_defs.is_empty() {
        return;
    }

    // "\n" separator between the document content and the section, matching
    // `convert_children_wrapped`'s output for adjacent siblings.
    add_text_node_with_ref(builder, ctx.newline_ref);

    let empty_ref = StringRef::empty();
    let footnotes_class = builder.alloc_string("footnotes");
    let section_props = build_props(
        builder,
        &[
            ("dataFootnotes", PROP_BOOL_TRUE, empty_ref),
            ("className", PROP_SPACE_SEP, footnotes_class),
        ],
    );
    open_element_with_props(builder, "section", &section_props);

    let sronly_class = builder.alloc_string("sr-only");
    let label_id_ref = builder.alloc_string("footnote-label");
    let h2_props = build_props(
        builder,
        &[
            ("className", PROP_SPACE_SEP, sronly_class),
            ("id", PROP_STRING, label_id_ref),
        ],
    );
    open_element_with_props(builder, "h2", &h2_props);
    add_text_node(builder, "Footnotes");
    builder.close_node(); // h2

    add_text_node_with_ref(builder, ctx.newline_ref);

    open_element(builder, "ol");
    add_text_node_with_ref(builder, ctx.newline_ref);

    for &def_id in ctx.footnote_defs {
        let def_data = view.get_type_data(def_id);
        if def_data.len() < 16 {
            continue;
        }
        let fd = decode_footnote_definition_data(def_data);
        let identifier = view.get_str(fd.identifier);
        let number = ctx
            .footnotes
            .and_then(|m| m.get(identifier).copied())
            .expect("footnote identifier missing from collected numbers");

        let safe_id = identifier.to_ascii_lowercase();
        let li_id = format!("user-content-fn-{}", safe_id);
        let li_id_ref = builder.alloc_string(&li_id);
        let li_props = build_props(builder, &[("id", PROP_STRING, li_id_ref)]);
        open_element_with_props(builder, "li", &li_props);
        // Carry position from the source FootnoteDefinition so the
        // generated `<li>` lines up with the original `[^id]: ...` span.
        copy_position(def_id, view, builder);
        add_text_node_with_ref(builder, ctx.newline_ref);

        let children: Vec<u32> = view.get_children(def_id).to_vec();
        let last_para_idx = children
            .iter()
            .enumerate()
            .rev()
            .find(|(_, &cid)| view.get_node(cid).node_type == MdastNodeType::Paragraph as u8)
            .map(|(i, _)| i);

        let total_refs = ctx
            .footnote_ref_totals
            .get(identifier)
            .copied()
            .unwrap_or(1)
            .max(1);

        if children.is_empty() {
            // Empty definition: emit the backref directly in the <li>.
            emit_footnote_backrefs(builder, &safe_id, number, total_refs);
        } else {
            for (i, &child_id) in children.iter().enumerate() {
                if i > 0 {
                    add_text_node_with_ref(builder, ctx.newline_ref);
                }
                if Some(i) == last_para_idx {
                    emit_paragraph_with_backrefs(
                        child_id, view, builder, ctx, &safe_id, number, total_refs,
                    );
                } else {
                    convert_node(child_id, view, builder, ctx);
                }
            }
            // Fallback: definition contained no paragraph at all. Append the
            // backref as a trailing sibling so it still reaches readers.
            if last_para_idx.is_none() {
                add_text_node_with_ref(builder, ctx.newline_ref);
                emit_footnote_backrefs(builder, &safe_id, number, total_refs);
            }
        }

        add_text_node_with_ref(builder, ctx.newline_ref);
        builder.close_node(); // li
        add_text_node_with_ref(builder, ctx.newline_ref);
    }

    builder.close_node(); // ol
    add_text_node_with_ref(builder, ctx.newline_ref);
    builder.close_node(); // section
}

/// Emit a `<p>` for `para_id`, converting its inline children and then
/// appending the GFM footnote backref(s). Matches remark-gfm's behaviour of
/// merging the separator space into the trailing text node when possible
/// (so the output has one text "foo " instead of two nodes "foo" + " ").
fn emit_paragraph_with_backrefs(
    para_id: u32,
    view: &Arena<Mdast>,
    builder: &mut ArenaBuilder<Hast>,
    ctx: &ConvertCtx<'_, '_>,
    identifier: &str,
    number: usize,
    total_refs: usize,
) {
    open_element(builder, "p");
    copy_position(para_id, view, builder);

    let inline_children = view.get_children(para_id);
    if let Some((&last_id, prefix)) = inline_children.split_last() {
        for &cid in prefix {
            convert_node(cid, view, builder, ctx);
        }
        let last_is_text = view.get_node(last_id).node_type == MdastNodeType::Text as u8;
        if last_is_text {
            let data = view.get_type_data(last_id);
            if data.len() >= 8 {
                let sr = StringRef::from_bytes(data);
                let value = view.get_str(sr);
                let with_space = format!("{} ", value);
                let leaf_id = add_text_node(builder, &with_space);
                // Position comes from the original text node (the synthesized
                // trailing space is not represented in source).
                copy_position_to(leaf_id, last_id, view, builder);
            } else {
                convert_node(last_id, view, builder, ctx);
                add_text_node(builder, " ");
            }
        } else {
            convert_node(last_id, view, builder, ctx);
            add_text_node(builder, " ");
        }
    }

    emit_footnote_backrefs(builder, identifier, number, total_refs);
    builder.close_node(); // p
}

/// Emit one or more backref `<a>` tags inside the footnotes section. With N
/// references to the same definition, remark emits N anchor tags separated
/// by single-space text nodes; the first uses the bare identifier, subsequent
/// ones use `-K` suffixes matching the `id`s stamped on the reference sups.
fn emit_footnote_backrefs(
    builder: &mut ArenaBuilder<Hast>,
    identifier: &str,
    number: usize,
    total_refs: usize,
) {
    for k in 1..=total_refs.max(1) {
        if k > 1 {
            add_text_node(builder, " ");
        }
        let href = if k > 1 {
            format!("#user-content-fnref-{}-{}", identifier, k)
        } else {
            format!("#user-content-fnref-{}", identifier)
        };
        let aria = if k > 1 {
            format!("Back to reference {}-{}", number, k)
        } else {
            format!("Back to reference {}", number)
        };
        let href_ref = builder.alloc_string(&href);
        let aria_ref = builder.alloc_string(&aria);
        let empty_ref = StringRef::empty();
        let backref_class = builder.alloc_string("data-footnote-backref");
        let props = build_props(
            builder,
            &[
                ("href", PROP_STRING, href_ref),
                ("dataFootnoteBackref", PROP_STRING, empty_ref),
                ("ariaLabel", PROP_STRING, aria_ref),
                ("className", PROP_SPACE_SEP, backref_class),
            ],
        );
        open_element_with_props(builder, "a", &props);
        add_text_node(builder, "\u{21a9}");
        if k > 1 {
            // Reuse markers: `<sup>K</sup>` follows the arrow for every
            // reference past the first so readers can tell which call site
            // the link targets.
            open_element(builder, "sup");
            add_text_node(builder, &k.to_string());
            builder.close_node();
        }
        builder.close_node(); // a
    }
}

fn produces_hast_output(child_id: u32, view: &Arena<Mdast>) -> bool {
    let raw_type = view.get_node(child_id).node_type;
    match MdastNodeType::from_u8(raw_type) {
        Some(
            MdastNodeType::Definition
            | MdastNodeType::Yaml
            | MdastNodeType::Toml
            // FootnoteDefinition is emitted only at document end as part
            // of the GFM `<section class="footnotes">` block.
            | MdastNodeType::FootnoteDefinition,
        ) => false,
        // Directives produce no output unless a plugin gave them an `hName`
        // (the only way to opt into a HAST representation).
        Some(
            MdastNodeType::ContainerDirective
            | MdastNodeType::LeafDirective
            | MdastNodeType::TextDirective,
        ) => HData::read(view, child_id).h_name().is_some(),
        _ => true,
    }
}

fn convert_children_wrapped(
    node_id: u32,
    view: &Arena<Mdast>,
    builder: &mut ArenaBuilder<Hast>,
    ctx: &ConvertCtx<'_, '_>,
) {
    let children = view.get_children(node_id);
    let mut has_output = false;
    for &child_id in children {
        if produces_hast_output(child_id, view) {
            if has_output {
                add_text_node_with_ref(builder, ctx.newline_ref);
            }
            has_output = true;
        }
        convert_node(child_id, view, builder, ctx);
    }
}

fn convert_table_row(
    row_id: u32,
    view: &Arena<Mdast>,
    builder: &mut ArenaBuilder<Hast>,
    ctx: &ConvertCtx<'_, '_>,
    is_header: bool,
    alignments: &[ColumnAlign],
) {
    open_element(builder, "tr");
    copy_position(row_id, view, builder);
    add_text_node_with_ref(builder, ctx.newline_ref);
    let all_cells = view.get_children(row_id);
    // mdast-util-to-hast truncates source cells past the header column count
    // (HAST padding fills underflow; this drops overflow). The MDAST tree
    // keeps all source cells per `mdast-util-gfm-table`.
    let max_cells = all_cells.len().min(alignments.len());
    let cell_ids = &all_cells[..max_cells];
    let cell_tag = if is_header { "th" } else { "td" };
    for (col_idx, &cell_id) in cell_ids.iter().enumerate() {
        let align = alignments
            .get(col_idx)
            .copied()
            .unwrap_or(ColumnAlign::None);
        let style = match align {
            ColumnAlign::None => None,
            ColumnAlign::Left => Some("text-align: left"),
            ColumnAlign::Right => Some("text-align: right"),
            ColumnAlign::Center => Some("text-align: center"),
        };
        if let Some(style) = style {
            let style_ref = builder.alloc_string(style);
            let props = build_props(builder, &[("style", PROP_STRING, style_ref)]);
            open_element_with_props(builder, cell_tag, &props);
        } else {
            open_element(builder, cell_tag);
        }
        copy_position(cell_id, view, builder);
        convert_children(cell_id, view, builder, ctx);
        builder.close_node();
        add_text_node_with_ref(builder, ctx.newline_ref);
    }
    // Pad to the header width — mdast-util-to-hast emits empty `<th>`/`<td>`
    // for any missing cells so the rendered table is rectangular, even though
    // the MDAST table row only stores the source cell count.
    for col_idx in cell_ids.len()..alignments.len() {
        let align = alignments
            .get(col_idx)
            .copied()
            .unwrap_or(ColumnAlign::None);
        let style = match align {
            ColumnAlign::None => None,
            ColumnAlign::Left => Some("text-align: left"),
            ColumnAlign::Right => Some("text-align: right"),
            ColumnAlign::Center => Some("text-align: center"),
        };
        if let Some(style) = style {
            let style_ref = builder.alloc_string(style);
            let props = build_props(builder, &[("style", PROP_STRING, style_ref)]);
            open_element_with_props(builder, cell_tag, &props);
        } else {
            open_element(builder, cell_tag);
        }
        builder.close_node();
        add_text_node_with_ref(builder, ctx.newline_ref);
    }
    builder.close_node(); // tr
}

fn convert_mdx_jsx_element(
    node_id: u32,
    view: &Arena<Mdast>,
    builder: &mut ArenaBuilder<Hast>,
    ctx: &ConvertCtx<'_, '_>,
    hast_type: u8,
) {
    let mdast_data = view.get_type_data(node_id);

    let name_ref_mdast = if mdast_data.len() >= 8 {
        decode_mdx_jsx_element_name(mdast_data)
    } else {
        StringRef::empty()
    };
    let name_str = if name_ref_mdast.len > 0 {
        view.get_str(name_ref_mdast)
    } else {
        ""
    };
    let name_ref = builder.alloc_string(name_str);

    // MDAST and HAST share the same attribute binary layout
    let attr_count = if mdast_data.len() >= 12 {
        decode_mdx_jsx_attr_count(mdast_data)
    } else {
        0
    };
    let explicit_jsx = decode_mdx_jsx_explicit(mdast_data);
    let mut attr_tuples = Vec::with_capacity(attr_count as usize);
    for i in 0..attr_count {
        let (kind, attr_name_ref, attr_value_ref) = decode_mdx_jsx_attr(mdast_data, i);
        attr_tuples.push((kind, attr_name_ref, attr_value_ref));
    }

    builder.open_node_raw(hast_type);
    let encoded = encode_mdx_jsx_element_data(name_ref, &attr_tuples, explicit_jsx);
    builder.set_data_current(&encoded);
    // Propagate `node_data` (e.g. `_mdxExplicitJsx` for source-parsed nodes,
    // or any other plugin-attached metadata) from mdast to hast.
    if let Some(mdast_nd) = view.get_node_data(node_id) {
        if !mdast_nd.is_empty() {
            let id = builder.current_node_id();
            let copy = mdast_nd.to_vec();
            builder.arena_mut().set_node_data(id, copy);
        }
    }
    copy_position(node_id, view, builder);

    convert_children(node_id, view, builder, ctx);
    builder.close_node();
}

fn extract_text_content(node_id: u32, view: &Arena<Mdast>) -> String {
    let mut out = String::new();
    extract_text_recursive(node_id, view, &mut out);
    out
}

fn extract_text_recursive(node_id: u32, view: &Arena<Mdast>, out: &mut String) {
    let node = view.get_node(node_id);
    if node.node_type == MdastNodeType::Text as u8 {
        let data = view.get_type_data(node_id);
        if !data.is_empty() {
            let sr = decode_string_ref_data(data);
            out.push_str(view.get_str(sr));
        }
    }
    for &child_id in view.get_children(node_id) {
        extract_text_recursive(child_id, view, out);
    }
}

#[cfg(test)]
mod hast_convert_tests {
    use super::*;

    #[test]
    fn multi_jsx_unraveled() {
        let source = "<Foo bar={1}/><Bar baz={2}/>\n";
        let opts = satteri_pulldown_cmark::Options::ENABLE_MDX;
        let (mdast, _) = satteri_pulldown_cmark::parse(source, opts);
        let hast = mdast_arena_to_hast_arena(&mdast);
        let root_children = hast.get_children(0);
        assert!(
            root_children.len() >= 2,
            "Expected at least 2 HAST root children"
        );
    }

    #[test]
    fn jsx_flow_with_full_options() {
        use satteri_pulldown_cmark::Options;
        let cases: &[(&str, &[u8])] = &[
            ("<a></a>\n", &[100]), // mdxJsxFlowElement
            ("<Foo/><Bar/>\n", &[100, 100]),
            ("<Box>{1}</Box>\n", &[100]),
            ("<Box><Foo/></Box>\n", &[100]),
            ("<Box>hello</Box>\n", &[100]), // unraveled to flow
        ];
        // Match the NAPI binding's default options for MDX
        let opts = satteri_pulldown_cmark::MDX_OPTIONS
            | Options::ENABLE_GFM
            | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS;
        for (source, expected_types) in cases {
            let (arena, _) = satteri_pulldown_cmark::parse(source, opts);
            let root_children = arena.get_children(0);
            let types: Vec<u8> = root_children
                .iter()
                .map(|&id| arena.get_node(id).node_type)
                .collect();
            assert_eq!(
                &types, expected_types,
                "source: {:?}, got types: {:?}",
                source, types
            );
        }
    }

    /// Set `data` JSON on a node by id; mirrors what the JS setProperty path
    /// does when a plugin writes `node.data`.
    fn set_data(arena: &mut Arena<Mdast>, node_id: u32, json: &str) {
        arena.set_node_data(node_id, json.as_bytes().to_vec());
    }

    use crate::hast::{hast_arena_to_html, HastNodeType};

    fn parse_md(source: &str) -> Arena<Mdast> {
        let (arena, _) =
            satteri_pulldown_cmark::parse(source, satteri_pulldown_cmark::Options::ENABLE_GFM);
        arena
    }

    fn find_first(arena: &Arena<Mdast>, node_type: MdastNodeType) -> u32 {
        for id in 0..arena.len() as u32 {
            if arena.get_node(id).node_type == node_type as u8 {
                return id;
            }
        }
        panic!("missing {node_type:?}");
    }

    fn first_element_tag(hast: &Arena<Hast>) -> String {
        for id in 0..hast.len() as u32 {
            if hast.get_node(id).node_type == HastNodeType::Element as u8 {
                let data = hast.get_type_data(id);
                let tag = StringRef::from_bytes(&data[0..8]);
                return hast.get_str(tag).to_string();
            }
        }
        panic!("no element in hast")
    }

    #[test]
    fn h_name_overrides_paragraph_tag() {
        let mut mdast = parse_md("Hello world\n");
        let para_id = find_first(&mdast, MdastNodeType::Paragraph);
        set_data(&mut mdast, para_id, r#"{"hName":"section"}"#);
        let hast = mdast_arena_to_hast_arena(&mdast);
        assert_eq!(first_element_tag(&hast), "section");
        let html = hast_arena_to_html(&hast);
        assert!(html.contains("<section>Hello world</section>"));
    }

    #[test]
    fn h_properties_merge_onto_paragraph() {
        let mut mdast = parse_md("Hi\n");
        let para_id = find_first(&mdast, MdastNodeType::Paragraph);
        set_data(
            &mut mdast,
            para_id,
            r#"{"hProperties":{"className":["note"],"id":"intro"}}"#,
        );
        let html = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        assert!(html.contains("class=\"note\""), "got {html}");
        assert!(html.contains("id=\"intro\""), "got {html}");
        assert!(html.contains("<p"), "tag stays <p>: {html}");
    }

    #[test]
    fn h_properties_null_strips() {
        let mut mdast = parse_md("- one\n- two\n");
        let list_id = find_first(&mdast, MdastNodeType::List);
        // Force className to null on a list with no task items so we can
        // verify a null would clear it. Use an explicit className first then
        // clear it via a second null entry that overrides.
        set_data(
            &mut mdast,
            list_id,
            r#"{"hProperties":{"className":["x","y"]}}"#,
        );
        let html_with = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        assert!(html_with.contains("class=\"x y\""));

        set_data(&mut mdast, list_id, r#"{"hProperties":{"className":null}}"#);
        let html_without = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        assert!(!html_without.contains("class="), "got {html_without}");
    }

    #[test]
    fn h_children_replaces_children() {
        let mut mdast = parse_md("Hello\n");
        let para_id = find_first(&mdast, MdastNodeType::Paragraph);
        set_data(
            &mut mdast,
            para_id,
            r#"{"hChildren":[{"type":"text","value":"replaced"}]}"#,
        );
        let html = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        assert!(html.contains("<p>replaced</p>"), "got {html}");
        assert!(!html.contains("Hello"), "original child kept: {html}");
    }

    #[test]
    fn h_name_with_h_children_emits_custom_tree() {
        let mut mdast = parse_md("Hello\n");
        let para_id = find_first(&mdast, MdastNodeType::Paragraph);
        set_data(
            &mut mdast,
            para_id,
            r#"{"hName":"aside","hProperties":{"className":["note"]},"hChildren":[{"type":"element","tagName":"strong","properties":{},"children":[{"type":"text","value":"Hi"}]}]}"#,
        );
        let html = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        assert!(
            html.contains("<aside class=\"note\"><strong>Hi</strong></aside>"),
            "got {html}"
        );
    }

    #[test]
    fn directive_without_h_name_drops() {
        let (mdast, _) = satteri_pulldown_cmark::parse(
            ":::note\nHello\n:::\n",
            satteri_pulldown_cmark::Options::ENABLE_GFM
                | satteri_pulldown_cmark::Options::ENABLE_DIRECTIVE,
        );
        let html = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        // No <note>, no <p>Hello</p> — the whole subtree dropped.
        assert!(!html.contains("Hello"), "got {html}");
    }

    #[test]
    fn directive_with_h_name_renders() {
        let (mut mdast, _) = satteri_pulldown_cmark::parse(
            ":::note\nHello\n:::\n",
            satteri_pulldown_cmark::Options::ENABLE_GFM
                | satteri_pulldown_cmark::Options::ENABLE_DIRECTIVE,
        );
        let dir_id = find_first(&mdast, MdastNodeType::ContainerDirective);
        set_data(
            &mut mdast,
            dir_id,
            r#"{"hName":"aside","hProperties":{"className":["note"]}}"#,
        );
        let html = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        assert!(html.contains("<aside class=\"note\">"), "got {html}");
        assert!(html.contains("Hello"), "got {html}");
        assert!(html.contains("</aside>"), "got {html}");
    }

    #[test]
    fn h_name_on_heading_keeps_children() {
        let mut mdast = parse_md("# Title\n");
        let heading_id = find_first(&mdast, MdastNodeType::Heading);
        set_data(&mut mdast, heading_id, r#"{"hName":"div"}"#);
        let html = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        assert!(html.contains("<div>Title</div>"), "got {html}");
    }

    #[test]
    fn h_properties_override_default_class() {
        let mut mdast = parse_md("- [ ] task\n");
        let item_id = find_first(&mdast, MdastNodeType::ListItem);
        // The default class for a task-list item is "task-list-item"; an
        // override should win.
        set_data(
            &mut mdast,
            item_id,
            r#"{"hProperties":{"className":["custom"]}}"#,
        );
        let html = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        assert!(html.contains("class=\"custom\""), "got {html}");
        assert!(!html.contains("task-list-item"), "got {html}");
    }

    #[test]
    fn invalid_data_json_is_ignored() {
        let mut mdast = parse_md("Hi\n");
        let para_id = find_first(&mdast, MdastNodeType::Paragraph);
        set_data(&mut mdast, para_id, "not json");
        let html = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        assert!(html.contains("<p>Hi</p>"), "got {html}");
    }

    #[test]
    fn data_without_h_fields_is_ignored() {
        let mut mdast = parse_md("Hi\n");
        let para_id = find_first(&mdast, MdastNodeType::Paragraph);
        set_data(&mut mdast, para_id, r#"{"someOther":"value"}"#);
        let html = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        assert!(html.contains("<p>Hi</p>"), "got {html}");
    }
}
