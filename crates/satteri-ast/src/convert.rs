//! Convert an MDAST arena to a HAST arena.

use rustc_hash::FxHashMap;
use satteri_arena::{decode_string_ref_data, Arena, ArenaBuilder, StringRef};

use crate::hast::codec::encode_element_data_into;
use crate::hast::HastNodeType;
use crate::mdast::{
    decode_code_data, decode_definition_data, decode_expression_data,
    decode_footnote_definition_data, decode_heading_data, decode_image_data, decode_link_data,
    decode_list_data, decode_list_item_data, decode_math_data, decode_mdx_jsx_attr,
    decode_mdx_jsx_attr_count, decode_mdx_jsx_element_name, decode_reference_data,
    decode_table_alignments, encode_mdx_jsx_element_data, ColumnAlign, ListItemData, MdastNodeType,
};
use crate::shared::{PROP_BOOL_FALSE, PROP_BOOL_TRUE, PROP_INT, PROP_SPACE_SEP, PROP_STRING};

fn encode_url(builder: &mut ArenaBuilder, url: &str) -> StringRef {
    if url.bytes().all(is_url_safe) {
        return builder.alloc_string(url);
    }
    let mut encoded = String::with_capacity(url.len() * 2);
    for byte in url.bytes() {
        if is_url_safe(byte) {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(hex_digit(byte >> 4));
            encoded.push(hex_digit(byte & 0xf));
        }
    }
    builder.alloc_string(&encoded)
}

fn is_url_safe(b: u8) -> bool {
    matches!(b,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
        | b'-' | b'.' | b'_' | b'~'
        | b':' | b'/' | b'?' | b'#' | b'@'
        | b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
        | b'%'
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
pub fn mdast_arena_to_hast_arena(source: &Arena) -> Arena {
    let src = source.source();
    let mut builder = ArenaBuilder::new(src.to_string());
    // Pre-allocate based on source arena size.
    let n = source.len();
    builder.arena_mut().nodes.reserve(n);
    builder.arena_mut().children.reserve(n);
    builder.arena_mut().type_data.reserve(n * 20);
    let refs = collect_refs(source);
    let ctx = ConvertCtx {
        defs: &refs.defs,
        footnotes: refs.footnotes.as_ref(),
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
    defs: &'a [Definition],
    footnotes: Option<&'a FxHashMap<&'src str, usize>>,
}

/// Definition data stored as StringRefs into the MDAST source, avoids cloning strings.
struct Definition {
    identifier: StringRef,
    url: StringRef,
    title: StringRef, // empty = no title
}

/// Single-pass collection of everything later arms need to cross-reference:
/// link/image reference definitions, plus source-order numbering for
/// footnote references and definitions.
struct CollectedRefs<'src> {
    defs: Vec<Definition>,
    /// `None` when the document contains no footnotes — saves the HashMap
    /// allocation on the common path.
    footnotes: Option<FxHashMap<&'src str, usize>>,
}

fn collect_refs(view: &Arena) -> CollectedRefs<'_> {
    let mut defs = Vec::new();
    let mut footnotes: Option<FxHashMap<&str, usize>> = None;
    let mut next_footnote = 1usize;

    for id in 0..view.len() as u32 {
        let node = view.get_node(id);
        let data = view.get_type_data(id);
        match MdastNodeType::from_u8(node.node_type) {
            Some(MdastNodeType::Definition) if data.len() >= 32 => {
                let dd = decode_definition_data(data);
                defs.push(Definition {
                    identifier: dd.identifier,
                    url: dd.url,
                    title: dd.title,
                });
            }
            Some(MdastNodeType::FootnoteReference) if data.len() >= 20 => {
                let rd = decode_reference_data(data);
                let identifier = view.get_str(rd.identifier);
                let map = footnotes.get_or_insert_with(FxHashMap::default);
                if !map.contains_key(identifier) {
                    map.insert(identifier, next_footnote);
                    next_footnote += 1;
                }
            }
            Some(MdastNodeType::FootnoteDefinition) if data.len() >= 16 => {
                let fd = decode_footnote_definition_data(data);
                let identifier = view.get_str(fd.identifier);
                let map = footnotes.get_or_insert_with(FxHashMap::default);
                if !map.contains_key(identifier) {
                    map.insert(identifier, next_footnote);
                    next_footnote += 1;
                }
            }
            _ => {}
        }
    }

    CollectedRefs { defs, footnotes }
}

fn find_def<'a>(defs: &'a [Definition], view: &Arena, identifier: &str) -> Option<&'a Definition> {
    defs.iter()
        .find(|d| view.get_str(d.identifier) == identifier)
}

/// Pre-built property data: refs already interned in the builder's string pool.
struct PropData {
    name_ref: StringRef,
    value_kind: u8,
    value_ref: StringRef,
}

fn build_props(builder: &mut ArenaBuilder, specs: &[(&str, u8, StringRef)]) -> Vec<PropData> {
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

fn list_contains_task_item(list_id: u32, view: &Arena) -> bool {
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

fn write_element_data(builder: &mut ArenaBuilder, tag_ref: StringRef, props: &[PropData]) {
    let writer = builder.begin_data_current();
    let tuples: Vec<(StringRef, u8, StringRef)> = props
        .iter()
        .map(|p| (p.name_ref, p.value_kind, p.value_ref))
        .collect();
    encode_element_data_into(tag_ref, &tuples, &mut builder.arena_mut().type_data);
    builder.finish_data_current(writer);
}

fn open_element(builder: &mut ArenaBuilder, tag: &str) -> u32 {
    let id = builder.open_node_raw(HastNodeType::Element as u8);
    let tag_ref = builder.alloc_string(tag);
    let writer = builder.begin_data_current();
    encode_element_data_into(tag_ref, &[], &mut builder.arena_mut().type_data);
    builder.finish_data_current(writer);
    id
}

fn open_element_with_props(builder: &mut ArenaBuilder, tag: &str, props: &[PropData]) -> u32 {
    let id = builder.open_node_raw(HastNodeType::Element as u8);
    let tag_ref = builder.alloc_string(tag);
    write_element_data(builder, tag_ref, props);
    id
}

fn add_void_element(builder: &mut ArenaBuilder, tag: &str) -> u32 {
    let id = builder.open_node_raw(HastNodeType::Element as u8);
    let tag_ref = builder.alloc_string(tag);
    let writer = builder.begin_data_current();
    encode_element_data_into(tag_ref, &[], &mut builder.arena_mut().type_data);
    builder.finish_data_current(writer);
    builder.close_node();
    id
}

fn add_void_element_with_props(builder: &mut ArenaBuilder, tag: &str, props: &[PropData]) -> u32 {
    let id = builder.open_node_raw(HastNodeType::Element as u8);
    let tag_ref = builder.alloc_string(tag);
    write_element_data(builder, tag_ref, props);
    builder.close_node();
    id
}

fn add_text_node(builder: &mut ArenaBuilder, text: &str) -> u32 {
    let text_ref = builder.alloc_string(text);
    let leaf_id = builder.add_leaf_raw(HastNodeType::Text as u8);
    builder
        .arena_mut()
        .set_type_data(leaf_id, &text_ref.as_bytes());
    leaf_id
}

fn add_raw_node(builder: &mut ArenaBuilder, html: &str) -> u32 {
    let html_ref = builder.alloc_string(html);
    let leaf_id = builder.add_leaf_raw(HastNodeType::Raw as u8);
    builder
        .arena_mut()
        .set_type_data(leaf_id, &html_ref.as_bytes());
    leaf_id
}

/// Set position on a node by id, copying from the given source mdast node.
/// Used for leaf nodes (void elements, text, raw) which can't use `set_position_current`.
fn copy_position_to(target_id: u32, src_node_id: u32, view: &Arena, builder: &mut ArenaBuilder) {
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

    let mut buf = Vec::with_capacity(32 + lang.len() + meta.len());
    buf.extend_from_slice(b"{\"lang\":\"");
    json_escape(lang, &mut buf);
    buf.extend_from_slice(b"\",\"meta\":\"");
    json_escape(meta, &mut buf);
    buf.extend_from_slice(b"\"}");
    buf
}

fn copy_position(node_id: u32, view: &Arena, builder: &mut ArenaBuilder) {
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

fn convert_node(node_id: u32, view: &Arena, builder: &mut ArenaBuilder, ctx: &ConvertCtx<'_, '_>) {
    let node = view.get_node(node_id);
    let raw_type = node.node_type;

    match MdastNodeType::from_u8(raw_type) {
        Some(MdastNodeType::Root) => {
            builder.open_node_raw(HastNodeType::Root as u8);
            copy_position(node_id, view, builder);
            convert_children_wrapped(node_id, view, builder, ctx);
            builder.close_node();
        }

        Some(MdastNodeType::Paragraph) => {
            // Note: MDX paragraph unraveling is handled by convert_children_wrapped
            // at the parent level, so by the time we get here it's a normal <p>.
            open_element(builder, "p");
            copy_position(node_id, view, builder);
            convert_children(node_id, view, builder, ctx);
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
            open_element(builder, tag);
            copy_position(node_id, view, builder);
            convert_children(node_id, view, builder, ctx);
            builder.close_node();
        }

        Some(MdastNodeType::ThematicBreak) => {
            let id = add_void_element(builder, "hr");
            copy_position_to(id, node_id, view, builder);
        }

        Some(MdastNodeType::Blockquote) => {
            open_element(builder, "blockquote");
            copy_position(node_id, view, builder);
            convert_children_with_newlines(node_id, view, builder, ctx);
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

            if prop_specs.is_empty() {
                open_element(builder, tag);
            } else {
                let props = build_props(builder, &prop_specs);
                open_element_with_props(builder, tag, &props);
            }
            copy_position(node_id, view, builder);
            convert_children_with_newlines(node_id, view, builder, ctx);
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

            if is_task {
                let class_ref = builder.alloc_string("task-list-item");
                let props = build_props(builder, &[("className", PROP_SPACE_SEP, class_ref)]);
                open_element_with_props(builder, "li", &props);
            } else {
                open_element(builder, "li");
            }
            copy_position(node_id, view, builder);

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

            open_element(builder, "pre");
            copy_position(node_id, view, builder);
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
            let id = add_text_node(builder, view.get_str(string_ref));
            copy_position_to(id, node_id, view, builder);
        }

        Some(MdastNodeType::Emphasis) => {
            open_element(builder, "em");
            copy_position(node_id, view, builder);
            convert_children(node_id, view, builder, ctx);
            builder.close_node();
        }

        Some(MdastNodeType::Strong) => {
            open_element(builder, "strong");
            copy_position(node_id, view, builder);
            convert_children(node_id, view, builder, ctx);
            builder.close_node();
        }

        Some(MdastNodeType::InlineCode) => {
            let data = view.get_type_data(node_id);
            let string_ref = decode_string_ref_data(data);
            open_element(builder, "code");
            copy_position(node_id, view, builder);
            let value = view.get_str(string_ref);
            if value.contains('\n') {
                let normalized = value.replace('\n', " ");
                let text_id = add_text_node(builder, &normalized);
                copy_position_to(text_id, node_id, view, builder);
            } else {
                let text_id = add_text_node(builder, value);
                copy_position_to(text_id, node_id, view, builder);
            }
            builder.close_node();
        }

        Some(MdastNodeType::Break) => {
            let id = add_void_element(builder, "br");
            copy_position_to(id, node_id, view, builder);
            add_text_node(builder, "\n");
        }

        Some(MdastNodeType::Link) => {
            let data = view.get_type_data(node_id);
            let link_data = decode_link_data(data);
            let url_ref = encode_url(builder, view.get_str(link_data.url));
            if link_data.title.len > 0 {
                let title_ref = builder.alloc_string(view.get_str(link_data.title));
                let props = build_props(
                    builder,
                    &[
                        ("href", PROP_STRING, url_ref),
                        ("title", PROP_STRING, title_ref),
                    ],
                );
                open_element_with_props(builder, "a", &props);
            } else {
                let props = build_props(builder, &[("href", PROP_STRING, url_ref)]);
                open_element_with_props(builder, "a", &props);
            }
            copy_position(node_id, view, builder);
            convert_children(node_id, view, builder, ctx);
            builder.close_node();
        }

        Some(MdastNodeType::Image) => {
            let data = view.get_type_data(node_id);
            let img_data = decode_image_data(data);
            let url_ref = encode_url(builder, view.get_str(img_data.url));
            let alt_ref = builder.alloc_string(view.get_str(img_data.alt));
            let id = if img_data.title.len > 0 {
                let title_ref = builder.alloc_string(view.get_str(img_data.title));
                let props = build_props(
                    builder,
                    &[
                        ("src", PROP_STRING, url_ref),
                        ("alt", PROP_STRING, alt_ref),
                        ("title", PROP_STRING, title_ref),
                    ],
                );
                add_void_element_with_props(builder, "img", &props)
            } else {
                let props = build_props(
                    builder,
                    &[("src", PROP_STRING, url_ref), ("alt", PROP_STRING, alt_ref)],
                );
                add_void_element_with_props(builder, "img", &props)
            };
            copy_position_to(id, node_id, view, builder);
        }

        Some(MdastNodeType::Delete) => {
            open_element(builder, "del");
            copy_position(node_id, view, builder);
            convert_children(node_id, view, builder, ctx);
            builder.close_node();
        }

        Some(MdastNodeType::Table) => {
            let alignments = decode_table_alignments(view.get_type_data(node_id));
            open_element(builder, "table");
            copy_position(node_id, view, builder);
            let child_ids = view.get_children(node_id);
            if !child_ids.is_empty() {
                add_text_node(builder, "\n");
                open_element(builder, "thead");
                copy_position(child_ids[0], view, builder);
                add_text_node(builder, "\n");
                convert_table_row(child_ids[0], view, builder, ctx, true, &alignments);
                add_text_node(builder, "\n");
                builder.close_node(); // thead
                add_text_node(builder, "\n");

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
                    add_text_node(builder, "\n");
                    for &row_id in &child_ids[1..] {
                        convert_table_row(row_id, view, builder, ctx, false, &alignments);
                        add_text_node(builder, "\n");
                    }
                    builder.close_node(); // tbody
                    add_text_node(builder, "\n");
                }
            }
            builder.close_node(); // table
        }

        Some(MdastNodeType::Math) => {
            let data = view.get_type_data(node_id);
            let math_data = decode_math_data(data);
            let value = view.get_str(math_data.value);
            let class_ref = builder.alloc_string("language-math math-display");
            let props = build_props(builder, &[("className", PROP_SPACE_SEP, class_ref)]);
            open_element(builder, "pre");
            copy_position(node_id, view, builder);
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
            let props = build_props(builder, &[("className", PROP_SPACE_SEP, class_ref)]);
            open_element_with_props(builder, "code", &props);
            copy_position(node_id, view, builder);
            add_text_node(builder, value);
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
                    if !def.title.is_empty() {
                        let title_ref = def.title;
                        let props = build_props(
                            builder,
                            &[
                                ("href", PROP_STRING, url_ref),
                                ("title", PROP_STRING, title_ref),
                            ],
                        );
                        open_element_with_props(builder, "a", &props);
                    } else {
                        let props = build_props(builder, &[("href", PROP_STRING, url_ref)]);
                        open_element_with_props(builder, "a", &props);
                    }
                    copy_position(node_id, view, builder);
                    convert_children(node_id, view, builder, ctx);
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
                    let alt = extract_text_content(node_id, view);
                    let url_ref = encode_url(builder, view.get_str(def.url));
                    let alt_ref = builder.alloc_string(&alt);
                    let id = if !def.title.is_empty() {
                        let title_ref = def.title;
                        let props = build_props(
                            builder,
                            &[
                                ("src", PROP_STRING, url_ref),
                                ("alt", PROP_STRING, alt_ref),
                                ("title", PROP_STRING, title_ref),
                            ],
                        );
                        add_void_element_with_props(builder, "img", &props)
                    } else {
                        let props = build_props(
                            builder,
                            &[("src", PROP_STRING, url_ref), ("alt", PROP_STRING, alt_ref)],
                        );
                        add_void_element_with_props(builder, "img", &props)
                    };
                    copy_position_to(id, node_id, view, builder);
                }
            }
        }

        Some(MdastNodeType::FootnoteReference) => {
            let data = view.get_type_data(node_id);
            if data.len() >= 20 {
                let rd = decode_reference_data(data);
                let identifier = view.get_str(rd.identifier);
                // `collect_refs` visits every FootnoteReference and
                // FootnoteDefinition in the source arena, so reaching this
                // arm guarantees the map exists and contains `identifier`.
                let number = ctx
                    .footnotes
                    .and_then(|m| m.get(identifier).copied())
                    .expect("footnote identifier missing from collected numbers");
                let href = format!("#{}", identifier);
                let href_ref = builder.alloc_string(&href);
                let class_ref = builder.alloc_string("footnote-reference");
                let sup_props = build_props(builder, &[("className", PROP_SPACE_SEP, class_ref)]);
                open_element_with_props(builder, "sup", &sup_props);
                copy_position(node_id, view, builder);
                let a_props = build_props(builder, &[("href", PROP_STRING, href_ref)]);
                open_element_with_props(builder, "a", &a_props);
                let num_str = number.to_string();
                add_text_node(builder, &num_str);
                builder.close_node(); // a
                builder.close_node(); // sup
            }
        }

        Some(MdastNodeType::FootnoteDefinition) => {
            let data = view.get_type_data(node_id);
            if data.len() >= 16 {
                let fd = decode_footnote_definition_data(data);
                let identifier = view.get_str(fd.identifier);
                // `collect_refs` visits every FootnoteReference and
                // FootnoteDefinition in the source arena, so reaching this
                // arm guarantees the map exists and contains `identifier`.
                let number = ctx
                    .footnotes
                    .and_then(|m| m.get(identifier).copied())
                    .expect("footnote identifier missing from collected numbers");
                let id_ref = builder.alloc_string(identifier);
                let class_ref = builder.alloc_string("footnote-definition");
                let div_props = build_props(
                    builder,
                    &[
                        ("className", PROP_SPACE_SEP, class_ref),
                        ("id", PROP_STRING, id_ref),
                    ],
                );
                open_element_with_props(builder, "div", &div_props);
                copy_position(node_id, view, builder);
                let label_class_ref = builder.alloc_string("footnote-definition-label");
                let label_props =
                    build_props(builder, &[("className", PROP_SPACE_SEP, label_class_ref)]);
                open_element_with_props(builder, "sup", &label_props);
                let num_str = number.to_string();
                add_text_node(builder, &num_str);
                builder.close_node(); // sup
                convert_children(node_id, view, builder, ctx);
                builder.close_node(); // div
            }
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
            // Directives have no standard HAST representation.
            // Like remark-rehype, skip unless a handler is registered.
        }

        _ => {
            // Unknown: recurse into children
            convert_children(node_id, view, builder, ctx);
        }
    }
}

fn convert_children(
    node_id: u32,
    view: &Arena,
    builder: &mut ArenaBuilder,
    ctx: &ConvertCtx<'_, '_>,
) {
    let children = view.get_children(node_id);
    for &child_id in children {
        convert_node(child_id, view, builder, ctx);
    }
}

/// Convert children with `\n` text nodes between them.
/// Matches `remark-rehype`'s behavior for block containers.
fn convert_children_with_newlines(
    node_id: u32,
    view: &Arena,
    builder: &mut ArenaBuilder,
    ctx: &ConvertCtx<'_, '_>,
) {
    let children = view.get_children(node_id);
    add_text_node(builder, "\n");
    if children.is_empty() {
        return;
    }
    for &child_id in children {
        convert_node(child_id, view, builder, ctx);
        add_text_node(builder, "\n");
    }
}

/// Convert children, unwrapping paragraphs (emitting their children directly).
/// Used for tight list items where the MDAST has paragraph wrappers but
/// HTML output should not have `<p>` tags.
fn convert_children_unwrap_paragraphs(
    node_id: u32,
    view: &Arena,
    builder: &mut ArenaBuilder,
    ctx: &ConvertCtx<'_, '_>,
) {
    let children = view.get_children(node_id);
    for &child_id in children {
        let child = view.get_node(child_id);
        if MdastNodeType::from_u8(child.node_type) == Some(MdastNodeType::Paragraph) {
            convert_children(child_id, view, builder, ctx);
        } else {
            add_text_node(builder, "\n");
            convert_node(child_id, view, builder, ctx);
            add_text_node(builder, "\n");
        }
    }
}

fn emit_checkbox(builder: &mut ArenaBuilder, item_data: ListItemData) {
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
    view: &Arena,
    builder: &mut ArenaBuilder,
    ctx: &ConvertCtx<'_, '_>,
    task: Option<ListItemData>,
) {
    let children = view.get_children(node_id);
    if children.is_empty() {
        if let Some(td) = task {
            add_text_node(builder, "\n");
            open_element(builder, "p");
            copy_position(node_id, view, builder);
            emit_checkbox(builder, td);
            builder.close_node();
            add_text_node(builder, "\n");
        }
        return;
    }
    add_text_node(builder, "\n");
    let mut first = true;
    for &child_id in children {
        let child_node = view.get_node(child_id);
        let is_para =
            MdastNodeType::from_u8(child_node.node_type) == Some(MdastNodeType::Paragraph);
        if first && is_para && task.is_some() {
            let td = task.unwrap();
            open_element(builder, "p");
            copy_position(child_id, view, builder);
            emit_checkbox(builder, td);
            convert_children(child_id, view, builder, ctx);
            builder.close_node();
        } else if first && task.is_some() {
            emit_checkbox(builder, task.unwrap());
            convert_node(child_id, view, builder, ctx);
        } else {
            convert_node(child_id, view, builder, ctx);
        }
        add_text_node(builder, "\n");
        first = false;
    }
}

fn convert_children_unwrap_paragraphs_task(
    node_id: u32,
    view: &Arena,
    builder: &mut ArenaBuilder,
    ctx: &ConvertCtx<'_, '_>,
    task: Option<ListItemData>,
) {
    let children = view.get_children(node_id);
    let mut first = true;
    for &child_id in children {
        let child = view.get_node(child_id);
        if MdastNodeType::from_u8(child.node_type) == Some(MdastNodeType::Paragraph) {
            if first && task.is_some() {
                emit_checkbox(builder, task.unwrap());
            }
            convert_children(child_id, view, builder, ctx);
        } else {
            add_text_node(builder, "\n");
            if first && task.is_some() {
                emit_checkbox(builder, task.unwrap());
            }
            convert_node(child_id, view, builder, ctx);
            add_text_node(builder, "\n");
        }
        first = false;
    }
}

/// Convert children with `\n` text nodes inserted between them.
/// These are needed by the MDX compilation path (JSX children spacing).
/// The HTML renderer skips whitespace-only text nodes between block elements.
fn produces_hast_output(child_id: u32, view: &Arena) -> bool {
    let raw_type = view.get_node(child_id).node_type;
    !matches!(
        MdastNodeType::from_u8(raw_type),
        Some(MdastNodeType::Definition | MdastNodeType::Yaml | MdastNodeType::Toml)
    )
}

fn convert_children_wrapped(
    node_id: u32,
    view: &Arena,
    builder: &mut ArenaBuilder,
    ctx: &ConvertCtx<'_, '_>,
) {
    let children = view.get_children(node_id);
    let mut has_output = false;
    for &child_id in children {
        if produces_hast_output(child_id, view) {
            if has_output {
                add_text_node(builder, "\n");
            }
            has_output = true;
        }
        convert_node(child_id, view, builder, ctx);
    }
}

fn convert_table_row(
    row_id: u32,
    view: &Arena,
    builder: &mut ArenaBuilder,
    ctx: &ConvertCtx<'_, '_>,
    is_header: bool,
    alignments: &[ColumnAlign],
) {
    open_element(builder, "tr");
    copy_position(row_id, view, builder);
    add_text_node(builder, "\n");
    let cell_ids = view.get_children(row_id);
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
        add_text_node(builder, "\n");
    }
    builder.close_node(); // tr
}

/// Check if a paragraph contains only MDX nodes and/or whitespace text.
/// If so, the paragraph should be "unraveled" (children output without `<p>`).
fn is_mdx_only_paragraph(node_id: u32, view: &Arena) -> bool {
    let children = view.get_children(node_id);
    if children.is_empty() {
        return false;
    }

    let mut has_mdx = false;
    for &child_id in children {
        let child = view.get_node(child_id);
        match MdastNodeType::from_u8(child.node_type) {
            Some(
                MdastNodeType::MdxJsxFlowElement
                | MdastNodeType::MdxJsxTextElement
                | MdastNodeType::MdxFlowExpression
                | MdastNodeType::MdxTextExpression,
            ) => {
                has_mdx = true;
            }
            Some(MdastNodeType::Text) => {
                // Only allow whitespace-only text
                let data = view.get_type_data(child_id);
                if !data.is_empty() {
                    let sr = decode_string_ref_data(data);
                    let text = view.get_str(sr);
                    if !text.chars().all(|c| c.is_ascii_whitespace()) {
                        return false;
                    }
                }
            }
            _ => return false,
        }
    }

    has_mdx
}

fn convert_mdx_jsx_element(
    node_id: u32,
    view: &Arena,
    builder: &mut ArenaBuilder,
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
    let mut attr_tuples = Vec::with_capacity(attr_count as usize);
    for i in 0..attr_count {
        let (kind, attr_name_ref, attr_value_ref) = decode_mdx_jsx_attr(mdast_data, i);
        let n = if attr_name_ref.len > 0 {
            builder.alloc_string(view.get_str(attr_name_ref))
        } else {
            StringRef::empty()
        };
        let v = if attr_value_ref.len > 0 {
            builder.alloc_string(view.get_str(attr_value_ref))
        } else {
            StringRef::empty()
        };
        attr_tuples.push((kind, n, v));
    }

    builder.open_node_raw(hast_type);
    let encoded = encode_mdx_jsx_element_data(name_ref, &attr_tuples);
    builder.set_data_current(&encoded);
    copy_position(node_id, view, builder);

    convert_children(node_id, view, builder, ctx);
    builder.close_node();
}

fn extract_text_content(node_id: u32, view: &Arena) -> String {
    let mut out = String::new();
    extract_text_recursive(node_id, view, &mut out);
    out
}

fn extract_text_recursive(node_id: u32, view: &Arena, out: &mut String) {
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
}
