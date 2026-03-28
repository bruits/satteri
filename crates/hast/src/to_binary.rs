//! Convert an MDAST binary buffer to a HAST binary buffer.

use tryckeri_mdast::{
    decode_code_data, decode_definition_data, decode_expression_data, decode_heading_data,
    decode_image_data, decode_link_data, decode_list_data, decode_list_item_data, decode_math_data,
    decode_mdx_jsx_attr, decode_mdx_jsx_attr_count, decode_mdx_jsx_element_name,
    decode_reference_data, decode_string_ref_data, BufferError, MdastArena, MdastBuilder,
    MdastNodeType, MdastView, StringRef,
};

use crate::codec::{encode_mdx_jsx_element_data, encode_text_data};
use crate::node_types::*;

pub fn mdast_to_hast_buffer(mdast_buf: &[u8]) -> Result<Vec<u8>, BufferError> {
    let view = MdastArena::from_raw_buffer(mdast_buf)?;
    let mut builder = MdastBuilder::new(String::new());

    // Pre-pass: collect definitions for link/image reference resolution.
    let defs = collect_definitions(&view);

    convert_node(0, &view, &mut builder, &defs);

    let hast_arena = builder.finish();
    Ok(hast_arena.to_raw_buffer())
}

struct Definition {
    identifier: String,
    url: String,
    title: Option<String>,
}

fn collect_definitions(view: &MdastView) -> Vec<Definition> {
    let mut defs = Vec::new();
    for id in 0..view.node_count() {
        let node = view.get_node(id);
        if node.node_type == MdastNodeType::Definition as u8 {
            let data = view.get_type_data(id);
            if data.len() >= 32 {
                let dd = decode_definition_data(data);
                let identifier = view.get_str(dd.identifier).to_string();
                let url = view.get_str(dd.url).to_string();
                let title = if dd.title.len > 0 {
                    Some(view.get_str(dd.title).to_string())
                } else {
                    None
                };
                defs.push(Definition {
                    identifier,
                    url,
                    title,
                });
            }
        }
    }
    defs
}

fn find_def<'a>(defs: &'a [Definition], identifier: &str) -> Option<&'a Definition> {
    defs.iter().find(|d| d.identifier == identifier)
}

/// Pre-built property data: refs already interned in the builder's string pool.
struct PropData {
    name_ref: StringRef,
    value_kind: u8,
    value_ref: StringRef,
}

fn build_props(builder: &mut MdastBuilder, specs: &[(&str, u8, StringRef)]) -> Vec<PropData> {
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

fn open_element(builder: &mut MdastBuilder, tag: &str) {
    builder.open_node_raw(HAST_ELEMENT);
    let tag_ref = builder.alloc_string(tag);
    let encoded = crate::codec::encode_element_data(tag_ref, &[]);
    builder.set_data_current(&encoded);
}

fn open_element_with_props(builder: &mut MdastBuilder, tag: &str, props: &[PropData]) {
    builder.open_node_raw(HAST_ELEMENT);
    let tag_ref = builder.alloc_string(tag);
    let prop_tuples: Vec<(StringRef, u8, StringRef)> = props
        .iter()
        .map(|p| (p.name_ref, p.value_kind, p.value_ref))
        .collect();
    let encoded = crate::codec::encode_element_data(tag_ref, &prop_tuples);
    builder.set_data_current(&encoded);
}

fn add_void_element(builder: &mut MdastBuilder, tag: &str) {
    builder.open_node_raw(HAST_ELEMENT);
    let tag_ref = builder.alloc_string(tag);
    let encoded = crate::codec::encode_element_data(tag_ref, &[]);
    builder.set_data_current(&encoded);
    builder.close_node();
}

fn add_void_element_with_props(builder: &mut MdastBuilder, tag: &str, props: &[PropData]) {
    builder.open_node_raw(HAST_ELEMENT);
    let tag_ref = builder.alloc_string(tag);
    let prop_tuples: Vec<(StringRef, u8, StringRef)> = props
        .iter()
        .map(|p| (p.name_ref, p.value_kind, p.value_ref))
        .collect();
    let encoded = crate::codec::encode_element_data(tag_ref, &prop_tuples);
    builder.set_data_current(&encoded);
    builder.close_node();
}

fn add_text_node(builder: &mut MdastBuilder, text: &str) {
    let text_ref = builder.alloc_string(text);
    let leaf_id = builder.add_leaf_raw(HAST_TEXT);
    builder
        .arena_mut()
        .set_type_data(leaf_id, &encode_text_data(text_ref));
}

fn add_raw_node(builder: &mut MdastBuilder, html: &str) {
    let html_ref = builder.alloc_string(html);
    let leaf_id = builder.add_leaf_raw(HAST_RAW);
    builder
        .arena_mut()
        .set_type_data(leaf_id, &encode_text_data(html_ref));
}

fn copy_position(node_id: u32, view: &MdastView, builder: &mut MdastBuilder) {
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

fn convert_node(node_id: u32, view: &MdastView, builder: &mut MdastBuilder, defs: &[Definition]) {
    let node = view.get_node(node_id);
    let raw_type = node.node_type;

    match MdastNodeType::from_u8(raw_type) {
        Some(MdastNodeType::Root) => {
            builder.open_node_raw(HAST_ROOT);
            convert_children_wrapped(node_id, view, builder, defs);
            builder.close_node();
        }

        Some(MdastNodeType::Paragraph) => {
            // Note: MDX paragraph unraveling is handled by convert_children_wrapped
            // at the parent level, so by the time we get here it's a normal <p>.
            open_element(builder, "p");
            copy_position(node_id, view, builder);
            convert_children(node_id, view, builder, defs);
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
            convert_children(node_id, view, builder, defs);
            builder.close_node();
        }

        Some(MdastNodeType::ThematicBreak) => {
            add_void_element(builder, "hr");
        }

        Some(MdastNodeType::Blockquote) => {
            open_element(builder, "blockquote");
            convert_children(node_id, view, builder, defs);
            builder.close_node();
        }

        Some(MdastNodeType::List) => {
            let data = view.get_type_data(node_id);
            let list_data = decode_list_data(data);
            let tag = if list_data.ordered { "ol" } else { "ul" };
            if list_data.ordered && list_data.start != 1 {
                let start_str = list_data.start.to_string();
                let start_ref = builder.alloc_string(&start_str);
                let props = build_props(builder, &[("start", PROP_STRING, start_ref)]);
                open_element_with_props(builder, tag, &props);
            } else {
                open_element(builder, tag);
            }
            convert_children(node_id, view, builder, defs);
            builder.close_node();
        }

        Some(MdastNodeType::ListItem) => {
            open_element(builder, "li");
            let data = view.get_type_data(node_id);
            if !data.is_empty() {
                let item_data = decode_list_item_data(data);
                if item_data.checked != 2 {
                    // Task list item — add disabled checkbox
                    let type_ref = builder.alloc_string("checkbox");
                    if item_data.checked == 1 {
                        let props = build_props(
                            builder,
                            &[
                                ("type", PROP_STRING, type_ref),
                                ("disabled", PROP_BOOL_TRUE, StringRef::empty()),
                                ("checked", PROP_BOOL_TRUE, StringRef::empty()),
                            ],
                        );
                        add_void_element_with_props(builder, "input", &props);
                    } else {
                        let props = build_props(
                            builder,
                            &[
                                ("type", PROP_STRING, type_ref),
                                ("disabled", PROP_BOOL_TRUE, StringRef::empty()),
                            ],
                        );
                        add_void_element_with_props(builder, "input", &props);
                    }
                }
            }
            convert_children(node_id, view, builder, defs);
            builder.close_node();
        }

        Some(MdastNodeType::Html) => {
            let data = view.get_type_data(node_id);
            let string_ref = decode_string_ref_data(data);
            add_raw_node(builder, view.get_str(string_ref));
        }

        Some(MdastNodeType::Code) => {
            let data = view.get_type_data(node_id);
            let code_data = decode_code_data(data);
            let value = view.get_str(code_data.value);

            open_element(builder, "pre");
            if code_data.lang.len > 0 {
                let lang = view.get_str(code_data.lang);
                let class_val = format!("language-{}", lang);
                let class_ref = builder.alloc_string(&class_val);
                let props = build_props(builder, &[("class", PROP_SPACE_SEP, class_ref)]);
                open_element_with_props(builder, "code", &props);
            } else {
                open_element(builder, "code");
            }
            add_text_node(builder, value);
            builder.close_node(); // code
            builder.close_node(); // pre
        }

        Some(MdastNodeType::Text) => {
            let data = view.get_type_data(node_id);
            let string_ref = decode_string_ref_data(data);
            add_text_node(builder, view.get_str(string_ref));
        }

        Some(MdastNodeType::Emphasis) => {
            open_element(builder, "em");
            convert_children(node_id, view, builder, defs);
            builder.close_node();
        }

        Some(MdastNodeType::Strong) => {
            open_element(builder, "strong");
            convert_children(node_id, view, builder, defs);
            builder.close_node();
        }

        Some(MdastNodeType::InlineCode) => {
            let data = view.get_type_data(node_id);
            let string_ref = decode_string_ref_data(data);
            open_element(builder, "code");
            add_text_node(builder, view.get_str(string_ref));
            builder.close_node();
        }

        Some(MdastNodeType::Break) => {
            add_void_element(builder, "br");
        }

        Some(MdastNodeType::Link) => {
            let data = view.get_type_data(node_id);
            let link_data = decode_link_data(data);
            let url_ref = builder.alloc_string(view.get_str(link_data.url));
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
            convert_children(node_id, view, builder, defs);
            builder.close_node();
        }

        Some(MdastNodeType::Image) => {
            let data = view.get_type_data(node_id);
            let img_data = decode_image_data(data);
            let url_ref = builder.alloc_string(view.get_str(img_data.url));
            let alt_ref = builder.alloc_string(view.get_str(img_data.alt));
            if img_data.title.len > 0 {
                let title_ref = builder.alloc_string(view.get_str(img_data.title));
                let props = build_props(
                    builder,
                    &[
                        ("src", PROP_STRING, url_ref),
                        ("alt", PROP_STRING, alt_ref),
                        ("title", PROP_STRING, title_ref),
                    ],
                );
                add_void_element_with_props(builder, "img", &props);
            } else {
                let props = build_props(
                    builder,
                    &[("src", PROP_STRING, url_ref), ("alt", PROP_STRING, alt_ref)],
                );
                add_void_element_with_props(builder, "img", &props);
            }
        }

        Some(MdastNodeType::Delete) => {
            open_element(builder, "del");
            convert_children(node_id, view, builder, defs);
            builder.close_node();
        }

        Some(MdastNodeType::Table) => {
            open_element(builder, "table");
            let child_ids = view.get_children(node_id);
            if !child_ids.is_empty() {
                open_element(builder, "thead");
                convert_table_row(child_ids[0], view, builder, defs, true);
                builder.close_node(); // thead

                if child_ids.len() > 1 {
                    open_element(builder, "tbody");
                    for &row_id in &child_ids[1..] {
                        convert_table_row(row_id, view, builder, defs, false);
                    }
                    builder.close_node(); // tbody
                }
            }
            builder.close_node(); // table
        }

        Some(MdastNodeType::Math) => {
            let data = view.get_type_data(node_id);
            let math_data = decode_math_data(data);
            let value = view.get_str(math_data.value);
            let class_ref = builder.alloc_string("language-math math-display");
            let props = build_props(builder, &[("class", PROP_SPACE_SEP, class_ref)]);
            open_element(builder, "pre");
            open_element_with_props(builder, "code", &props);
            add_text_node(builder, value);
            builder.close_node(); // code
            builder.close_node(); // pre
        }

        Some(MdastNodeType::InlineMath) => {
            let data = view.get_type_data(node_id);
            let string_ref = decode_string_ref_data(data);
            let value = view.get_str(string_ref);
            let class_ref = builder.alloc_string("language-math math-inline");
            let props = build_props(builder, &[("class", PROP_SPACE_SEP, class_ref)]);
            open_element_with_props(builder, "code", &props);
            add_text_node(builder, value);
            builder.close_node();
        }

        Some(MdastNodeType::Definition)
        | Some(MdastNodeType::Yaml)
        | Some(MdastNodeType::Toml)
        | Some(MdastNodeType::FootnoteDefinition) => {
            // No HAST output
        }

        Some(MdastNodeType::LinkReference) => {
            let data = view.get_type_data(node_id);
            if data.len() >= 20 {
                let rd = decode_reference_data(data);
                let identifier = view.get_str(rd.identifier);
                if let Some(def) = find_def(defs, identifier) {
                    let url_ref = builder.alloc_string(&def.url);
                    if let Some(ref title) = def.title {
                        let title_ref = builder.alloc_string(title);
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
                    convert_children(node_id, view, builder, defs);
                    builder.close_node();
                } else {
                    // Unresolved: output children as-is
                    convert_children(node_id, view, builder, defs);
                }
            }
        }

        Some(MdastNodeType::ImageReference) => {
            let data = view.get_type_data(node_id);
            if data.len() >= 20 {
                let rd = decode_reference_data(data);
                let identifier = view.get_str(rd.identifier);
                if let Some(def) = find_def(defs, identifier) {
                    let alt = extract_text_content(node_id, view);
                    let url_ref = builder.alloc_string(&def.url);
                    let alt_ref = builder.alloc_string(&alt);
                    if let Some(ref title) = def.title {
                        let title_ref = builder.alloc_string(title);
                        let props = build_props(
                            builder,
                            &[
                                ("src", PROP_STRING, url_ref),
                                ("alt", PROP_STRING, alt_ref),
                                ("title", PROP_STRING, title_ref),
                            ],
                        );
                        add_void_element_with_props(builder, "img", &props);
                    } else {
                        let props = build_props(
                            builder,
                            &[("src", PROP_STRING, url_ref), ("alt", PROP_STRING, alt_ref)],
                        );
                        add_void_element_with_props(builder, "img", &props);
                    }
                }
            }
        }

        Some(MdastNodeType::FootnoteReference) => {
            // Skip for now
        }

        Some(MdastNodeType::MdxJsxFlowElement) => {
            convert_mdx_jsx_element(node_id, view, builder, defs, HAST_MDX_JSX_ELEMENT);
        }
        Some(MdastNodeType::MdxJsxTextElement) => {
            convert_mdx_jsx_element(node_id, view, builder, defs, HAST_MDX_JSX_TEXT_ELEMENT);
        }

        Some(MdastNodeType::MdxFlowExpression) | Some(MdastNodeType::MdxTextExpression) => {
            let data = view.get_type_data(node_id);
            let value = if data.is_empty() {
                ""
            } else {
                let d = decode_expression_data(data);
                view.get_str(d.value)
            };
            let value_ref = builder.alloc_string(value);
            let leaf_id = builder.add_leaf_raw(HAST_MDX_EXPRESSION);
            builder
                .arena_mut()
                .set_type_data(leaf_id, &encode_text_data(value_ref));
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
            let leaf_id = builder.add_leaf_raw(HAST_MDX_ESM);
            builder
                .arena_mut()
                .set_type_data(leaf_id, &encode_text_data(value_ref));
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

        _ => {
            // Unknown: recurse into children
            convert_children(node_id, view, builder, defs);
        }
    }
}

fn convert_children(
    node_id: u32,
    view: &MdastView,
    builder: &mut MdastBuilder,
    defs: &[Definition],
) {
    let children = view.get_children(node_id);
    for &child_id in children {
        convert_node(child_id, view, builder, defs);
    }
}

/// Convert children with `\n` text nodes inserted between them (wrap behavior).
/// Also handles Fragment results from paragraph unraveling by splicing
/// the unraveled children into the parent with `\n` between them.
fn convert_children_wrapped(
    node_id: u32,
    view: &MdastView,
    builder: &mut MdastBuilder,
    defs: &[Definition],
) {
    let children = view.get_children(node_id);
    let mut first = true;
    for &child_id in children {
        let child_node = view.get_node(child_id);
        let is_unraveled_paragraph = MdastNodeType::from_u8(child_node.node_type)
            == Some(MdastNodeType::Paragraph)
            && is_mdx_only_paragraph(child_id, view);

        if is_unraveled_paragraph {
            // Paragraph unraveled — emit its children with \n between them
            let para_children = view.get_children(child_id);
            for &para_child_id in para_children {
                if !first {
                    add_text_node(builder, "\n");
                }
                first = false;
                convert_node(para_child_id, view, builder, defs);
            }
        } else {
            if !first {
                add_text_node(builder, "\n");
            }
            first = false;
            convert_node(child_id, view, builder, defs);
        }
    }
}

fn convert_table_row(
    row_id: u32,
    view: &MdastView,
    builder: &mut MdastBuilder,
    defs: &[Definition],
    is_header: bool,
) {
    open_element(builder, "tr");
    let cell_ids = view.get_children(row_id);
    let cell_tag = if is_header { "th" } else { "td" };
    for &cell_id in cell_ids {
        open_element(builder, cell_tag);
        convert_children(cell_id, view, builder, defs);
        builder.close_node();
    }
    builder.close_node(); // tr
}

/// Check if a paragraph contains only MDX nodes and/or whitespace text.
/// If so, the paragraph should be "unraveled" (children output without `<p>`).
fn is_mdx_only_paragraph(node_id: u32, view: &MdastView) -> bool {
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
    view: &MdastView,
    builder: &mut MdastBuilder,
    defs: &[Definition],
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

    convert_children(node_id, view, builder, defs);
    builder.close_node();
}

fn extract_text_content(node_id: u32, view: &MdastView) -> String {
    let mut out = String::new();
    extract_text_recursive(node_id, view, &mut out);
    out
}

fn extract_text_recursive(node_id: u32, view: &MdastView, out: &mut String) {
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
