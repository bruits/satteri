use satteri_arena::{Arena, Hast, Mdast};
use std::sync::{Arc, Mutex};

#[test]
fn renderers_preserve_function_pointer_and_deref_call_sites() {
    let (arena, errors) =
        satteri_pulldown_cmark::parse("Hi &amp;", satteri_pulldown_cmark::DEFAULT_OPTIONS);
    assert!(errors.is_empty());
    let render: fn(&Arena<Mdast>) -> String = satteri_ast::mdast_to_html;
    let convert: fn(&Arena<Mdast>) -> Arena<Hast> = satteri_ast::hast::mdast_arena_to_hast_arena;
    let render_hast: fn(&Arena<Hast>) -> String = satteri_ast::hast::hast_arena_to_html;
    let expected = "<p>Hi &amp;</p>\n";
    assert_eq!(render(&arena), expected);
    assert_eq!(render_hast(&convert(&arena)), expected);

    let locked = Mutex::new(Arc::new(Box::new(arena)));
    let guard = locked.lock().unwrap();
    assert_eq!(satteri_ast::mdast_to_html(&guard), expected);
    let hast = satteri_ast::hast::mdast_arena_to_hast_arena(&guard);
    let locked_hast = Mutex::new(hast);
    assert_eq!(
        satteri_ast::hast::hast_arena_to_html(&locked_hast.lock().unwrap()),
        expected
    );
}
