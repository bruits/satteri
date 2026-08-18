//! Convert an MDAST arena to a HAST arena.

use rustc_hash::FxHashMap;
use satteri_arena::{Arena, ArenaBuilder, Hast, Mdast, StringRef, decode_string_ref_data};

use crate::emit::{AttrName, AttrValue, Children, ConvertSink, EmitCtx, Pos, emit_node};
use crate::hast::HastNodeType;
use crate::mdast::{
    ListItemData, MdastNodeType, decode_definition_data, decode_footnote_definition_data,
    decode_list_item_data, decode_reference_data,
};
#[cfg(feature = "mdx")]
use crate::mdast::{
    decode_expression_data, decode_mdx_jsx_attr, decode_mdx_jsx_attr_count,
    decode_mdx_jsx_element_name, decode_mdx_jsx_explicit, encode_mdx_jsx_element_data,
};
use crate::shared::{PROP_BOOL_FALSE, PROP_BOOL_TRUE, PROP_INT, PROP_SPACE_SEP, PROP_STRING};
use crate::swar::{has_zero, splat};

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
#[inline]
pub(crate) fn contains_h_key(bytes: &[u8]) -> bool {
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

/// remark lowercases the identifier so fragment targets resist collisions across source casing.
pub(crate) fn footnote_fragment_id(identifier: &str) -> String {
    normalize_url(&identifier.to_ascii_lowercase()).into_owned()
}

#[inline]
pub(crate) fn normalize_url(url: &str) -> std::borrow::Cow<'_, str> {
    let bytes = url.as_bytes();
    // micromark's `normalizeUri` keeps a `%` as-is when it is followed by two
    // ASCII *alphanumerics* (not strictly hex digits, so `%ax` and `%2g` are
    // kept); otherwise the `%` itself is percent-encoded as `%25`.
    let pct_safe = |i: usize| -> bool {
        i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_alphanumeric()
            && bytes[i + 2].is_ascii_alphanumeric()
    };
    let mut from = 0;
    let needs_encode = loop {
        let Some(offset) = bytes[from..].iter().position(|&b| !URL_SAFE[b as usize]) else {
            break false;
        };
        let at = from + offset;
        if bytes[at] != b'%' || !pct_safe(at) {
            break true;
        }
        from = at + 1;
    };
    if !needs_encode {
        return std::borrow::Cow::Borrowed(url);
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
    std::borrow::Cow::Owned(encoded)
}

const fn url_safe_table() -> [bool; 256] {
    let mut table = [false; 256];
    let mut byte = 0usize;
    while byte < 256 {
        table[byte] = is_url_safe(byte as u8);
        byte += 1;
    }
    table
}

static URL_SAFE: [bool; 256] = url_safe_table();

const fn is_url_safe(b: u8) -> bool {
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

/// Conversion-time options that don't affect parsing
pub struct ConvertOptions {
    /// Visible-to-screen-readers label on the `<h2>` that opens the footnotes
    /// section. Default: `"Footnotes"`.
    pub footnote_label: String,
    /// Content of each backref `<a>`. The default emits `"↩"` for every
    /// backref; for k > 1, a `<sup>K</sup>` is appended automatically.
    pub footnote_back_content: Backref,
    /// `aria-label` on each backref `<a>`. The default template substitutes
    /// `{reference}` with the footnote number (e.g. `1`) for the first
    /// reference, or `number-K` (e.g. `1-2`) for subsequent references back
    /// to the same definition, matching remark-rehype's default.
    /// Default: `"Back to reference {reference}"`.
    pub footnote_back_label: Backref,
    /// Prefix applied to footnote IDs to prevent DOM clobbering.
    /// Default: `"user-content-"`.
    pub clobber_prefix: String,
    /// Reparse raw HTML embedded in the converted tree into real HAST nodes
    /// (see [`raw_to_hast_arena`](crate::hast::from_html::raw_to_hast_arena)).
    /// Applied as the final conversion step so every pipeline that converts
    /// MDAST to HAST gets it. Positions are not preserved through the reparse.
    /// Default: `false`.
    #[cfg(feature = "from-html")]
    pub raw_html: bool,
}

/// Value for per-backref strings. Either a template with the `{reference}`
/// placeholder, or a callback invoked with `(footnote_number, rerun_index)`
/// (both 1-based) returning the final string.
pub enum Backref {
    /// String template with the `{reference}` placeholder.
    Template(String),
    /// Per-backref callback. `rerun_index` starts at 1.
    Callback(Box<dyn Fn(usize, usize) -> String>),
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            footnote_label: "Footnotes".to_string(),
            footnote_back_content: Backref::Template("\u{21a9}".to_string()),
            footnote_back_label: Backref::Template("Back to reference {reference}".to_string()),
            clobber_prefix: "user-content-".to_string(),
            #[cfg(feature = "from-html")]
            raw_html: false,
        }
    }
}

pub(crate) fn resolve_backref(backref: &Backref, number: usize, k: usize) -> String {
    match backref {
        Backref::Template(tpl) => {
            let token = if k > 1 {
                format!("{}-{}", number, k)
            } else {
                number.to_string()
            };
            tpl.replace("{reference}", &token)
        }
        Backref::Callback(cb) => cb(number, k),
    }
}

/// Convert an MDAST arena directly to a HAST arena using default options.
pub fn mdast_arena_to_hast_arena(source: &Arena<Mdast>) -> Arena<Hast> {
    mdast_arena_to_hast_arena_impl(source, &ConvertOptions::default(), None)
}

/// Convert an MDAST arena to a HAST arena with the given conversion options.
pub fn mdast_arena_to_hast_arena_with_options(
    source: &Arena<Mdast>,
    options: &ConvertOptions,
) -> Arena<Hast> {
    mdast_arena_to_hast_arena_impl(source, options, None)
}

/// Reuse-friendly variant: takes a pre-pooled `Arena<Hast>`, resets it, and
/// fills it in instead of allocating a fresh arena. Saves the per-compile
/// `Vec` and `String` mallocs that dominate the cost on tiny inputs.
pub fn mdast_arena_to_hast_arena_into(
    source: &Arena<Mdast>,
    options: &ConvertOptions,
    reuse: Arena<Hast>,
) -> Arena<Hast> {
    mdast_arena_to_hast_arena_impl(source, options, Some(reuse))
}

fn mdast_arena_to_hast_arena_impl(
    source: &Arena<Mdast>,
    options: &ConvertOptions,
    reuse: Option<Arena<Hast>>,
) -> Arena<Hast> {
    let src = source.string_pool();
    let n = source.len();
    let mut hast_arena = if let Some(mut a) = reuse {
        a.reset();
        a.string_pool.reserve(src.len());
        a.string_pool.push_str(src);
        a.nodes.reserve(n);
        a.children.reserve(n);
        a.type_data.reserve(n * 20);
        a
    } else {
        Arena::<Hast>::with_capacity(src.to_string(), n, n, n * 20)
    };
    // Reuses the MDAST pool (heap included) so StringRefs stay valid; the
    // original-input prefix is identical, so carry the boundary over.
    hast_arena.source_len = source.source_len;
    let builder: ArenaBuilder<Hast> = ArenaBuilder::from_arena(hast_arena);
    let refs = collect_refs(source);
    let ctx = EmitCtx {
        view: source,
        refs: &refs,
        options,
    };
    let mut sink = HastSink::new(builder, source);
    emit_node(0, &ctx, &mut sink, 0);
    let arena = sink.finish();
    #[cfg(feature = "from-html")]
    if options.raw_html {
        return crate::hast::from_html::raw_to_hast_arena(&arena);
    }
    arena
}

/// Definition data stored as StringRefs into the MDAST source, avoids cloning strings.
pub(crate) struct Definition {
    pub(crate) url: StringRef,
    pub(crate) title: StringRef, // empty = no title
}

/// Single-pass collection of everything later arms need to cross-reference:
/// link/image reference definitions, plus source-order numbering for
/// footnote references and definitions.
pub(crate) struct CollectedRefs<'src> {
    pub(crate) defs: FxHashMap<&'src str, Definition>,
    /// `None` when the document contains no footnotes — saves the HashMap
    /// allocation on the common path.
    pub(crate) footnotes: Option<FxHashMap<&'src str, usize>>,
    /// FootnoteDefinition node ids in the order their identifiers are first
    /// referenced (main flow first, then inside definitions that got queued).
    pub(crate) footnote_defs: Vec<u32>,
    /// 1-based occurrence index for each FootnoteReference node id.
    pub(crate) footnote_ref_occurrence: FxHashMap<u32, usize>,
    /// Total reference count per identifier.
    pub(crate) footnote_ref_totals: FxHashMap<&'src str, usize>,
}

/// Flat probe over the node array for the two types [`collect_refs`] resolves.
#[inline]
fn has_any_ref_node(view: &Arena<Mdast>) -> bool {
    let definition = MdastNodeType::Definition as u8;
    let footnote_definition = MdastNodeType::FootnoteDefinition as u8;
    view.nodes
        .iter()
        .any(|n| n.node_type == definition || n.node_type == footnote_definition)
}

pub(crate) fn collect_refs(view: &Arena<Mdast>) -> CollectedRefs<'_> {
    let mut defs: FxHashMap<&str, Definition> = FxHashMap::default();
    let mut fn_def_nodes: FxHashMap<&str, u32> = FxHashMap::default();

    // First-wins for duplicate identifiers means *source order*, not
    // node-id order: top-level refdefs are appended at the end of the
    // root after blockquote-nested defs have already been allocated, so
    // their IDs come later even though they appear earlier in the
    // document. Collect Definition node ids first, then sort by source
    // position before inserting.
    // Root walk, not an id scan: in-place applies leave detached garbage that must not resolve refs
    let mut def_nodes: Vec<u32> = Vec::new();
    // Reachable implies present, so "none present" skips the walk safely.
    let mut stack: Vec<u32> = if has_any_ref_node(view) {
        vec![0]
    } else {
        Vec::new()
    };
    while let Some(id) = stack.pop() {
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
        // Push children in reverse so they pop in document order: `or_insert`
        // below is first-wins, and duplicate footnote definitions must resolve
        // to the *first* one in the document (matching remark-gfm).
        stack.extend(view.get_children(id).iter().rev().copied());
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

    // A footnote ref inside a directive only reaches the output when the
    // directive renders its own mdast children: it is dropped without an
    // `hName`, and `hChildren` replaces those children. Counting a ref that
    // won't render forces an empty footnote `<section>`.
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
            let h = HData::read(view, node_id);
            if h.h_name().is_none() || h.h_children().is_some() {
                return;
            }
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

/// Pre-built property data: refs already interned in the builder's string pool.
struct PropData {
    name_ref: StringRef,
    value_kind: u8,
    value_ref: StringRef,
}

#[inline]
pub(crate) fn list_contains_task_item(list_id: u32, view: &Arena<Mdast>) -> bool {
    for &child_id in view.get_children(list_id) {
        let child = view.get_node(child_id);
        if MdastNodeType::from_u8(child.node_type) != Some(MdastNodeType::ListItem) {
            continue;
        }
        let data = view.get_type_data(child_id);
        if data.len() < size_of::<ListItemData>() {
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

fn open_element_with_props(builder: &mut ArenaBuilder<Hast>, tag: &str, props: &[PropData]) -> u32 {
    let id = builder.open_node_raw(HastNodeType::Element as u8);
    let tag_ref = builder.alloc_string(tag);
    write_element_data(builder, tag_ref, props);
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
#[inline]
pub(crate) fn trim_lines_for_hast(value: &str) -> std::borrow::Cow<'_, str> {
    if !needs_line_trim(value.as_bytes()) {
        return std::borrow::Cow::Borrowed(value);
    }
    std::borrow::Cow::Owned(trim_lines_rewrite(value))
}

const fn is_line_break(byte: u8) -> bool {
    byte == b'\n' || byte == b'\r'
}

const fn is_space_or_tab(byte: u8) -> bool {
    byte == b' ' || byte == b'\t'
}

const fn line_break_mask(word: u64) -> u64 {
    has_zero(word ^ splat(b'\n')) | has_zero(word ^ splat(b'\r'))
}

fn line_break_touches_space_or_tab(bytes: &[u8], at: usize) -> bool {
    (at > 0 && is_space_or_tab(bytes[at - 1]))
        || bytes.get(at + 1).is_some_and(|&next| is_space_or_tab(next))
}

/// Every text node pays this scan, so it stays separate from the rewrite it guards.
/// A trimmed space or tab always sits beside a line break, so an adjacent pair
/// is the whole condition and `\r\n` needs no case of its own.
#[inline]
fn needs_line_trim(bytes: &[u8]) -> bool {
    let mut i = 0;
    while let Some(chunk) = bytes[i..].first_chunk::<8>() {
        let mut mask = line_break_mask(u64::from_le_bytes(*chunk));
        while mask != 0 {
            // Only `has_zero`'s lowest lane is trustworthy, so confirm each one.
            let at = i + (mask.trailing_zeros() / 8) as usize;
            if is_line_break(bytes[at]) && line_break_touches_space_or_tab(bytes, at) {
                return true;
            }
            mask &= mask - 1;
        }
        i += 8;
    }

    bytes[i..].iter().enumerate().any(|(offset, &byte)| {
        is_line_break(byte) && line_break_touches_space_or_tab(bytes, i + offset)
    })
}

fn trim_lines_rewrite(value: &str) -> String {
    let bytes = value.as_bytes();
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
    out
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
    // See `copy_position`: offsets flow through even in skip-positions mode.
    if node.start_line > 0 || node.start_offset > 0 || node.end_offset > 0 {
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
    // Skip-positions mode leaves line/col 0 but the parser still records byte
    // offsets; copy them so MDX codegen resolves line:col on demand via
    // `Location`. `readPosition` gates plugin `node.position` on the line, so a
    // non-opted-in plugin still sees `undefined`.
    if node.start_line > 0 || node.start_offset > 0 || node.end_offset > 0 {
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

/// Each line ending in a code span renders as one space, so a `\r\n` collapses
/// to a single space rather than two.
pub(crate) fn code_span_line_endings_to_spaces(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(i) = rest.find(['\r', '\n']) {
        out.push_str(&rest[..i]);
        out.push(' ');
        let after = &rest[i..];
        rest = after.strip_prefix("\r\n").unwrap_or(&after[1..]);
    }
    out.push_str(rest);
    out
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
    let source_bytes = arena.string_pool.as_bytes();
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

pub(crate) fn extract_text_content(node_id: u32, view: &Arena<Mdast>) -> String {
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

const MAX_INLINE_ATTRS: usize = 8;

#[derive(Clone, Copy)]
struct PendingAttr {
    name: &'static str,
    name_ref: StringRef,
    kind: u8,
    value: StringRef,
}

/// The sink that materializes property lists, positions, and `hName` overrides.
struct HastSink<'a> {
    builder: ArenaBuilder<Hast>,
    view: &'a Arena<Mdast>,
    /// Shared by every block separator; avoids re-pushing a single byte into the pool.
    newline_ref: StringRef,
    attrs: [PendingAttr; MAX_INLINE_ATTRS],
    attr_count: usize,
    tag: &'static str,
    pos: Pos,
    h: Option<HData>,
}

impl<'a> HastSink<'a> {
    fn new(mut builder: ArenaBuilder<Hast>, view: &'a Arena<Mdast>) -> Self {
        let newline_ref = builder.alloc_string("\n");
        let empty = PendingAttr {
            name: "",
            name_ref: StringRef::empty(),
            kind: 0,
            value: StringRef::empty(),
        };
        Self {
            builder,
            view,
            newline_ref,
            attrs: [empty; MAX_INLINE_ATTRS],
            attr_count: 0,
            tag: "",
            pos: Pos::None,
            h: None,
        }
    }

    fn finish(self) -> Arena<Hast> {
        self.builder.finish()
    }

    fn apply_pos_current(&mut self, pos: Pos) {
        match pos {
            Pos::None => {}
            Pos::Node(src_id) => copy_position(src_id, self.view, &mut self.builder),
            Pos::Span(first, last) => {
                let f = self.view.get_node(first);
                let l = self.view.get_node(last);
                self.builder.set_position_current(
                    f.start_offset,
                    l.end_offset,
                    f.start_line,
                    f.start_column,
                    l.end_line,
                    l.end_column,
                );
            }
        }
    }

    fn apply_pos_to(&mut self, node_id: u32, pos: Pos) {
        match pos {
            Pos::None => {}
            Pos::Node(src_id) => copy_position_to(node_id, src_id, self.view, &mut self.builder),
            Pos::Span(first, last) => {
                let f = self.view.get_node(first);
                let l = self.view.get_node(last);
                self.builder.arena_mut().set_position(
                    node_id,
                    f.start_offset,
                    l.end_offset,
                    f.start_line,
                    f.start_column,
                    l.end_line,
                    l.end_column,
                );
            }
        }
    }

    fn write_element(&mut self, tag: &str) {
        let count = self.attr_count;
        for i in 0..count {
            let name = self.attrs[i].name;
            self.attrs[i].name_ref = self.builder.alloc_string(name);
        }
        let tag_ref = self.builder.alloc_string(tag);
        let writer = self.builder.begin_data_current();
        let Self { attrs, builder, .. } = self;
        let out = &mut builder.arena_mut().type_data;
        out.extend_from_slice(&tag_ref.as_bytes());
        out.extend_from_slice(&(count as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        for attr in &attrs[..count] {
            out.extend_from_slice(&attr.name_ref.as_bytes());
            out.push(attr.kind);
            out.extend_from_slice(&[0u8; 3]);
            out.extend_from_slice(&attr.value.as_bytes());
        }
        builder.finish_data_current(writer);
    }

    fn finish_plain(&mut self) {
        let tag = self.tag;
        self.write_element(tag);
        let pos = self.pos;
        self.apply_pos_current(pos);
    }

    /// Outlined so the common h-less element keeps a straight-line finish.
    #[inline(never)]
    fn finish_h_element(&mut self, h: HData, allow_h_children: bool) -> Children {
        let count = self.attr_count;
        let defaults: Vec<(&str, u8, StringRef)> = self.attrs[..count]
            .iter()
            .map(|a| (a.name, a.kind, a.value))
            .collect();
        let props = merged_h_props(&mut self.builder, &defaults, h.h_properties());
        let tag = h.h_name().unwrap_or(self.tag);
        let tag_ref = self.builder.alloc_string(tag);
        write_element_data(&mut self.builder, tag_ref, &props);
        let pos = self.pos;
        self.apply_pos_current(pos);
        match h.h_children() {
            Some(children) if allow_h_children => {
                emit_h_children(&mut self.builder, children);
                Children::Replaced
            }
            _ => Children::Recurse,
        }
    }

    fn add_text_ref(&mut self, text_ref: StringRef, pos: Pos) {
        let id = add_text_node_with_ref(&mut self.builder, text_ref);
        self.apply_pos_to(id, pos);
    }
}

impl ConvertSink for HastSink<'_> {
    type BreakMark = usize;

    fn open_root(&mut self, pos: Pos) {
        self.builder.open_node_raw(HastNodeType::Root as u8);
        self.apply_pos_current(pos);
    }

    fn close_root(&mut self) {
        self.builder.close_node();
    }

    #[inline]
    fn open_element(&mut self, tag: &'static str, pos: Pos) {
        debug_assert!(self.h.is_none());
        self.builder.open_node_raw(HastNodeType::Element as u8);
        self.tag = tag;
        self.pos = pos;
        self.attr_count = 0;
    }

    #[inline]
    fn open_source_element(&mut self, tag: &'static str, src_id: u32) {
        self.builder.open_node_raw(HastNodeType::Element as u8);
        self.tag = tag;
        self.pos = Pos::Node(src_id);
        self.attr_count = 0;
        let h = HData::read(self.view, src_id);
        if !h.is_empty() {
            self.h = Some(h);
        }
    }

    #[inline]
    fn open_void(&mut self, tag: &'static str, pos: Pos) {
        self.open_element(tag, pos);
    }

    #[inline]
    fn open_source_void(&mut self, tag: &'static str, src_id: u32) {
        self.open_source_element(tag, src_id);
    }

    #[inline]
    fn attr(&mut self, name: AttrName, value: AttrValue<'_>) {
        let (kind, value_ref) = match value {
            AttrValue::Pooled(kind, sref) => (kind, sref),
            AttrValue::Text(kind, text) => (kind, self.builder.alloc_string(text)),
            AttrValue::Prefixed(kind, prefix, tail) => {
                let head = self.builder.alloc_string(prefix);
                (kind, self.builder.arena_mut().append_string(head, tail))
            }
            AttrValue::Flag(true) => (PROP_BOOL_TRUE, StringRef::empty()),
            AttrValue::Flag(false) => (PROP_BOOL_FALSE, StringRef::empty()),
        };
        let i = self.attr_count;
        debug_assert!(i < MAX_INLINE_ATTRS, "too many props for the inline buffer");
        self.attrs[i].name = name.property;
        self.attrs[i].kind = kind;
        self.attrs[i].value = value_ref;
        self.attr_count = i + 1;
    }

    #[inline]
    fn finish_attrs(&mut self) {
        self.finish_plain();
    }

    #[inline]
    fn finish_source_attrs(&mut self) -> Children {
        match self.h.take() {
            None => {
                self.finish_plain();
                Children::Recurse
            }
            Some(h) => self.finish_h_element(h, true),
        }
    }

    #[inline]
    fn finish_void(&mut self) {
        self.finish_plain();
        self.builder.close_node();
    }

    #[inline]
    fn finish_source_void(&mut self) {
        match self.h.take() {
            None => self.finish_plain(),
            Some(h) => {
                self.finish_h_element(h, false);
            }
        }
        self.builder.close_node();
    }

    #[inline]
    fn close_element(&mut self, _tag: &'static str) {
        self.builder.close_node();
    }

    fn text(&mut self, value: &str, pos: Pos) {
        let text_ref = self.builder.alloc_string(value);
        self.add_text_ref(text_ref, pos);
    }

    fn text_pooled(&mut self, value: StringRef, pos: Pos) {
        self.add_text_ref(value, pos);
    }

    fn text_trimmed(&mut self, value: StringRef, pos: Pos) {
        let raw = self.view.get_str(value);
        match trim_lines_for_hast(raw) {
            std::borrow::Cow::Borrowed(_) => self.add_text_ref(value, pos),
            std::borrow::Cow::Owned(trimmed) => {
                let text_ref = self.builder.alloc_string(&trimmed);
                self.add_text_ref(text_ref, pos);
            }
        }
    }

    fn text_with_trailing_space(&mut self, value: StringRef, pos: Pos) {
        let text_ref = self.builder.arena_mut().append_string(value, " ");
        self.add_text_ref(text_ref, pos);
    }

    fn code_block_text(&mut self, value: &str, pos: Pos) {
        let text_ref = self.builder.alloc_string(value);
        let text_ref = if value.is_empty() {
            text_ref
        } else {
            self.builder.arena_mut().append_string(text_ref, "\n")
        };
        self.add_text_ref(text_ref, pos);
    }

    fn raw_html(&mut self, value: &str, pos: Pos) {
        let id = add_raw_node(&mut self.builder, value);
        self.apply_pos_to(id, pos);
    }

    #[inline]
    fn newline(&mut self) {
        let newline_ref = self.newline_ref;
        self.add_text_ref(newline_ref, Pos::None);
    }

    fn code_info(&mut self, lang: StringRef, meta: StringRef) {
        let view = self.view;
        let lang = view.get_str(lang);
        let meta = view.get_str(meta);
        if lang.is_empty() && meta.is_empty() {
            return;
        }
        let json = encode_code_node_data(lang, meta);
        let id = self.builder.current_node_id();
        self.builder.arena_mut().set_node_data(id, json);
    }

    #[inline]
    fn mark_break_boundary(&mut self, after_break: bool) -> usize {
        if after_break {
            self.builder.current_pending_children().len()
        } else {
            0
        }
    }

    #[inline]
    fn apply_break_trim(&mut self, after_break: bool, mark: usize) {
        if !after_break {
            return;
        }
        let pending = self.builder.current_pending_children();
        if pending.len() > mark {
            let first_new = pending[mark];
            trim_leading_ws_after_break(&mut self.builder, first_new);
        }
    }

    fn produces_output(&self, child_id: u32) -> bool {
        produces_hast_output(child_id, self.view)
    }

    fn has_no_h_data(&self, node_id: u32) -> bool {
        HData::read(self.view, node_id).is_empty()
    }

    fn open_directive(&mut self, node_id: u32) -> Option<Children> {
        let h = HData::read(self.view, node_id);
        let name = h.h_name()?;
        let props = merged_h_props(&mut self.builder, &[], h.h_properties());
        open_element_with_props(&mut self.builder, name, &props);
        copy_position(node_id, self.view, &mut self.builder);
        match h.h_children() {
            Some(children) => {
                emit_h_children(&mut self.builder, children);
                Some(Children::Replaced)
            }
            None => Some(Children::Recurse),
        }
    }

    fn close_dynamic_element(&mut self) {
        self.builder.close_node();
    }

    #[cfg(feature = "mdx")]
    fn open_mdx_jsx_element(&mut self, node_id: u32, node_type: MdastNodeType) -> bool {
        let hast_type = if node_type == MdastNodeType::MdxJsxTextElement {
            HastNodeType::MdxJsxTextElement as u8
        } else {
            HastNodeType::MdxJsxElement as u8
        };
        let view = self.view;
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
        let name_ref = self.builder.alloc_string(name_str);

        // MDAST and HAST share the same attribute binary layout.
        let attr_count = if mdast_data.len() >= 12 {
            decode_mdx_jsx_attr_count(mdast_data)
        } else {
            0
        };
        let explicit_jsx = decode_mdx_jsx_explicit(mdast_data);
        let mut attr_tuples = Vec::with_capacity(attr_count as usize);
        for i in 0..attr_count {
            attr_tuples.push(decode_mdx_jsx_attr(mdast_data, i));
        }

        self.builder.open_node_raw(hast_type);
        let encoded = encode_mdx_jsx_element_data(name_ref, &attr_tuples, explicit_jsx);
        self.builder.set_data_current(&encoded);
        if let Some(mdast_nd) = view.get_node_data(node_id)
            && !mdast_nd.is_empty()
        {
            let id = self.builder.current_node_id();
            let copy = mdast_nd.to_vec();
            self.builder.arena_mut().set_node_data(id, copy);
        }
        copy_position(node_id, view, &mut self.builder);
        true
    }

    #[cfg(feature = "mdx")]
    fn mdx_leaf(&mut self, node_id: u32, node_type: MdastNodeType) {
        let hast_type = match node_type {
            MdastNodeType::MdxFlowExpression => HastNodeType::MdxFlowExpression as u8,
            MdastNodeType::MdxTextExpression => HastNodeType::MdxTextExpression as u8,
            _ => HastNodeType::MdxEsm as u8,
        };
        let view = self.view;
        let data = view.get_type_data(node_id);
        let value = if data.is_empty() {
            ""
        } else {
            view.get_str(decode_expression_data(data).value)
        };
        let value_ref = self.builder.alloc_string(value);
        let leaf_id = self.builder.add_leaf_raw(hast_type);
        self.builder
            .arena_mut()
            .set_type_data(leaf_id, &value_ref.as_bytes());
        let node = view.get_node(node_id);
        self.builder.arena_mut().set_position(
            leaf_id,
            node.start_offset,
            node.end_offset,
            node.start_line,
            node.start_column,
            node.end_line,
            node.end_column,
        );
    }
}

#[cfg(test)]
mod hast_convert_tests {
    use super::*;

    #[cfg(feature = "mdx")]
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

    #[cfg(feature = "mdx")]
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

    use crate::hast::{HastNodeType, hast_arena_to_html};

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
    fn footnote_ref_inside_rendered_directive_is_numbered() {
        // Regression test for #157.
        let (mut mdast, _) = satteri_pulldown_cmark::parse(
            "Outside.[^a]\n\n:::note\nInside.[^b]\n:::\n\n[^a]: One.\n\n[^b]: Two.\n",
            satteri_pulldown_cmark::Options::ENABLE_GFM
                | satteri_pulldown_cmark::Options::ENABLE_DIRECTIVE
                | satteri_pulldown_cmark::Options::ENABLE_FOOTNOTES,
        );
        let dir_id = find_first(&mdast, MdastNodeType::ContainerDirective);
        set_data(&mut mdast, dir_id, r#"{"hName":"div"}"#);
        let html = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        assert!(!html.contains("[^b]"), "ref rendered literally, got {html}");
        assert!(
            html.contains("user-content-fnref-b"),
            "nested ref not numbered, got {html}"
        );
        assert!(
            html.contains("user-content-fn-b") && html.contains("Two."),
            "nested definition dropped, got {html}"
        );
        // Numbering follows document order: `a` (#1) before `b` (#2).
        let a_pos = html.find("user-content-fn-a").unwrap();
        let b_pos = html.find("user-content-fn-b").unwrap();
        assert!(a_pos < b_pos, "footnotes out of order, got {html}");
    }

    #[test]
    fn footnote_ref_inside_h_children_directive_is_not_numbered() {
        // `hChildren` replaces the mdast children, so the nested ref never
        // renders and must not force a footnote `<section>`.
        let (mut mdast, _) = satteri_pulldown_cmark::parse(
            ":::note\nInside.[^b]\n:::\n\n[^b]: Two.\n",
            satteri_pulldown_cmark::Options::ENABLE_GFM
                | satteri_pulldown_cmark::Options::ENABLE_DIRECTIVE
                | satteri_pulldown_cmark::Options::ENABLE_FOOTNOTES,
        );
        let dir_id = find_first(&mdast, MdastNodeType::ContainerDirective);
        set_data(
            &mut mdast,
            dir_id,
            r#"{"hName":"div","hChildren":[{"type":"text","value":"replaced"}]}"#,
        );
        let html = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        assert!(
            html.contains("replaced"),
            "hChildren not rendered, got {html}"
        );
        assert!(!html.contains("class=\"footnotes\""), "got {html}");
        assert!(!html.contains("user-content-fn-b"), "got {html}");
    }

    #[test]
    fn footnote_ref_inside_dropped_directive_is_not_numbered() {
        // Without an `hName` the directive is dropped, so the nested ref must
        // not force a footnote `<section>`.
        let (mdast, _) = satteri_pulldown_cmark::parse(
            ":::note\nInside.[^b]\n:::\n\n[^b]: Two.\n",
            satteri_pulldown_cmark::Options::ENABLE_GFM
                | satteri_pulldown_cmark::Options::ENABLE_DIRECTIVE
                | satteri_pulldown_cmark::Options::ENABLE_FOOTNOTES,
        );
        let html = hast_arena_to_html(&mdast_arena_to_hast_arena(&mdast));
        assert!(!html.contains("class=\"footnotes\""), "got {html}");
        assert!(!html.contains("user-content-fn-b"), "got {html}");
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

    /// Reference oracle: the straightforward byte-at-a-time form of the predicate.
    fn needs_line_trim_scalar(bytes: &[u8]) -> bool {
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'\n' || b == b'\r' {
                if i > 0 && (bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
                    return true;
                }
                let after = if b == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                    i + 2
                } else {
                    i + 1
                };
                if after < bytes.len() && (bytes[after] == b' ' || bytes[after] == b'\t') {
                    return true;
                }
                i = after;
                continue;
            }
            i += 1;
        }
        false
    }

    fn check_line_trim(value: &str) {
        let bytes = value.as_bytes();
        assert_eq!(
            super::needs_line_trim(bytes),
            needs_line_trim_scalar(bytes),
            "needs_line_trim disagrees on {value:?}"
        );
    }

    #[test]
    fn line_trim_scan_matches_scalar_on_every_short_arrangement_at_every_alignment() {
        // 0x0b and 0x0e are the bytes a `\n` or `\r` lane can spuriously light up above itself.
        let alphabet = [b'\n', b'\r', b' ', b'\t', b'x', 0x0b, 0x0e];
        for len in 0..=4u32 {
            for encoded in 0..alphabet.len().pow(len) {
                let mut rest = encoded;
                let suffix: Vec<u8> = (0..len)
                    .map(|_| {
                        let byte = alphabet[rest % alphabet.len()];
                        rest /= alphabet.len();
                        byte
                    })
                    .collect();
                let suffix = std::str::from_utf8(&suffix).unwrap();
                for lead in 0..=8usize {
                    check_line_trim(&("x".repeat(lead) + suffix));
                }
            }
        }
    }

    #[test]
    fn line_trim_scan_matches_scalar_at_every_offset_and_length() {
        for pair in [
            "\n ", " \n", "\r\t", "\t\r", "\r\n", "\n\r", " \r\n", "\r\n ", "\n\u{b}", "\r\u{e}",
            "\n\u{b} ", "\r\u{e} ",
        ] {
            for len in pair.len()..=40usize {
                for at in 0..=len - pair.len() {
                    let mut s = "x".repeat(len);
                    s.replace_range(at..at + pair.len(), pair);
                    check_line_trim(&s);
                }
            }
        }
    }

    #[test]
    fn line_trim_scan_matches_scalar_on_multibyte_utf8() {
        let pieces = ["é", "€", "🎉", "日", "x", "\n", "\r", " ", "\t"];
        for lead in 0..12usize {
            for a in pieces {
                for b in pieces {
                    let mut s = "z".repeat(lead);
                    s.push_str(a);
                    s.push_str(b);
                    s.push_str(a);
                    check_line_trim(&s);
                    s.push_str("tail");
                    check_line_trim(&s);
                }
            }
        }
    }

    #[test]
    fn line_trim_scan_matches_scalar_on_pseudorandom_bytes() {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let alphabet = b"\n\r \tabc.\x0b\x0e";
        for len in 0..300usize {
            let mut s = String::with_capacity(len);
            for _ in 0..len {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                s.push(alphabet[(state % alphabet.len() as u64) as usize] as char);
            }
            check_line_trim(&s);
        }
    }

    /// Reference oracle: the straightforward per-byte form of the encode decision.
    fn url_needs_encode_scalar(url: &str) -> bool {
        let bytes = url.as_bytes();
        bytes.iter().enumerate().any(|(i, &b)| {
            let pct_safe = i + 2 < bytes.len()
                && bytes[i + 1].is_ascii_alphanumeric()
                && bytes[i + 2].is_ascii_alphanumeric();
            if b == b'%' {
                !pct_safe
            } else {
                !is_url_safe(b)
            }
        })
    }

    fn check_normalize_url(url: &str) {
        let borrowed = matches!(normalize_url(url), std::borrow::Cow::Borrowed(_));
        assert_eq!(
            !borrowed,
            url_needs_encode_scalar(url),
            "normalize_url borrow decision disagrees on {url:?}"
        );
    }

    #[test]
    fn normalize_url_matches_scalar_on_percent_sequences() {
        for tail in ["", "a", "ab", "abc", "%", "%a", "%ab", "2f", "2g", " ", "é"] {
            for head in ["", "x", "https://a.b/", "%", "%2f"] {
                check_normalize_url(&format!("{head}%{tail}"));
                check_normalize_url(&format!("{head}{tail}"));
            }
        }
    }

    #[test]
    fn normalize_url_matches_scalar_on_pseudorandom_urls() {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let alphabet = b"%abZ09-._~:/?#@!$&'()*+,;= <>\"{}|\\^`\n\t";
        for len in 0..200usize {
            let mut s = String::with_capacity(len);
            for _ in 0..len {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                s.push(alphabet[(state % alphabet.len() as u64) as usize] as char);
            }
            check_normalize_url(&s);
        }
    }

    #[test]
    fn trim_lines_borrows_when_nothing_changes() {
        for value in [
            "", "x", "a\nb", "a\r\nb", "\n", "\r\n", "a b", " lead", "end ",
        ] {
            assert!(
                matches!(trim_lines_for_hast(value), std::borrow::Cow::Borrowed(_)),
                "expected a borrow for {value:?}"
            );
        }
    }

    #[test]
    fn code_span_line_endings_render_as_one_space() {
        let f = super::code_span_line_endings_to_spaces;
        assert_eq!(f("a\r\nb"), "a b");
        assert_eq!(f("a\rb"), "a b");
        assert_eq!(f("a\nb"), "a b");
        assert_eq!(f("a\r\n\r\nb"), "a  b");
        assert_eq!(f("\r\n"), " ");
        assert_eq!(f("ab"), "ab");
    }
}
