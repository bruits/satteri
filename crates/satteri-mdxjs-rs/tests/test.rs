extern crate satteri_mdxjs;
use pretty_assertions::assert_eq;
use satteri_mdxjs::{
    ElementAttributeNameCase, JsxRuntime, OptimizeStaticConfig, Options, OutputFormat,
    StylePropertyNameCase, compile,
};

const MDX_OPTS: satteri_pulldown_cmark::Options = satteri_pulldown_cmark::MDX_OPTIONS;

#[test]
fn simple() -> Result<(), satteri_arena::mdx_types::Message> {
    assert_eq!(
        compile("", &Options::default(), MDX_OPTS)?,
        "import { Fragment as _Fragment, jsx as _jsx } from \"react/jsx-runtime\";
function _createMdxContent(props) {
    return _jsx(_Fragment, {});
}
function MDXContent(props = {}) {
    const { wrapper: MDXLayout } = props.components || {};
    return MDXLayout ? _jsx(MDXLayout, Object.assign({}, props, { children: _jsx(_createMdxContent, props) })) : _createMdxContent(props);
}
export default MDXContent;
",
        "should work",
    );

    Ok(())
}

#[test]
fn development() -> Result<(), satteri_arena::mdx_types::Message> {
    assert_eq!(
        compile("<A />", &Options {
            development: true,
            filepath: Some("example.mdx".into()),
            ..Default::default()
        }, MDX_OPTS)?,
        "import { jsxDEV as _jsxDEV } from \"react/jsx-dev-runtime\";
function _createMdxContent(props) {
    const { A } = props.components || {};
    if (!A) _missingMdxReference(\"A\", true, \"1:1-1:6\");
    return _jsxDEV(A, {}, undefined, false, {
        fileName: \"example.mdx\",
        lineNumber: 1,
        columnNumber: 1
    }, this);
}
function MDXContent(props = {}) {
    const { wrapper: MDXLayout } = props.components || {};
    return MDXLayout ? _jsxDEV(MDXLayout, Object.assign({}, props, { children: _jsxDEV(_createMdxContent, props, undefined, false, { fileName: \"example.mdx\" }, this) }), undefined, false, { fileName: \"example.mdx\" }, this) : _createMdxContent(props);
}
export default MDXContent;
function _missingMdxReference(id, component, place) {
    throw new Error(\"Expected \" + (component ? \"component\" : \"object\") + \" `\" + id + \"` to be defined: you likely forgot to import, pass, or provide it.\" + (place ? \"\\nIt’s referenced in your code at `\" + place + \"` in `example.mdx`\" : \"\"));
}
",
        "should support `options.development: true`",
    );

    Ok(())
}

#[test]
fn provider() -> Result<(), satteri_arena::mdx_types::Message> {
    assert_eq!(
        compile("<A />",  &Options {
            provider_import_source: Some("@mdx-js/react".into()),
            ..Default::default()
        }, MDX_OPTS)?,
        "import { jsx as _jsx } from \"react/jsx-runtime\";
import { useMDXComponents as _provideComponents } from \"@mdx-js/react\";
function _createMdxContent(props) {
    const { A } = Object.assign({}, _provideComponents(), props.components);
    if (!A) _missingMdxReference(\"A\", true);
    return _jsx(A, {});
}
function MDXContent(props = {}) {
    const { wrapper: MDXLayout } = Object.assign({}, _provideComponents(), props.components);
    return MDXLayout ? _jsx(MDXLayout, Object.assign({}, props, { children: _jsx(_createMdxContent, props) })) : _createMdxContent(props);
}
export default MDXContent;
function _missingMdxReference(id, component) {
    throw new Error(\"Expected \" + (component ? \"component\" : \"object\") + \" `\" + id + \"` to be defined: you likely forgot to import, pass, or provide it.\");
}
",
        "should support `options.provider_import_source`",
    );

    Ok(())
}

#[test]
fn jsx() -> Result<(), satteri_arena::mdx_types::Message> {
    assert_eq!(
        compile("", &Options {
            jsx: true,
            ..Default::default()
        }, MDX_OPTS)?,
        "function _createMdxContent(props) {
    return <></>;
}
function MDXContent(props = {}) {
    const { wrapper: MDXLayout } = props.components || {};
    return MDXLayout ? <MDXLayout {...props}><_createMdxContent {...props} /></MDXLayout> : _createMdxContent(props);
}
export default MDXContent;
",
        "should support `options.jsx: true`",
    );

    Ok(())
}

#[test]
fn layout_export_default_from() -> Result<(), satteri_arena::mdx_types::Message> {
    // `export { default } from` lowers to a layout import; the wrapper
    // destructure would otherwise shadow it to `undefined`.
    let result = compile(
        "export { default } from './Layout.astro';\n\n# Hello\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("import { default as MDXLayout } from \"./Layout.astro\""),
        "should rewrite `export {{ default }} from` as an MDXLayout import: {result}"
    );
    assert!(
        !result.contains("wrapper: MDXLayout"),
        "MDXContent must not destructure `wrapper: MDXLayout` when MDXLayout is imported: {result}"
    );
    assert!(
        !result.contains("MDXLayout ?"),
        "MDXContent must not use a conditional when there is an internal layout: {result}"
    );
    assert!(
        result.contains(
            "return _jsx(MDXLayout, Object.assign({}, props, { children: _jsx(_createMdxContent, props) }));"
        ),
        "MDXContent must wrap with MDXLayout directly: {result}"
    );
    Ok(())
}

#[test]
fn element_attribute_name_case_react_default() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "```js\nconsole.log(1);\n```\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("className: \"language-js\""),
        "default casing should be React-cased `className`: {result}"
    );
    assert!(
        !result.contains("class: \"language-js\""),
        "default casing should not emit `class`: {result}"
    );
    Ok(())
}

#[test]
fn element_attribute_name_case_html() -> Result<(), satteri_arena::mdx_types::Message> {
    let opts = MDX_OPTS | satteri_pulldown_cmark::Options::ENABLE_FOOTNOTES;
    let result = compile(
        "```js\nconsole.log(1);\n```\n\na[^1]\n\n[^1]: note\n",
        &Options {
            element_attribute_name_case: ElementAttributeNameCase::Html,
            ..Default::default()
        },
        opts,
    )?;
    assert!(
        result.contains("class: \"language-js\""),
        "`html` casing should emit `class`: {result}"
    );
    assert!(
        !result.contains("className:"),
        "`html` casing should not emit `className`: {result}"
    );
    assert!(
        result.contains("class: \"footnotes\""),
        "footnote section class should be lowercased to `class`: {result}"
    );
    // `data-*` / `aria-*` are already kebab-cased in both modes, but confirm
    // they're still present (these come from a regular String, not via the
    // React mapping table).
    assert!(
        result.contains("\"data-footnote-ref\""),
        "data-* attrs should survive: {result}"
    );
    assert!(
        result.contains("\"aria-describedby\""),
        "aria-* attrs should survive: {result}"
    );
    Ok(())
}

#[test]
fn style_property_name_case_dom_default() -> Result<(), satteri_arena::mdx_types::Message> {
    let opts = MDX_OPTS | satteri_pulldown_cmark::Options::ENABLE_TABLES;
    let result = compile(
        "| a | b |\n|:--|--:|\n| c | d |\n",
        &Options::default(),
        opts,
    )?;
    assert!(
        result.contains("style: { textAlign: \"right\" }"),
        "default style casing should be DOM (camelCase): {result}"
    );
    assert!(
        result.contains("style: { textAlign: \"left\" }"),
        "default style casing should be DOM (camelCase): {result}"
    );
    Ok(())
}

#[test]
fn style_property_name_case_css() -> Result<(), satteri_arena::mdx_types::Message> {
    let opts = MDX_OPTS | satteri_pulldown_cmark::Options::ENABLE_TABLES;
    let result = compile(
        "| a | b |\n|:--|--:|\n| c | d |\n",
        &Options {
            style_property_name_case: StylePropertyNameCase::Css,
            ..Default::default()
        },
        opts,
    )?;
    // `text-align` is not a valid JS identifier so it becomes a string key.
    assert!(
        result.contains("\"text-align\": \"right\""),
        "css casing should kebab-case the style key: {result}"
    );
    assert!(
        result.contains("\"text-align\": \"left\""),
        "css casing should kebab-case the style key: {result}"
    );
    Ok(())
}

#[test]
fn layout_export_default_function() -> Result<(), satteri_arena::mdx_types::Message> {
    // `export default function Layout(...)` lowers to `const MDXLayout = function Layout(...) {...}`.
    // Same scope rule applies: no wrapper destructure, no conditional.
    let result = compile(
        "export default function Layout(props) { return <section {...props} /> }\n\na\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("const MDXLayout = function Layout"),
        "should rewrite `export default function` as `const MDXLayout = ...`: {result}"
    );
    assert!(
        !result.contains("wrapper: MDXLayout"),
        "MDXContent must not destructure `wrapper: MDXLayout` when MDXLayout is a top-level const: {result}"
    );
    assert!(
        !result.contains("MDXLayout ?"),
        "MDXContent must not use a conditional when there is an internal layout: {result}"
    );
    Ok(())
}

#[test]
fn classic() -> Result<(), satteri_arena::mdx_types::Message> {
    assert_eq!(
        compile("", &Options {
            jsx_runtime: Some(JsxRuntime::Classic),
            ..Default::default()
        }, MDX_OPTS)?,
        "import React from \"react\";
function _createMdxContent(props) {
    return React.createElement(React.Fragment);
}
function MDXContent(props = {}) {
    const { wrapper: MDXLayout } = props.components || {};
    return MDXLayout ? React.createElement(MDXLayout, props, React.createElement(_createMdxContent, props)) : _createMdxContent(props);
}
export default MDXContent;
",
        "should support `options.jsx_runtime: JsxRuntime::Classic`",
    );

    Ok(())
}

#[test]
fn import_source() -> Result<(), satteri_arena::mdx_types::Message> {
    assert_eq!(
        compile(
            "",
            &Options {
                jsx_import_source: Some("preact".into()),
                ..Default::default()
            },
            MDX_OPTS,
        )?,
        "import { Fragment as _Fragment, jsx as _jsx } from \"preact/jsx-runtime\";
function _createMdxContent(props) {
    return _jsx(_Fragment, {});
}
function MDXContent(props = {}) {
    const { wrapper: MDXLayout } = props.components || {};
    return MDXLayout ? _jsx(MDXLayout, Object.assign({}, props, { children: _jsx(_createMdxContent, props) })) : _createMdxContent(props);
}
export default MDXContent;
",
        "should support `options.jsx_import_source: Some(\"preact\".into())`",
    );

    Ok(())
}

#[test]
fn pragmas() -> Result<(), satteri_arena::mdx_types::Message> {
    assert_eq!(
        compile("", &Options {
            jsx_runtime: Some(JsxRuntime::Classic),
            pragma: Some("a.b".into()),
            pragma_frag: Some("a.c".into()),
            pragma_import_source: Some("d".into()),
            ..Default::default()
        }, MDX_OPTS)?,
        "import a from \"d\";
function _createMdxContent(props) {
    return a.b(a.c);
}
function MDXContent(props = {}) {
    const { wrapper: MDXLayout } = props.components || {};
    return MDXLayout ? a.b(MDXLayout, props, a.b(_createMdxContent, props)) : _createMdxContent(props);
}
export default MDXContent;
",
        "should support `options.pragma`, `options.pragma_frag`, `options.pragma_import_source`",
    );

    Ok(())
}

#[test]
fn unravel_elements() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile("<x>a</x>\n<x>\n  b\n</x>\n", &Default::default(), MDX_OPTS)?;
    // Must produce valid JS with both <x> elements.
    assert!(
        result.contains("\"x\""),
        "should have x component: {result}"
    );
    assert!(result.contains("\"a\""), "should have 'a' text: {result}");
    assert!(result.contains("\"b\""), "should have 'b' text: {result}");
    assert!(
        result.contains("export default MDXContent"),
        "should have default export: {result}"
    );
    Ok(())
}

#[test]
fn unravel_expressions() -> Result<(), satteri_arena::mdx_types::Message> {
    assert_eq!(
        compile("{1} {2}", &Default::default(), MDX_OPTS)?,
        "import { Fragment as _Fragment, jsx as _jsx, jsxs as _jsxs } from \"react/jsx-runtime\";
function _createMdxContent(props) {
    return _jsxs(_Fragment, { children: [
        1,
        \"\\n\",
        2
    ] });
}
function MDXContent(props = {}) {
    const { wrapper: MDXLayout } = props.components || {};
    return MDXLayout ? _jsx(MDXLayout, Object.assign({}, props, { children: _jsx(_createMdxContent, props) })) : _createMdxContent(props);
}
export default MDXContent;
",
        "should unravel paragraphs (2)",
    );

    Ok(())
}

#[test]
fn explicit_jsx() -> Result<(), satteri_arena::mdx_types::Message> {
    assert_eq!(
        compile(
            "<h1>asd</h1>
# qwe
",
            &Default::default(),
            MDX_OPTS,
        )?,
        "import { Fragment as _Fragment, jsx as _jsx, jsxs as _jsxs } from \"react/jsx-runtime\";
function _createMdxContent(props) {
    const _components = Object.assign({ h1: \"h1\" }, props.components);
    return _jsxs(_Fragment, { children: [
        _jsx(\"h1\", { children: \"asd\" }),
        \"\\n\",
        _jsx(_components.h1, { children: \"qwe\" })
    ] });
}
function MDXContent(props = {}) {
    const { wrapper: MDXLayout } = props.components || {};
    return MDXLayout ? _jsx(MDXLayout, Object.assign({}, props, { children: _jsx(_createMdxContent, props) })) : _createMdxContent(props);
}
export default MDXContent;
",
        "should not support overwriting explicit JSX",
    );

    Ok(())
}

// optimize_static tests

#[test]
fn optimize_static_default_off() -> Result<(), satteri_arena::mdx_types::Message> {
    // With no optimize_static, output should contain _jsx("h1", ...) calls.
    let result = compile("# Hello\n\nWorld", &Options::default(), MDX_OPTS)?;
    assert!(
        result.contains("\"h1\""),
        "should have h1 element call: {result}"
    );
    assert!(
        !result.contains("set:html"),
        "should not have set:html without optimization: {result}"
    );
    Ok(())
}

#[test]
fn optimize_static_astro_style() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "# Hello\n\nWorld",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    // Should contain set:html with serialized HTML
    assert!(
        result.contains("set:html"),
        "should have set:html attribute: {result}"
    );
    assert!(
        result.contains("<h1>Hello</h1>"),
        "should have collapsed h1 HTML: {result}"
    );
    assert!(
        result.contains("<p>World</p>"),
        "should have collapsed p HTML: {result}"
    );
    // Should NOT contain individual _jsx("h1", ...) calls
    assert!(
        !result.contains("\"h1\""),
        "should not have h1 element call: {result}"
    );
    Ok(())
}

#[test]
fn optimize_static_react_style() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "# Hello\n\nWorld",
        &Options {
            optimize_static: Some(OptimizeStaticConfig {
                component: "div".into(),
                prop: "dangerouslySetInnerHTML".into(),
                wrap_prop_value: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    assert!(
        result.contains("dangerouslySetInnerHTML"),
        "should have dangerouslySetInnerHTML: {result}"
    );
    assert!(
        result.contains("__html"),
        "should have __html wrapper: {result}"
    );
    Ok(())
}

#[test]
fn optimize_static_mixed_dynamic() -> Result<(), satteri_arena::mdx_types::Message> {
    // Static content + dynamic MDX component, static parts should be collapsed,
    // dynamic parts should remain as JSX.
    let result = compile(
        "# Hello\n\n<Component />\n\nWorld",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    assert!(
        result.contains("set:html"),
        "should have set:html for static parts: {result}"
    );
    // Component should remain as a JSX call
    assert!(
        result.contains("Component"),
        "should preserve Component reference: {result}"
    );
    Ok(())
}

#[test]
fn optimize_static_ignore_elements() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "# Hello\n\nWorld",
        &Options {
            optimize_static: Some(OptimizeStaticConfig {
                ignore_elements: vec!["h1".into()],
                ..Default::default()
            }),
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    // h1 should NOT be collapsed (it's in ignore list), but p should be
    assert!(
        result.contains("set:html"),
        "should have set:html for non-ignored elements: {result}"
    );
    // h1 should remain as a JSX component reference (not collapsed)
    assert!(
        result.contains("\"h1\""),
        "should preserve h1 as JSX call: {result}"
    );
    Ok(())
}

#[test]
fn optimize_static_sibling_grouping() -> Result<(), satteri_arena::mdx_types::Message> {
    // Multiple consecutive static elements should be grouped into one set:html
    let result = compile(
        "# A\n\n## B\n\n### C",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    // All three headings should be in a single set:html
    assert!(result.contains("<h1>A</h1>"), "should contain h1: {result}");
    assert!(result.contains("<h2>B</h2>"), "should contain h2: {result}");
    assert!(result.contains("<h3>C</h3>"), "should contain h3: {result}");
    // Count occurrences of set:html, should be exactly 1 for fully static content
    let count = result.matches("set:html").count();
    assert_eq!(count, 1, "should have exactly one set:html group: {result}");
    Ok(())
}

#[test]
fn optimize_static_nested_dynamic_prevents_collapse()
-> Result<(), satteri_arena::mdx_types::Message> {
    // A paragraph with an inline expression cannot be collapsed
    let result = compile(
        "# Static\n\nHello {name} world",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    // The h1 should be collapsed (static)
    assert!(
        result.contains("<h1>Static</h1>"),
        "should collapse static h1: {result}"
    );
    // The paragraph with expression should NOT be collapsed
    assert!(
        result.contains("name"),
        "should preserve expression: {result}"
    );
    Ok(())
}

// optimize_static component override detection tests

#[test]
fn optimize_static_detects_component_overrides() -> Result<(), satteri_arena::mdx_types::Message> {
    // A file declaring `export const components = { h1: Custom }` must not
    // collapse its <h1> subtree, otherwise the runtime override never fires.
    let result = compile(
        "import Custom from './c.js'\nexport const components = { h1: Custom }\n\n# Heading\n\nPara",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    assert!(
        result.contains("\"h1\""),
        "h1 should remain as JSX call: {result}"
    );
    assert!(
        !result.contains("<h1>"),
        "h1 should not appear inside raw HTML: {result}"
    );
    assert!(
        result.contains("set:html"),
        "other static content should still collapse: {result}"
    );
    Ok(())
}

#[test]
fn optimize_static_detect_overrides_shorthand() -> Result<(), satteri_arena::mdx_types::Message> {
    // Shorthand `{ h1, p }` resolves to keys "h1" and "p".
    let result = compile(
        "import h1 from './h1.js'\nimport p from './p.js'\nexport const components = { h1, p }\n\n# Heading\n\nPara",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    assert!(result.contains("\"h1\""), "h1 should remain JSX: {result}");
    assert!(result.contains("\"p\""), "p should remain JSX: {result}");
    assert!(
        !result.contains("<h1>"),
        "h1 should not be inlined as HTML: {result}"
    );
    assert!(
        !result.contains("<p>"),
        "p should not be inlined as HTML: {result}"
    );
    Ok(())
}

#[test]
fn optimize_static_detect_overrides_mixed() -> Result<(), satteri_arena::mdx_types::Message> {
    // `{ h1, p: Custom }` — shorthand + explicit mapped both collected.
    let result = compile(
        "import h1 from './h1.js'\nimport Custom from './c.js'\nexport const components = { h1, p: Custom }\n\n# Heading\n\nPara",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    assert!(result.contains("\"h1\""), "h1 should remain JSX: {result}");
    assert!(result.contains("\"p\""), "p should remain JSX: {result}");
    Ok(())
}

#[test]
fn optimize_static_detect_overrides_spread_ignored() -> Result<(), satteri_arena::mdx_types::Message>
{
    // Spread elements (`...base`) are silently skipped; only identifier keys
    // are collected. Here `h1` still counts, but `base`'s contents don't.
    let result = compile(
        "import base from './b.js'\nimport Custom from './c.js'\nexport const components = { ...base, h1: Custom }\n\n# Heading\n\nPara",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    assert!(
        result.contains("\"h1\""),
        "h1 should remain JSX despite spread sibling: {result}"
    );
    // The paragraph isn't referenced by any identifier key, so it must still
    // collapse — confirms spreads don't implicitly ignore everything.
    assert!(
        result.contains("set:html") && result.contains("<p>Para</p>"),
        "p should still collapse: {result}"
    );
    Ok(())
}

#[test]
fn optimize_static_detect_overrides_string_keys_ignored()
-> Result<(), satteri_arena::mdx_types::Message> {
    // Non-identifier literal keys (`"h1": Custom`) are intentionally skipped
    // (matches the astro plugin's behavior). Documents the v1 limitation.
    let result = compile(
        "import Custom from './c.js'\nexport const components = { \"h1\": Custom }\n\n# Heading",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    assert!(
        result.contains("<h1>Heading</h1>"),
        "h1 should collapse because string-literal key wasn't detected: {result}"
    );
    Ok(())
}

#[test]
fn optimize_static_detect_overrides_no_declaration() -> Result<(), satteri_arena::mdx_types::Message>
{
    // Files without `export const components` behave identically to the
    // pre-feature optimizer — the prepass costs only a substring gate.
    let result = compile(
        "# Heading\n\nPara",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    assert!(
        result.contains("<h1>Heading</h1>"),
        "h1 should collapse normally: {result}"
    );
    assert!(
        result.contains("<p>Para</p>"),
        "p should collapse normally: {result}"
    );
    Ok(())
}

#[test]
fn optimize_static_detect_overrides_first_wins() -> Result<(), satteri_arena::mdx_types::Message> {
    // Two `export const components` blocks are ill-formed JS, but our
    // detection follows first-wins semantics (matches the astro plugin).
    // The first declares `{ h1: A }`, so h1 stays JSX; the second declares
    // `{ h2: B }` and is ignored — h2 should still collapse.
    let result = compile(
        "import A from './a.js'\nexport const components = { h1: A }\n\n# Heading\n\n## Sub\n\nimport B from './b.js'\nexport const components = { h2: B }\n",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    assert!(
        result.contains("\"h1\""),
        "h1 should remain JSX (first block wins): {result}"
    );
    assert!(
        result.contains("<h2>Sub</h2>"),
        "h2 should still collapse (second block ignored): {result}"
    );
    Ok(())
}

#[test]
fn explicit_only_literal_tags_not_in_components() -> Result<(), satteri_arena::mdx_types::Message> {
    // Explicit JSX with a hyphenated tag name: matches @mdx-js/mdx — the tag
    // stays as a string literal and does not get added to _components defaults.
    // The hyphen is what previously risked emitting an invalid defaults key
    // (`{ astro-image: ... }`) and an invalid `_components.astro-image` access;
    // both are avoided by not including the tag at all.
    let result = compile(
        "# Test\n\n<my-widget foo=\"bar\">child</my-widget>\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        !result.contains("my-widget\": \"my-widget\""),
        "explicit-only tag should not be in _components defaults: {result}"
    );
    assert!(
        !result.contains("_components.my-widget") && !result.contains("_components[\"my-widget\"]"),
        "explicit hyphenated JSX should not be routed through _components: {result}"
    );
    assert!(
        result.contains("_jsx(\"my-widget\""),
        "explicit hyphenated JSX stays as a string literal: {result}"
    );
    // Meanwhile, the markdown-generated `h1` still gets routed through _components.
    assert!(
        result.contains("_jsx(_components.h1"),
        "markdown-generated h1 should route through _components: {result}"
    );
    Ok(())
}

#[test]
fn mixed_explicit_and_implicit_tag_routes_implicit_only()
-> Result<(), satteri_arena::mdx_types::Message> {
    // When the same tag `p` appears both as a markdown paragraph (implicit) and
    // as a user-written `<p foo>` (explicit), only the implicit occurrence is
    // routed through _components. The explicit one keeps its string literal.
    let result = compile(
        "Para1\n\n<p foo=\"bar\">explicit</p>\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("p: \"p\""),
        "p should be in defaults (has implicit occurrence): {result}"
    );
    assert!(
        result.contains("_jsx(_components.p, { children: \"Para1\""),
        "implicit p should route through _components: {result}"
    );
    assert!(
        result.contains("_jsx(\"p\", {\n            foo: \"bar\""),
        "explicit <p foo> should stay as string literal: {result}"
    );
    Ok(())
}

#[test]
fn children_arrow_returning_string() -> Result<(), satteri_arena::mdx_types::Message> {
    // An arrow function as element body must be preserved as `children`,
    // not discarded when the element would otherwise look single-line.
    let result = compile(
        "import Comp from './Comp.astro';\n\n<Comp id=\"test\">{() => \"hello\"}</Comp>\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("children: () => \"hello\""),
        "arrow-returning-string child must be kept: {result}"
    );
    assert!(
        result.contains("_jsx(Comp, {"),
        "outer element should compile to _jsx(Comp, ...): {result}"
    );
    Ok(())
}

#[test]
fn children_arrow_returning_jsx() -> Result<(), satteri_arena::mdx_types::Message> {
    // JSX inside an arrow function body must itself be transformed to _jsx(...).
    let result = compile(
        "import Comp from './Comp.astro';\n\n<Comp id=\"test\">{(text) => <span>{text}</span>}</Comp>\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("children: (text) => _jsx(_components.span, { children: text })"),
        "JSX inside arrow body must be transformed: {result}"
    );
    Ok(())
}

#[test]
fn jsx_in_expression_preserves_significant_whitespace()
-> Result<(), satteri_arena::mdx_types::Message> {
    // JSX keeps a no-newline whitespace run as a significant `" "`, even via the expression path.
    let result = compile(
        "<C d={<><x>a</x> <y>b</y><em> </em></>} />\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.matches("\" \"").count() >= 2,
        "significant inter-element and spacer whitespace must be preserved: {result}"
    );
    Ok(())
}

#[test]
fn jsx_inside_map_callback() -> Result<(), satteri_arena::mdx_types::Message> {
    // `items.map(x => <li>{x}</li>)` must produce _jsx calls in the callback,
    // not leave raw JSX in the compiled output.
    let result = compile(
        "export const items = [\"a\", \"b\"];\n\n<ul>\n  {items.map(x => <li>{x}</li>)}\n</ul>\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("items.map((x) => _jsx(_components.li, { children: x }))"),
        "JSX inside .map callback must be transformed: {result}"
    );
    assert!(
        !result.contains("<li>") && !result.contains("</li>"),
        "compiled output must not contain raw JSX: {result}"
    );
    Ok(())
}

#[test]
fn jsx_inside_flow_expression_block() -> Result<(), satteri_arena::mdx_types::Message> {
    // A block-level `{expression}` containing JSX (e.g. inside a div) must
    // recurse into the expression body and transform the inner JSX.
    let result = compile(
        "export const data = [1, 2, 3];\n\n<div class=\"list\">\n  {data.map(i => (\n    <span>{i}</span>\n  ))}\n</div>\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("data.map((i) => _jsx(_components.span, { children: i }))"),
        "JSX inside block-level expression must be transformed: {result}"
    );
    assert!(
        !result.contains("<span>") && !result.contains("</span>"),
        "compiled output must not contain raw JSX: {result}"
    );
    Ok(())
}

#[test]
fn jsx_inside_template_literal() -> Result<(), satteri_arena::mdx_types::Message> {
    // Template literals can contain JSX via `${...}` interpolation; the
    // transform must recurse through the template expression parts.
    let result = compile(
        "<div>{`prefix ${<span>x</span>} suffix`}</div>\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("_jsx(\"span\", { children: \"x\" })"),
        "JSX inside template literal interpolation must be transformed: {result}"
    );
    Ok(())
}

#[test]
fn jsx_inside_new_expression() -> Result<(), satteri_arena::mdx_types::Message> {
    // `new Thing(<foo />)` — JSX in arg lists of NewExpression must recurse.
    let result = compile(
        "<div>{new Wrapper(<span>y</span>)}</div>\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("new Wrapper(_jsx(\"span\", { children: \"y\" }))"),
        "JSX inside NewExpression must be transformed: {result}"
    );
    Ok(())
}

#[test]
fn jsx_inside_switch_case() -> Result<(), satteri_arena::mdx_types::Message> {
    // JSX returned from a switch case inside an IIFE must be lowered, not
    // left as raw JSX. Regression from Cloudflare Docs' c3-post-run-steps.mdx.
    let result = compile(
        "{(function () {\n  switch (props.k) {\n    case 'a': return <ul><li>A</li><li>AA</li></ul>;\n    case 'b': return <p>B</p>;\n  }\n})()}\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("case \"a\": return _jsxs(\"ul\",")
            && result.contains("case \"b\": return _jsx(\"p\","),
        "JSX inside switch case bodies must be lowered: {result}"
    );
    assert!(
        !result.contains("<ul>")
            && !result.contains("</ul>")
            && !result.contains("<li>")
            && !result.contains("<p>"),
        "compiled output must not contain raw JSX: {result}"
    );
    Ok(())
}

#[test]
fn jsx_inside_try_catch_finally() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "{(() => {\n  try { return <a>t</a>; }\n  catch (e) { return <b>c</b>; }\n  finally { void <i>f</i>; }\n})()}\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("_jsx(\"a\", { children: \"t\" })"),
        "try-block JSX must be lowered: {result}"
    );
    assert!(
        result.contains("_jsx(\"b\", { children: \"c\" })"),
        "catch-block JSX must be lowered: {result}"
    );
    assert!(
        result.contains("_jsx(\"i\", { children: \"f\" })"),
        "finally-block JSX must be lowered: {result}"
    );
    Ok(())
}

#[test]
fn jsx_inside_while_and_for_bodies() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "{(() => {\n  while (cond) { void <w>1</w>; }\n  for (let i = 0; i < 3; i++) { void <f>2</f>; }\n  for (const x of xs) { void <o>3</o>; }\n  return null;\n})()}\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("_jsx(\"w\", { children: \"1\" })"),
        "while-body JSX must be lowered: {result}"
    );
    assert!(
        result.contains("_jsx(\"f\", { children: \"2\" })"),
        "for-body JSX must be lowered: {result}"
    );
    assert!(
        result.contains("_jsx(\"o\", { children: \"3\" })"),
        "for-of-body JSX must be lowered: {result}"
    );
    Ok(())
}

#[test]
fn jsx_inside_class_method_and_field() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "export class Demo {\n  field = <d>init</d>;\n  render() { return <r>rendered</r>; }\n}\n\n<p>hi</p>\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("_jsx(\"d\", { children: \"init\" })"),
        "class field initializer JSX must be lowered: {result}"
    );
    assert!(
        result.contains("_jsx(\"r\", { children: \"rendered\" })"),
        "class method body JSX must be lowered: {result}"
    );
    assert!(
        !result.contains("<d>") && !result.contains("<r>"),
        "compiled output must not contain raw JSX: {result}"
    );
    Ok(())
}

#[test]
fn jsx_inside_if_statement_test() -> Result<(), satteri_arena::mdx_types::Message> {
    // JSX inside an `if (...)` test expression (used for truthiness) must be
    // lowered too, not left as a raw JSXElement.
    let result = compile(
        "{(() => {\n  if (<span>c</span>) { return <div>t</div>; }\n  return null;\n})()}\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("_jsx(\"span\", { children: \"c\" })"),
        "JSX inside if-test must be lowered: {result}"
    );
    assert!(
        result.contains("_jsx(_components.div, { children: \"t\" })"),
        "JSX inside if-consequent must be lowered: {result}"
    );
    Ok(())
}

#[test]
fn jsx_in_default_export_expression() -> Result<(), satteri_arena::mdx_types::Message> {
    // `export default <Foo/>` should have the JSX expression lowered.
    let result = compile(
        "export default <r>x</r>;\n\n<p>hi</p>\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("_jsx(\"r\", { children: \"x\" })"),
        "JSX in default export expression must be lowered: {result}"
    );
    Ok(())
}

#[test]
fn function_body_simple() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "# Hi!",
        &Options {
            output_format: OutputFormat::FunctionBody,
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    assert!(
        result.starts_with("\"use strict\""),
        "should start with use strict: {result}"
    );
    assert!(
        !result.contains("import "),
        "should not contain import statements: {result}"
    );
    assert!(
        !result.contains("export default"),
        "should not contain export default: {result}"
    );
    assert!(
        result.contains("const { jsx: _jsx } = arguments[0]"),
        "should destructure from arguments[0]: {result}"
    );
    assert!(
        result.contains("default: MDXContent"),
        "should return default MDXContent: {result}"
    );
    Ok(())
}

#[test]
fn function_body_with_exports() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "export const name = \"world\";\n\n# Hello {name}",
        &Options {
            output_format: OutputFormat::FunctionBody,
            ..Default::default()
        },
        MDX_OPTS,
    )?;
    assert!(
        result.contains("const name = \"world\""),
        "should unwrap export declaration: {result}"
    );
    assert!(
        !result.contains("export const"),
        "should not contain export keyword: {result}"
    );
    assert!(
        result.contains("name") && result.contains("default: MDXContent"),
        "should include named export in return: {result}"
    );
    Ok(())
}

// Components defined at module scope (via export const / function / class,
// or via imports) must resolve to that binding rather than being destructured
// out of `props.components`. Mirrors `@mdx-js/mdx` (via `estree-util-scope`).
#[test]
fn module_scope_component_not_destructured() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "export const Comp = () => <span>Comp</span>\n\n<Comp />\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        !result.contains("{ Comp }"),
        "must not destructure `Comp` from components when it is bound at module scope: {result}"
    );
    assert!(
        !result.contains("_missingMdxReference(\"Comp\""),
        "must not emit a missing-ref guard for a module-bound component: {result}"
    );
    Ok(())
}

#[test]
fn module_scope_exported_function_component_not_destructured()
-> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "export function FnComp() { return <span /> }\n\n<FnComp />\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(!result.contains("{ FnComp }"), "{result}");
    assert!(
        !result.contains("_missingMdxReference(\"FnComp\""),
        "{result}"
    );
    Ok(())
}

#[test]
fn module_scope_exported_class_component_not_destructured()
-> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "export class ClassComp { render() { return null } }\n\n<ClassComp />\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(!result.contains("{ ClassComp }"), "{result}");
    assert!(
        !result.contains("_missingMdxReference(\"ClassComp\""),
        "{result}"
    );
    Ok(())
}

// Only identifiers without a module-scope binding should be destructured.
// `Comp` is exported locally; `Other` is imported; `NotInScope` has no binding.
#[test]
fn mixed_module_scope_and_dynamic_components() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "import Other from './other.jsx'\nexport const Comp = () => <span>Comp</span>\n\n<Comp /> and <Other /> and <NotInScope />\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("const { NotInScope } = _components;"),
        "only the unbound `NotInScope` should be destructured: {result}"
    );
    assert!(
        !result.contains("_missingMdxReference(\"Comp\""),
        "no missing-ref guard for module-bound `Comp`: {result}"
    );
    assert!(
        !result.contains("_missingMdxReference(\"Other\""),
        "no missing-ref guard for imported `Other`: {result}"
    );
    assert!(
        result.contains("_missingMdxReference(\"NotInScope\""),
        "still emit guard for unbound `NotInScope`: {result}"
    );
    Ok(())
}

#[test]
fn esm_parse_error_carries_source_position() {
    use satteri_arena::mdx_types::Place;

    // Invalid ESM (`export const x = ;`) on line 3. The oxc parse error must
    // resolve to a source point, not be dropped to `place: None`.
    let err = compile(
        "# Title\n\nexport const x = ;\n",
        &Options::default(),
        MDX_OPTS,
    )
    .expect_err("invalid ESM should fail to compile");

    let place = err
        .place
        .expect("parse error should carry a source position");
    match *place {
        Place::Point(point) => assert_eq!(
            point.line, 3,
            "error should point at the ESM line, got {point:?}"
        ),
        Place::Position(position) => assert_eq!(
            position.start.line, 3,
            "error should point at the ESM line, got {position:?}"
        ),
    }
}

#[test]
fn expression_parse_error_carries_source_position() {
    use satteri_arena::mdx_types::Place;

    // Invalid MDX expression (`{ 1 + }`) on line 3. Parse-time errors must
    // carry a source point too, not just a bare byte offset.
    let err = compile("# Title\n\n{ 1 + }\n", &Options::default(), MDX_OPTS)
        .expect_err("invalid expression should fail to compile");

    let place = err
        .place
        .expect("parse error should carry a source position");
    match *place {
        Place::Point(point) => assert_eq!(
            point.line, 3,
            "error should point at the expression line, got {point:?}"
        ),
        Place::Position(position) => assert_eq!(
            position.start.line, 3,
            "error should point at the expression line, got {position:?}"
        ),
    }
}

#[test]
fn jsx_attribute_expression_error_points_at_attribute() {
    use satteri_arena::mdx_types::Place;

    // The error is in the *second* attribute (`bad={2 *}`) on line 3. The
    // position must point inside that attribute, not at the element's `<`.
    let err = compile(
        "# T\n\n<Foo a={1} bad={2 *} />\n",
        &Options::default(),
        MDX_OPTS,
    )
    .expect_err("invalid attribute expression should fail to compile");

    let place = err
        .place
        .expect("parse error should carry a source position");
    let point = match *place {
        Place::Point(p) => p,
        Place::Position(p) => p.start,
    };
    assert_eq!(
        point.line, 3,
        "should be on the element's line, got {point:?}"
    );
    // `bad={…}` opens at column 16; column 1 would mean we regressed to
    // pointing at the element's `<` instead of the offending attribute.
    assert!(
        point.column >= 16,
        "should point inside the second attribute, got {point:?}"
    );
}

#[test]
fn top_level_await_makes_content_async() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "<ShowcaseCard site={await getEntry('showcase', 'a.dev')} />\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("async function _createMdxContent(props)"),
        "top-level `await` in content should make `_createMdxContent` async: {result}"
    );
    assert!(
        !result.contains("async function MDXContent"),
        "`MDXContent` should stay sync: {result}"
    );
    Ok(())
}

#[test]
fn await_inside_nested_function_stays_sync() -> Result<(), satteri_arena::mdx_types::Message> {
    let result = compile(
        "<A fn={async () => await getEntry()} />\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        result.contains("function _createMdxContent(props)")
            && !result.contains("async function _createMdxContent"),
        "`await` inside a nested async function should not make `_createMdxContent` async: {result}"
    );
    Ok(())
}

#[test]
fn multiline_jsx_attribute_expression_error_is_exact() {
    use satteri_arena::mdx_types::Place;

    // The offending token (`2` — a second operand with no operator) sits on a
    // continuation line that the expression dedent indents by two columns. The
    // position must point at the verbatim source column (5, in `  1 2`), not at
    // column 3 as it would if the dedented copy were validated instead.
    let point = |src: &str| -> satteri_arena::mdx_types::Point {
        let err = compile(src, &Options::default(), MDX_OPTS)
            .expect_err("invalid attribute expression should fail to compile");
        match *err
            .place
            .expect("parse error should carry a source position")
        {
            Place::Point(p) => p,
            Place::Position(p) => p.start,
        }
    };

    let p = point("# T\n\n<Foo bar={\n  1 2\n} />\n");
    assert_eq!(
        (p.line, p.column),
        (4, 5),
        "two-space indent: error must point at the verbatim column, got {p:?}"
    );

    // Same, with a deeper six-column indent: column 9, not column 7.
    let p = point("# T\n\n<Foo bar={\n      1 2\n} />\n");
    assert_eq!(
        (p.line, p.column),
        (4, 9),
        "six-space indent: error must point at the verbatim column, got {p:?}"
    );
}

#[test]
fn multiline_flow_expression_error_is_exact() {
    use satteri_arena::mdx_types::Place;

    // Block `{…}` expression spanning lines: the bad token `2` is on a
    // continuation line indented two columns. The position must be the
    // verbatim column 5, not the dedented column 3.
    let err = compile("# T\n\n{\n  1 2\n}\n", &Options::default(), MDX_OPTS)
        .expect_err("invalid flow expression should fail to compile");
    let point = match *err
        .place
        .expect("parse error should carry a source position")
    {
        Place::Point(p) => p,
        Place::Position(p) => p.start,
    };
    assert_eq!(
        (point.line, point.column),
        (4, 5),
        "flow expression error must point at the verbatim column, got {point:?}"
    );
}

#[test]
fn multiline_inline_expression_error_is_exact() {
    use satteri_arena::mdx_types::Place;

    let point = |src: &str| -> satteri_arena::mdx_types::Point {
        let err = compile(src, &Options::default(), MDX_OPTS)
            .expect_err("invalid inline expression should fail to compile");
        match *err
            .place
            .expect("parse error should carry a source position")
        {
            Place::Point(p) => p,
            Place::Position(p) => p.start,
        }
    };

    // Inline `{…}` spanning lines: the bad `2` sits on a continuation line
    // dedented two columns. The position must be the verbatim column 5.
    let p = point("para {\n  1 2\n} end\n");
    assert_eq!(
        (p.line, p.column),
        (2, 5),
        "inline expression error must point at the verbatim column, got {p:?}"
    );

    // Inside a blockquote: the continuation line is `>   1 2`, so the `2` is at
    // column 7. Getting this right requires mapping through both the stripped
    // `> ` container prefix and the dedent.
    let p = point("> para {\n>   1 2\n> } end\n");
    assert_eq!(
        (p.line, p.column),
        (2, 7),
        "inline expression error in a blockquote must account for the container \
         prefix, got {p:?}"
    );
}

#[test]
fn jsx_in_attribute_expression_is_lowered() -> Result<(), satteri_arena::mdx_types::Message> {
    // JSX nested in an attribute expression must be transformed to `_jsx(...)`
    // calls, exactly like JSX in children. It used to leak through un-lowered
    // as a raw JSX element in the output (`d: <_components.p>...`), producing
    // invalid JavaScript.
    let element = compile(
        "<Foo d={<p>hi there</p>} />\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        element.contains(r#"d: _jsx(_components.p, { children: "hi there" })"#),
        "element attr should be lowered, got:\n{element}"
    );

    let fragment = compile("<Foo d={<>hi</>} />\n", &Options::default(), MDX_OPTS)?;
    assert!(
        fragment.contains(r#"d: _jsx(_Fragment, { children: "hi" })"#),
        "fragment attr should be lowered, got:\n{fragment}"
    );

    let conditional = compile(
        "<Foo d={cond ? <a>x</a> : <b>y</b>} />\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        conditional.contains("_jsx(_components.a,") && conditional.contains("_jsx(_components.b,"),
        "JSX in a conditional attr value should be lowered, got:\n{conditional}"
    );

    // No raw JSX (`<_components.` / `<_Fragment`) may survive into the output.
    for out in [&element, &fragment, &conditional] {
        assert!(
            !out.contains("<_components.") && !out.contains("<_Fragment"),
            "raw JSX leaked into output:\n{out}"
        );
    }

    Ok(())
}

#[test]
fn apostrophe_in_jsx_text_after_close_tag() -> Result<(), satteri_arena::mdx_types::Message> {
    // End-to-end guard combining both halves of the fix: the scanner must run
    // past an apostrophe that follows a child close tag inside an attribute
    // expression (`</b>'s`), and the resulting JSX must then be lowered.
    let out = compile(
        "<Foo d={<p>a<b>x</b>'s</p>} />\n",
        &Options::default(),
        MDX_OPTS,
    )?;
    assert!(
        out.contains("_jsxs(_components.p,")
            && out.contains(r#"_jsx(_components.b, { children: "x" })"#)
            && out.contains(r#""'s""#),
        "apostrophe-after-close-tag attr expr should parse and lower, got:\n{out}"
    );
    assert!(
        !out.contains("<_components."),
        "raw JSX leaked into output:\n{out}"
    );

    // Quotes elsewhere in JSX text — after a `.` (`Corp.'s`) and a paired
    // double-quote (`"!?"`) — must likewise stay literal text, not open a JS
    // string. (These defeat a preceding-token heuristic; the scanner consumes
    // element children as text instead.)
    for src in [
        "<Foo d={<p>Acme Corp.'s view</p>} />\n",
        "<Foo d={<p>a \"!?\" badge here</p>} />\n",
    ] {
        let out = compile(src, &Options::default(), MDX_OPTS)?;
        assert!(
            out.contains("_jsx(_components.p,") && !out.contains("<_components."),
            "quote in JSX text should parse and lower: {src:?} ->\n{out}"
        );
    }

    Ok(())
}
