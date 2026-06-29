use satteri_pulldown_cmark::{parse, Options};

const ISSUE_MARKDOWN: &str = "This is an MDX page! You can access it using `/mdx`.

## Images with Markdown syntax

![an image](https://placehold.co/600x400)

## Fenced code

```js
const a = 2;
const b = 4;
```
";

#[test]
fn source_is_the_verbatim_input() {
    let (arena, _) = parse(ISSUE_MARKDOWN, Options::empty());
    assert_eq!(arena.source(), ISSUE_MARKDOWN);
    // URL and inline-code text appear once in the input but are duplicated in
    // the heap past the boundary, so count occurrences rather than `contains`.
    assert!(arena.string_pool().len() > ISSUE_MARKDOWN.len());
    assert_eq!(arena.source().matches("placehold.co").count(), 1);
    assert!(arena.string_pool().matches("placehold.co").count() > 1);
    assert_eq!(arena.source().matches("/mdx").count(), 1);
}

#[test]
fn smart_punctuation_does_not_duplicate_into_source() {
    let input = "page d'accueil\n";
    let (arena, _) = parse(input, Options::ENABLE_SMART_QUOTES);
    assert_eq!(arena.source(), input);
}

#[test]
fn mdast_to_hast_conversion_preserves_the_source_boundary() {
    let (mdast, _) = parse(ISSUE_MARKDOWN, Options::empty());
    let hast = satteri_ast::hast::mdast_arena_to_hast_arena(&mdast);
    // HAST reuses the MDAST pool for StringRefs; its source view must still match.
    assert_eq!(hast.source(), ISSUE_MARKDOWN);
}
