//! Integration tests for `mdast_to_hast`.

extern crate mdxjs;
use mdast_arena::codec::{
    encode_code_data, encode_heading_data, encode_image_data, encode_list_data,
    encode_list_item_data, encode_string_ref_data,
};
use mdast_arena::node::StringRef;
use mdast_arena::{MdastArena, NodeType};
use mdxjs::hast;
use mdxjs::mdast_to_hast;

// ---------------------------------------------------------------------------
// Helper: build a minimal MdastArena manually.
//   Root → Heading(depth=1) → Text("Hello")
// ---------------------------------------------------------------------------

fn build_heading_paragraph_arena() -> MdastArena {
    // We need a source string that contains all sub-strings referenced via
    // StringRef. We concatenate them ourselves.
    let source = "Hello world".to_string();
    let hello_offset = 0u32; // "Hello" starts at 0, len 5
    let world_offset = 6u32; // "world" starts at 6, len 5

    let mut arena = MdastArena::new(source);

    // Root
    let root_id = arena.alloc_node(NodeType::Root);

    // Heading (depth=1)
    let h1_id = arena.alloc_node(NodeType::Heading);
    arena.set_type_data(h1_id, &encode_heading_data(1));

    // Text "Hello" inside heading
    let text_h_id = arena.alloc_node(NodeType::Text);
    let sr_hello = StringRef::new(hello_offset, 5);
    arena.set_type_data(text_h_id, &encode_string_ref_data(sr_hello));

    // Paragraph
    let para_id = arena.alloc_node(NodeType::Paragraph);

    // Text "world" inside paragraph
    let text_w_id = arena.alloc_node(NodeType::Text);
    let sr_world = StringRef::new(world_offset, 5);
    arena.set_type_data(text_w_id, &encode_string_ref_data(sr_world));

    // Wire up children
    arena.set_children(h1_id, &[text_h_id]);
    arena.set_children(para_id, &[text_w_id]);
    arena.set_children(root_id, &[h1_id, para_id]);

    arena
}

// ---------------------------------------------------------------------------
// Test 1: Heading + paragraph → correct hast structure
// ---------------------------------------------------------------------------

#[test]
fn test_heading_and_paragraph() {
    let arena = build_heading_paragraph_arena();
    let result = mdast_to_hast(&arena, None);

    match &result {
        hast::Node::Root(root) => {
            // wrap(children, false) — no leading \n for root, just \n between items
            // children: [h1, \n, p]
            assert_eq!(
                root.children.len(),
                3,
                "root should have 3 children (h1, \\n, p)"
            );

            match &root.children[0] {
                hast::Node::Element(el) => {
                    assert_eq!(el.tag_name, "h1");
                    assert_eq!(el.children.len(), 1);
                    match &el.children[0] {
                        hast::Node::Text(t) => assert_eq!(t.value, "Hello"),
                        other => panic!("expected Text, got {other:?}"),
                    }
                }
                other => panic!("expected Element(h1), got {other:?}"),
            }

            match &root.children[1] {
                hast::Node::Text(t) => assert_eq!(t.value, "\n"),
                other => panic!("expected newline text, got {other:?}"),
            }

            match &root.children[2] {
                hast::Node::Element(el) => {
                    assert_eq!(el.tag_name, "p");
                    assert_eq!(el.children.len(), 1);
                    match &el.children[0] {
                        hast::Node::Text(t) => assert_eq!(t.value, "world"),
                        other => panic!("expected Text, got {other:?}"),
                    }
                }
                other => panic!("expected Element(p), got {other:?}"),
            }
        }
        other => panic!("expected Root, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 2: Definition + LinkReference → since the arena Definition codec does
// not store the identifier, references always remain unresolved (both in the
// old two-step pipeline and in the new direct path). This test documents that
// behavior by verifying an unresolved LinkReference emits its children as a
// fragment.
// ---------------------------------------------------------------------------

#[test]
fn test_link_reference_unresolved_returns_children() {
    // Build: Root → Paragraph → LinkReference(identifier="foo") → Text("click")
    let source = "fooclick".to_string();
    // identifier "foo" at offset 0..3; text "click" at offset 3..8
    let id_ref = StringRef::new(0, 3); // "foo"
    let text_ref = StringRef::new(3, 5); // "click"

    let mut arena = MdastArena::new(source);
    let root_id = arena.alloc_node(NodeType::Root);
    let para_id = arena.alloc_node(NodeType::Paragraph);
    let link_ref_id = arena.alloc_node(NodeType::LinkReference);
    {
        // Encode ReferenceData: identifier=foo, label=empty, reference_kind=Full(2)
        use mdast_arena::codec::encode_reference_data;
        let label_ref = StringRef::empty();
        arena.set_type_data(link_ref_id, &encode_reference_data(id_ref, label_ref, 2));
    }
    let text_id = arena.alloc_node(NodeType::Text);
    arena.set_type_data(text_id, &encode_string_ref_data(text_ref));

    arena.set_children(link_ref_id, &[text_id]);
    arena.set_children(para_id, &[link_ref_id]);
    arena.set_children(root_id, &[para_id]);

    let result = mdast_to_hast(&arena, None);

    // Since the definition has no identifier stored in the arena codec,
    // the lookup fails and LinkReference emits its children as a fragment.
    // The paragraph therefore wraps the text "click".
    match &result {
        hast::Node::Root(root) => {
            // Root children after wrap(false): just [p]  (single child, no separating \n needed)
            let p = root
                .children
                .iter()
                .find(|n| matches!(n, hast::Node::Element(e) if e.tag_name == "p"));
            assert!(p.is_some(), "should have a <p> element");
            if let Some(hast::Node::Element(p_el)) = p {
                // LinkReference was unresolved → its children ("click") were inlined
                assert!(
                    p_el.children
                        .iter()
                        .any(|n| matches!(n, hast::Node::Text(t) if t.value == "click")),
                    "p should contain the text 'click' from the unresolved link reference"
                );
            }
        }
        other => panic!("expected Root, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 3: Unordered list → <ul> with <li> children
// ---------------------------------------------------------------------------

#[test]
fn test_unordered_list() {
    // Root → List(ordered=false) → [ListItem → Text("a"), ListItem → Text("b")]
    let source = "ab".to_string();
    let mut arena = MdastArena::new(source);

    let root_id = arena.alloc_node(NodeType::Root);
    let list_id = arena.alloc_node(NodeType::List);
    arena.set_type_data(list_id, &encode_list_data(false, 1, false));

    let item_a_id = arena.alloc_node(NodeType::ListItem);
    arena.set_type_data(item_a_id, &encode_list_item_data(2, false)); // checked=2 (not task)

    let text_a_id = arena.alloc_node(NodeType::Text);
    arena.set_type_data(text_a_id, &encode_string_ref_data(StringRef::new(0, 1)));

    let item_b_id = arena.alloc_node(NodeType::ListItem);
    arena.set_type_data(item_b_id, &encode_list_item_data(2, false));

    let text_b_id = arena.alloc_node(NodeType::Text);
    arena.set_type_data(text_b_id, &encode_string_ref_data(StringRef::new(1, 1)));

    arena.set_children(item_a_id, &[text_a_id]);
    arena.set_children(item_b_id, &[text_b_id]);
    arena.set_children(list_id, &[item_a_id, item_b_id]);
    arena.set_children(root_id, &[list_id]);

    let result = mdast_to_hast(&arena, None);

    match &result {
        hast::Node::Root(root) => {
            let ul = root
                .children
                .iter()
                .find(|n| matches!(n, hast::Node::Element(e) if e.tag_name == "ul"));
            assert!(ul.is_some(), "should have a <ul>");

            if let Some(hast::Node::Element(ul_el)) = ul {
                // wrap(true): [\n, li_a, \n, li_b, \n]
                let lis: Vec<_> = ul_el
                    .children
                    .iter()
                    .filter(|n| matches!(n, hast::Node::Element(e) if e.tag_name == "li"))
                    .collect();
                assert_eq!(lis.len(), 2, "ul should have 2 li children");
            }
        }
        other => panic!("expected Root, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 4: Code block with lang → <pre><code class="language-rust">
// ---------------------------------------------------------------------------

#[test]
fn test_code_block_with_lang() {
    // source contains lang "rust" and code "fn main() {}"
    let source = "rustfn main() {}".to_string();
    // lang: offset 0, len 4 ("rust")
    // code: offset 4, len 12 ("fn main() {}")
    let lang_ref = StringRef::new(0, 4);
    let code_ref = StringRef::new(4, 12);
    let meta_ref = StringRef::empty();

    let mut arena = MdastArena::new(source);
    let root_id = arena.alloc_node(NodeType::Root);
    let code_id = arena.alloc_node(NodeType::Code);
    arena.set_type_data(
        code_id,
        &encode_code_data(lang_ref, meta_ref, code_ref, b'`'),
    );

    arena.set_children(root_id, &[code_id]);

    let result = mdast_to_hast(&arena, None);

    match &result {
        hast::Node::Root(root) => {
            let pre = root
                .children
                .iter()
                .find(|n| matches!(n, hast::Node::Element(e) if e.tag_name == "pre"));
            assert!(pre.is_some(), "should have a <pre>");

            if let Some(hast::Node::Element(pre_el)) = pre {
                let code = pre_el
                    .children
                    .iter()
                    .find(|n| matches!(n, hast::Node::Element(e) if e.tag_name == "code"));
                assert!(code.is_some(), "pre should have a <code> child");

                if let Some(hast::Node::Element(code_el)) = code {
                    // Should have className="language-rust"
                    let has_class = code_el.properties.iter().any(|(k, v)| {
                        k == "class"
                            && matches!(
                                v,
                                hast::PropertyValue::SpaceSeparated(classes)
                                if classes.iter().any(|c| c == "language-rust")
                            )
                    });
                    assert!(has_class, "code should have class 'language-rust'");

                    // Content should be "fn main() {}\n"
                    let text = code_el
                        .children
                        .iter()
                        .find(|n| matches!(n, hast::Node::Text(_)));
                    assert!(text.is_some(), "code should have text content");
                    if let Some(hast::Node::Text(t)) = text {
                        assert_eq!(t.value, "fn main() {}\n");
                    }
                }
            }
        }
        other => panic!("expected Root, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 5: Image → <img> with src and alt
// ---------------------------------------------------------------------------

#[test]
fn test_image() {
    // source: "https://example.commy alt"
    let source = "https://example.commy alt".to_string();
    let url_ref = StringRef::new(0, 19); // "https://example.com"
    let alt_ref = StringRef::new(19, 6); // "my alt"
    let title_ref = StringRef::empty();

    let mut arena = MdastArena::new(source);
    let root_id = arena.alloc_node(NodeType::Root);
    // Images are inline; put them inside a paragraph
    let para_id = arena.alloc_node(NodeType::Paragraph);
    let img_id = arena.alloc_node(NodeType::Image);
    arena.set_type_data(img_id, &encode_image_data(url_ref, alt_ref, title_ref));

    arena.set_children(para_id, &[img_id]);
    arena.set_children(root_id, &[para_id]);

    let result = mdast_to_hast(&arena, None);

    match &result {
        hast::Node::Root(root) => {
            let p = root
                .children
                .iter()
                .find(|n| matches!(n, hast::Node::Element(e) if e.tag_name == "p"));
            assert!(p.is_some(), "should have a <p>");

            if let Some(hast::Node::Element(p_el)) = p {
                let img = p_el
                    .children
                    .iter()
                    .find(|n| matches!(n, hast::Node::Element(e) if e.tag_name == "img"));
                assert!(img.is_some(), "p should have an <img>");

                if let Some(hast::Node::Element(img_el)) = img {
                    let src = img_el.properties.iter().find(|(k, _)| k == "src");
                    assert!(src.is_some(), "img should have src");
                    if let Some((_, hast::PropertyValue::String(url))) = src {
                        assert_eq!(url, "https://example.com");
                    }

                    let alt = img_el.properties.iter().find(|(k, _)| k == "alt");
                    assert!(alt.is_some(), "img should have alt");
                    if let Some((_, hast::PropertyValue::String(a))) = alt {
                        assert_eq!(a, "my alt");
                    }
                }
            }
        }
        other => panic!("expected Root, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 6: Emphasis → <em>
// ---------------------------------------------------------------------------

#[test]
fn test_emphasis() {
    // Root → Paragraph → Emphasis → Text("hi")
    let source = "hi".to_string();
    let text_ref = StringRef::new(0, 2);

    let mut arena = MdastArena::new(source);
    let root_id = arena.alloc_node(NodeType::Root);
    let para_id = arena.alloc_node(NodeType::Paragraph);
    let em_id = arena.alloc_node(NodeType::Emphasis);
    let text_id = arena.alloc_node(NodeType::Text);
    arena.set_type_data(text_id, &encode_string_ref_data(text_ref));

    arena.set_children(em_id, &[text_id]);
    arena.set_children(para_id, &[em_id]);
    arena.set_children(root_id, &[para_id]);

    let result = mdast_to_hast(&arena, None);

    match &result {
        hast::Node::Root(root) => {
            let p = root
                .children
                .iter()
                .find(|n| matches!(n, hast::Node::Element(e) if e.tag_name == "p"));
            assert!(p.is_some(), "should have a <p>");

            if let Some(hast::Node::Element(p_el)) = p {
                let em = p_el
                    .children
                    .iter()
                    .find(|n| matches!(n, hast::Node::Element(e) if e.tag_name == "em"));
                assert!(em.is_some(), "p should have an <em>");

                if let Some(hast::Node::Element(em_el)) = em {
                    assert_eq!(em_el.children.len(), 1);
                    match &em_el.children[0] {
                        hast::Node::Text(t) => assert_eq!(t.value, "hi"),
                        other => panic!("expected Text in em, got {other:?}"),
                    }
                }
            }
        }
        other => panic!("expected Root, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 7: compile_arena still works end-to-end
// ---------------------------------------------------------------------------

#[test]
fn test_compile_arena_end_to_end() {
    // Use the mdx crate's parser to build a MdastArena and compile it.
    // We just verify it doesn't panic and produces JavaScript.
    use mdxjs::{Options, compile_arena};

    // Build a simple arena manually: Root → Paragraph → Text("hello")
    let source = "hello".to_string();
    let text_ref = StringRef::new(0, 5);

    let mut arena = MdastArena::new(source);
    let root_id = arena.alloc_node(NodeType::Root);
    let para_id = arena.alloc_node(NodeType::Paragraph);
    let text_id = arena.alloc_node(NodeType::Text);
    arena.set_type_data(text_id, &encode_string_ref_data(text_ref));

    arena.set_children(para_id, &[text_id]);
    arena.set_children(root_id, &[para_id]);

    let result = compile_arena(&arena, &Options::default());
    assert!(result.is_ok(), "compile_arena should succeed: {result:?}");
    let js = result.unwrap();
    assert!(
        js.contains("_createMdxContent"),
        "output should contain MDX boilerplate"
    );
}

#[test]
fn tab_indent_code_fence_in_list_compiles() {
    // This pattern appears in real-world MDX (Astro docs) - deeply indented
    // code fences inside list items.  After tab expansion, the fence has
    // 8 spaces of indent inside a "5. " list item (5 spaces after list strip).
    let input = "5. item:\n\n        ```ts\n    content {a: 1}\n        ```\n";
    let result = mdxjs::compile(input, &mdxjs::Options::default());
    assert!(
        result.is_ok(),
        "deep-indent code fence failed: {:?}",
        result.err()
    );
}

#[test]
fn double_quote_string_expression() {
    let input = r#"{"hello"}"#;
    let result = mdxjs::compile(input, &mdxjs::Options::default());
    assert!(
        result.is_ok(),
        "Double-quote expression failed: {:?}",
        result.err()
    );
}

#[test]
fn double_brace_object_expression() {
    let input = r#"{{ key: "value" }}"#;
    let result = mdxjs::compile(input, &mdxjs::Options::default());
    assert!(
        result.is_ok(),
        "Double-brace expression failed: {:?}",
        result.err()
    );
}

/// List items inside JSX elements should not have extra leading/trailing newlines.
/// Matches the output of `@mdx-js/mdx`.
#[test]
fn test_list_in_jsx_no_extra_newlines() {
    let input = "<FileTree>\n- src/\n  - index.ts\n- package.json\n</FileTree>\n";
    let result = mdxjs::compile(input, &mdxjs::Options::default()).unwrap();

    // The last <li> should have a single child "package.json", not wrapped in newlines
    assert!(
        result.contains(r#"children: "package.json"#),
        "last <li> should have single text child without newlines, got:\n{result}"
    );
    // The first <li> children should NOT start with "\n" (no leading newline)
    assert!(
        !result.contains(r#"li, { children: [
            "\n",
            "src/"#),
        "first <li> should not have leading newline before text, got:\n{result}"
    );
}
#[test]
fn test_loose_list_jsx_mdast() {
    let input = "1. Item one\n\n2. Add config:\n    <FileTree>\n      - public\n        - admin\n    </FileTree>\n";
    let (arena, errors) = parser::parse(input, &parser::ParseOptions::mdx());
    
    eprintln!("Parse errors: {:?}", errors);
    
    fn dump(arena: &dyn mdast_arena::ReadMdast, node_id: u32, indent: usize) {
        let raw = arena.get_node(node_id);
        let nt = mdast_arena::NodeType::from_u8(raw.node_type).unwrap_or(mdast_arena::NodeType::Root);
        let pad = " ".repeat(indent);
        let src = &arena.source()[raw.start_offset as usize..raw.end_offset as usize];
        let preview: String = src.chars().take(40).collect();
        eprintln!("{pad}{nt:?} (id={}, {}..{}) {:?}", raw.id, raw.start_offset, raw.end_offset, preview);
        for &child_id in arena.get_children(node_id) {
            dump(arena, child_id, indent + 2);
        }
    }
    
    dump(&arena, 0, 0);
}

#[test]
fn test_loose_list_paragraphs() {
    let input = "1. First item\n\n    Continuation paragraph.\n\n2. Second item\n";
    let result = mdxjs::compile(input, &mdxjs::Options::default()).unwrap();
    eprintln!("{result}");
    // Loose list items should have <p> wrapped content
    let p_count = result.matches("_components.p").count();
    assert!(p_count >= 3, "loose list should have at least 3 <p> references, got {p_count}");
}
