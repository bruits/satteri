//! MDAST → HAST conversion.

use tryckeri_mdast::codec::{
    decode_code_data, decode_expression_data, decode_heading_data, decode_image_data,
    decode_link_data, decode_list_data, decode_list_item_data, decode_mdx_jsx_element_name,
    decode_string_ref_data,
};
use tryckeri_mdast::MdastArena;
use tryckeri_mdast::MdastNodeType;

use crate::node::{HastArena, HastBuilder, HastNodeType, Property, PropertyValue};

pub fn mdast_to_hast(arena: &MdastArena) -> HastArena {
    let mut builder = HastBuilder::new();
    builder.open_root();

    // Insert \n text nodes between root children (matching the binary path's
    // wrap behavior and the hast spec for inter-block whitespace).
    let root_children = arena.get_children(0);
    let mut first = true;
    for &child_id in root_children {
        let child = arena.get_node(child_id);
        let is_unraveled = MdastNodeType::from_u8(child.node_type)
            == Some(MdastNodeType::Paragraph)
            && is_mdx_only_paragraph(child_id, arena);

        if is_unraveled {
            let para_children = arena.get_children(child_id);
            for &para_child_id in para_children {
                if !first {
                    builder.add_text("\n");
                }
                first = false;
                convert_node(para_child_id, arena, &mut builder);
            }
        } else {
            if !first {
                builder.add_text("\n");
            }
            first = false;
            convert_node(child_id, arena, &mut builder);
        }
    }

    builder.finish()
}

fn convert_node(node_id: u32, arena: &MdastArena, builder: &mut HastBuilder) {
    let node = arena.get_node(node_id);
    let node_type = match MdastNodeType::from_u8(node.node_type) {
        Some(t) => t,
        None => return,
    };

    match node_type {
        MdastNodeType::Paragraph => {
            // MDX paragraph unraveling: if all children are MDX nodes or
            // whitespace text, output children directly without <p>.
            if is_mdx_only_paragraph(node_id, arena) {
                convert_children(node_id, arena, builder);
            } else {
                builder.open_element("p");
                convert_children(node_id, arena, builder);
                builder.close();
            }
        }

        MdastNodeType::Heading => {
            let data = arena.get_type_data(node_id);
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
            builder.open_element(tag);
            convert_children(node_id, arena, builder);
            builder.close();
        }

        MdastNodeType::ThematicBreak => {
            builder.open_element("hr");
            builder.close();
        }

        MdastNodeType::Blockquote => {
            builder.open_element("blockquote");
            convert_children(node_id, arena, builder);
            builder.close();
        }

        MdastNodeType::List => {
            let data = arena.get_type_data(node_id);
            let list_data = decode_list_data(data);
            let tag = if list_data.ordered { "ol" } else { "ul" };
            let elem_id = builder.open_element(tag);

            if list_data.ordered && list_data.start != 1 {
                let start_str = list_data.start.to_string();
                let name_ref = builder.alloc_string("start");
                let val_ref = builder.alloc_string(&start_str);
                builder.set_properties(
                    elem_id,
                    &[Property {
                        name: name_ref,
                        value: PropertyValue::String(val_ref),
                    }],
                );
            }

            convert_children(node_id, arena, builder);
            builder.close();
        }

        MdastNodeType::ListItem => {
            builder.open_element("li");
            let data = arena.get_type_data(node_id);
            if !data.is_empty() {
                let item_data = decode_list_item_data(data);
                if item_data.checked != 2 {
                    let checkbox_id = builder.open_element("input");
                    let type_name = builder.alloc_string("type");
                    let type_val = builder.alloc_string("checkbox");
                    let disabled_name = builder.alloc_string("disabled");

                    if item_data.checked == 1 {
                        let checked_name = builder.alloc_string("checked");
                        builder.set_properties(
                            checkbox_id,
                            &[
                                Property {
                                    name: type_name,
                                    value: PropertyValue::String(type_val),
                                },
                                Property {
                                    name: disabled_name,
                                    value: PropertyValue::Bool(true),
                                },
                                Property {
                                    name: checked_name,
                                    value: PropertyValue::Bool(true),
                                },
                            ],
                        );
                    } else {
                        builder.set_properties(
                            checkbox_id,
                            &[
                                Property {
                                    name: type_name,
                                    value: PropertyValue::String(type_val),
                                },
                                Property {
                                    name: disabled_name,
                                    value: PropertyValue::Bool(true),
                                },
                            ],
                        );
                    }
                    builder.close(); // input
                }
            }
            convert_children(node_id, arena, builder);
            builder.close();
        }

        MdastNodeType::Html => {
            let data = arena.get_type_data(node_id);
            let string_ref = decode_string_ref_data(data);
            let html = arena.get_str(string_ref);
            builder.add_raw(html);
        }

        MdastNodeType::Code => {
            let data = arena.get_type_data(node_id);
            let code_data = decode_code_data(data);

            builder.open_element("pre");
            let code_id = builder.open_element("code");

            if code_data.lang.len > 0 {
                let lang = arena.get_str(code_data.lang);
                let class_value = format!("language-{}", lang);
                let class_name = builder.alloc_string("class");
                let class_ref = builder.alloc_string(&class_value);
                builder.set_properties(
                    code_id,
                    &[Property {
                        name: class_name,
                        value: PropertyValue::SpaceSeparated(class_ref),
                    }],
                );
            }

            let value = arena.get_str(code_data.value);
            builder.add_text(value);
            builder.close(); // code
            builder.close(); // pre
        }

        MdastNodeType::Definition => {
            // Definitions don't produce HAST output
        }

        MdastNodeType::Text => {
            let data = arena.get_type_data(node_id);
            let string_ref = decode_string_ref_data(data);
            let text = arena.get_str(string_ref);
            builder.add_text(text);
        }

        MdastNodeType::Emphasis => {
            builder.open_element("em");
            convert_children(node_id, arena, builder);
            builder.close();
        }

        MdastNodeType::Strong => {
            builder.open_element("strong");
            convert_children(node_id, arena, builder);
            builder.close();
        }

        MdastNodeType::InlineCode => {
            let data = arena.get_type_data(node_id);
            let string_ref = decode_string_ref_data(data);
            let code = arena.get_str(string_ref);
            builder.open_element("code");
            builder.add_text(code);
            builder.close();
        }

        MdastNodeType::Break => {
            builder.open_element("br");
            builder.close();
        }

        MdastNodeType::Link => {
            let data = arena.get_type_data(node_id);
            let link_data = decode_link_data(data);
            let url = arena.get_str(link_data.url);

            let elem_id = builder.open_element("a");
            let href_name = builder.alloc_string("href");
            let href_val = builder.alloc_string(url);

            if link_data.title.len > 0 {
                let title = arena.get_str(link_data.title);
                let title_name = builder.alloc_string("title");
                let title_val = builder.alloc_string(title);
                builder.set_properties(
                    elem_id,
                    &[
                        Property {
                            name: href_name,
                            value: PropertyValue::String(href_val),
                        },
                        Property {
                            name: title_name,
                            value: PropertyValue::String(title_val),
                        },
                    ],
                );
            } else {
                builder.set_properties(
                    elem_id,
                    &[Property {
                        name: href_name,
                        value: PropertyValue::String(href_val),
                    }],
                );
            }
            convert_children(node_id, arena, builder);
            builder.close();
        }

        MdastNodeType::Image => {
            let data = arena.get_type_data(node_id);
            let img_data = decode_image_data(data);
            let url = arena.get_str(img_data.url);
            let alt = arena.get_str(img_data.alt);

            let elem_id = builder.open_element("img");
            let src_name = builder.alloc_string("src");
            let src_val = builder.alloc_string(url);
            let alt_name = builder.alloc_string("alt");
            let alt_val = builder.alloc_string(alt);

            if img_data.title.len > 0 {
                let title = arena.get_str(img_data.title);
                let title_name = builder.alloc_string("title");
                let title_val = builder.alloc_string(title);
                builder.set_properties(
                    elem_id,
                    &[
                        Property {
                            name: src_name,
                            value: PropertyValue::String(src_val),
                        },
                        Property {
                            name: alt_name,
                            value: PropertyValue::String(alt_val),
                        },
                        Property {
                            name: title_name,
                            value: PropertyValue::String(title_val),
                        },
                    ],
                );
            } else {
                builder.set_properties(
                    elem_id,
                    &[
                        Property {
                            name: src_name,
                            value: PropertyValue::String(src_val),
                        },
                        Property {
                            name: alt_name,
                            value: PropertyValue::String(alt_val),
                        },
                    ],
                );
            }
            builder.close();
        }

        MdastNodeType::Table => {
            builder.open_element("table");
            let child_ids = arena.get_children(node_id);
            let child_count = child_ids.len();
            if child_count > 0 {
                let first_row = child_ids[0];
                // Copy remaining row IDs before mutating builder
                let body_row_ids: Vec<u32> = if child_count > 1 {
                    child_ids[1..].to_vec()
                } else {
                    Vec::new()
                };

                builder.open_element("thead");
                convert_table_row(first_row, arena, builder, true);
                builder.close(); // thead

                if !body_row_ids.is_empty() {
                    builder.open_element("tbody");
                    for row_id in body_row_ids {
                        convert_table_row(row_id, arena, builder, false);
                    }
                    builder.close(); // tbody
                }
            }
            builder.close(); // table
        }

        MdastNodeType::Delete => {
            builder.open_element("del");
            convert_children(node_id, arena, builder);
            builder.close();
        }

        MdastNodeType::FootnoteDefinition
        | MdastNodeType::FootnoteReference
        | MdastNodeType::LinkReference
        | MdastNodeType::ImageReference => {
            // Skip for Phase 7 — reference resolution requires a pre-pass
        }

        MdastNodeType::MdxJsxFlowElement => {
            convert_mdx_jsx_element(node_id, arena, builder, HastNodeType::MdxJsxElement);
        }
        MdastNodeType::MdxJsxTextElement => {
            convert_mdx_jsx_element(node_id, arena, builder, HastNodeType::MdxJsxTextElement);
        }

        MdastNodeType::MdxFlowExpression | MdastNodeType::MdxTextExpression => {
            let data = arena.get_type_data(node_id);
            let value = if data.is_empty() {
                ""
            } else {
                let d = decode_expression_data(data);
                arena.get_str(d.value)
            };
            let id = builder.add_mdx_value_node(HastNodeType::MdxExpression, value);
            let _ = id;
        }

        MdastNodeType::MdxjsEsm => {
            let data = arena.get_type_data(node_id);
            let value = if data.is_empty() {
                ""
            } else {
                let d = decode_expression_data(data);
                arena.get_str(d.value)
            };
            let id = builder.add_mdx_value_node(HastNodeType::MdxEsm, value);
            let _ = id;
        }

        _ => {
            // Unknown or unhandled: recurse into children
            convert_children(node_id, arena, builder);
        }
    }
}

fn convert_mdx_jsx_element(
    node_id: u32,
    arena: &MdastArena,
    builder: &mut HastBuilder,
    hast_type: HastNodeType,
) {
    let data = arena.get_type_data(node_id);
    let name = if data.is_empty() {
        ""
    } else {
        let name_ref = decode_mdx_jsx_element_name(data);
        if name_ref.len > 0 {
            arena.get_str(name_ref)
        } else {
            ""
        }
    };

    let id = builder.open_mdx_jsx_element(hast_type, name);
    let _ = id;
    convert_children(node_id, arena, builder);
    builder.close();
}

fn is_mdx_only_paragraph(node_id: u32, arena: &MdastArena) -> bool {
    let children = arena.get_children(node_id);
    if children.is_empty() {
        return false;
    }

    let mut has_mdx = false;
    for &child_id in children {
        let child = arena.get_node(child_id);
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
                let data = arena.get_type_data(child_id);
                if !data.is_empty() {
                    let sr = decode_string_ref_data(data);
                    let text = arena.get_str(sr);
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

fn convert_children(node_id: u32, arena: &MdastArena, builder: &mut HastBuilder) {
    let children = arena.get_children(node_id);
    for &child_id in children {
        convert_node(child_id, arena, builder);
    }
}

fn convert_table_row(row_id: u32, arena: &MdastArena, builder: &mut HastBuilder, is_header: bool) {
    builder.open_element("tr");
    let cell_ids = arena.get_children(row_id);
    let cell_tag = if is_header { "th" } else { "td" };
    for &cell_id in cell_ids {
        builder.open_element(cell_tag);
        convert_children(cell_id, arena, builder);
        builder.close();
    }
    builder.close(); // tr
}
