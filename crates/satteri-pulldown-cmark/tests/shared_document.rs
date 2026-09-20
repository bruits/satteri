use satteri_pulldown_cmark::{DEFAULT_OPTIONS, Options, document};

#[test]
fn direct_and_materialized_readers_agree_on_commonmark_and_extensions() {
    let commonmark: serde_json::Value =
        serde_json::from_str(include_str!("../third_party/CommonMark/spec.json")).unwrap();
    let mut sources: Vec<&str> = commonmark
        .as_array()
        .unwrap()
        .iter()
        .map(|example| example["markdown"].as_str().unwrap())
        .collect();
    sources.extend([
        "# Unicode 雪😀\n\n**strong** and [link](target)\nnext",
        "- [x] task[^a]\n\n[^a]: footnote **body**\n",
        "|a|b|\n|:-|-:|\n|&amp;|雪|\n",
        "---\ntitle: heading\n---\n\n[x]: /target\n\n[x] www.example.com",
        ":::note\nbody\n:::\n",
        "Term 1\nTerm 2\n: definition\n",
    ]);
    for source in sources {
        for options in [Options::empty(), DEFAULT_OPTIONS] {
            for positions in [false, true] {
                let (document, errors) = document::parse(source, options, positions);
                assert!(errors.is_empty());
                let arena = document.materialize();
                assert_eq!(
                    satteri_ast::mdast_to_html(&document),
                    satteri_ast::mdast_to_html(&arena),
                    "{source:?}"
                );
                let direct = satteri_ast::hast::mdast_arena_to_hast_arena(&document);
                let materialized = satteri_ast::hast::mdast_arena_to_hast_arena(&arena);
                assert_eq!(
                    direct.to_raw_buffer(),
                    materialized.to_raw_buffer(),
                    "{source:?}"
                );
            }
        }
    }
}

#[test]
fn nested_ancestors_keep_source_positions() {
    for depth in [32, 128, 512] {
        let source = "> ".repeat(depth) + "leaf";
        let (document, _) = document::parse(&source, DEFAULT_OPTIONS, true);
        let arena = document.materialize();
        let mut id = 0;
        for _ in 0..depth + 2 {
            assert_eq!(arena.get_children(id).len(), 1);
            id = arena.get_children(id)[0];
        }
        let leaf = arena.get_node(id);
        assert_eq!(leaf.start_offset as usize, depth * 2);
        assert_eq!(leaf.end_offset as usize, source.len());
        assert_eq!(leaf.start_column as usize, depth * 2 + 1);
        assert_eq!(leaf.end_column as usize, source.len() + 1);
        assert_eq!(
            satteri_ast::mdast_to_html(&document),
            satteri_ast::mdast_to_html(&arena)
        );
    }
}

#[test]
fn recycled_storage_clears_source_metadata_and_position_modes() {
    let mut storage = None;
    for _ in 0..3 {
        for source in [
            "---\ntitle: 雪\n---\n\n# Heading",
            "[^x]: note\n\ntext[^x]",
            "small",
            "",
            "**😀**\nnext",
        ] {
            for positions in [true, false] {
                let (fresh, fresh_errors) = document::parse(source, DEFAULT_OPTIONS, positions);
                let (reused, errors) =
                    document::parse_reusing(source, DEFAULT_OPTIONS, positions, storage);
                assert_eq!(errors, fresh_errors);
                assert_eq!(
                    reused.materialize().to_raw_buffer(),
                    fresh.materialize().to_raw_buffer()
                );
                assert_eq!(
                    satteri_ast::mdast_to_html(&reused),
                    satteri_ast::mdast_to_html(&fresh)
                );
                storage = Some(reused.into_reusable());
            }
        }
    }
}

#[test]
fn early_strong_keeps_attention_family_registration_order() {
    for (source, expected) in [
        (
            "**word** ~a*b~*",
            "<p><strong>word</strong> ~a<em>b~</em></p>\n",
        ),
        (
            "**word** ~_~:_<",
            "<p><strong>word</strong> ~<em>~:</em>&lt;</p>\n",
        ),
    ] {
        let (document, errors) = document::parse(source, Options::from_bits_truncate(3230), false);
        assert!(errors.is_empty());
        assert_eq!(satteri_ast::mdast_to_html(&document), expected);
    }
}

#[test]
fn owning_the_document_preserves_wire_and_values_after_releasing_source() {
    let source = String::from("hello &amp;");
    let (document, _) = document::parse(&source, DEFAULT_OPTIONS, true);
    let expected = document.materialize().to_raw_buffer();
    let arena = document.into_materialized();
    drop(source);
    assert_eq!(arena.to_raw_buffer(), expected);
    assert_eq!(satteri_ast::mdast_to_html(&arena), "<p>hello &amp;</p>\n");
}

#[test]
fn source_code_values_survive_owning_and_recycling() {
    use satteri_ast::mdast::{MdastNodeType, decode_string_ref_data};
    use satteri_pulldown_cmark::{Event, Parser};
    let mut storage = None;
    for _ in 0..3 {
        for (source, expected) in [
            ("`code`", "code"),
            ("`` x ``", "x"),
            ("`   `", "   "),
            ("`雪😀`", "雪😀"),
            ("`a\\|b`", "a\\|b"),
            ("`a\nb`", "a\nb"),
            ("`a\r\nb`", "a\r\nb"),
        ] {
            let events: Vec<_> = Parser::new_ext(source, Options::empty())
                .filter_map(|event| match event {
                    Event::Code(value) => Some(value.into_string()),
                    _ => None,
                })
                .collect();
            assert_eq!(events, [expected], "{source:?}");
            for positions in [false, true] {
                let owned_source = source.to_string();
                let (document, errors) = document::parse_reusing(
                    &owned_source,
                    DEFAULT_OPTIONS,
                    positions,
                    storage.take(),
                );
                assert!(errors.is_empty());
                let id = (0..document.len() as u32)
                    .find(|&id| document.get_node(id).node_type == MdastNodeType::InlineCode as u8)
                    .unwrap();
                let value = decode_string_ref_data(document.get_type_data(id));
                assert_eq!(document.get_str(value), expected);
                // Owning must keep references valid after releasing the input.
                let arena = document.materialize();
                storage = Some(document.into_reusable());
                drop(owned_source);
                assert_eq!(
                    arena.get_str(decode_string_ref_data(arena.get_type_data(id))),
                    expected
                );
            }
        }
    }
}

#[test]
fn compact_strong_preserves_image_alt_and_positions() {
    use satteri_ast::mdast::{MdastNodeType, decode_string_ref_data};
    let source = "雪 **two\twords** ![**alt**](x)";
    let (document, errors) = document::parse(source, DEFAULT_OPTIONS, true);
    assert!(errors.is_empty());
    assert_eq!(
        satteri_ast::mdast_to_html(&document),
        "<p>雪 <strong>two\twords</strong> <img src=\"x\" alt=\"alt\"></p>\n"
    );
    let strong = (0..document.len() as u32)
        .find(|&id| document.get_node(id).node_type == MdastNodeType::Strong as u8)
        .unwrap();
    let children = document.get_children(strong);
    assert_eq!(children.len(), 1);
    let child = document.get_node(children[0]);
    let value = decode_string_ref_data(document.get_type_data(child.id));
    assert_eq!(document.get_str(value), "two\twords");
    assert_eq!(
        &source[child.start_offset as usize..child.end_offset as usize],
        "two\twords"
    );
    assert_eq!(
        (
            child.start_line,
            child.start_column,
            child.end_line,
            child.end_column
        ),
        (1, 5, 1, 14)
    );
}

#[test]
fn owned_arena_reuse_preserves_wire_when_sources_and_position_modes_change() {
    use satteri_arena::{Arena, Mdast};
    use satteri_pulldown_cmark::{parse, parse_into, parse_no_positions, parse_no_positions_into};
    let large = "**word** `code` [link](target)\n\n".repeat(1000);
    let mut reused = Arena::<Mdast>::new(String::new());
    for source in [large.as_str(), "雪😀", "", "---\ntitle: x\n---\n\nsmall"] {
        for positions in [true, false] {
            let (fresh, expected_errors) = if positions {
                parse(source, DEFAULT_OPTIONS)
            } else {
                parse_no_positions(source, DEFAULT_OPTIONS)
            };
            let (next, errors) = if positions {
                parse_into(source, DEFAULT_OPTIONS, reused)
            } else {
                parse_no_positions_into(source, DEFAULT_OPTIONS, reused)
            };
            assert_eq!(errors, expected_errors);
            assert_eq!(next.to_raw_buffer(), fresh.to_raw_buffer());
            reused = next;
        }
    }
}
