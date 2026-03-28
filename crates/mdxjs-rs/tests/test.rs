extern crate mdxjs;
use mdxjs::{JsxRuntime, OptimizeStaticConfig, Options, compile};
use pretty_assertions::assert_eq;

#[test]
fn simple() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    assert_eq!(
        compile("", &Options::default())?,
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
fn development() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    assert_eq!(
        compile("<A />", &Options {
            development: true,
            filepath: Some("example.mdx".into()),
            ..Default::default()
        })?,
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
fn provider() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    assert_eq!(
        compile("<A />",  &Options {
            provider_import_source: Some("@mdx-js/react".into()),
            ..Default::default()
        })?,
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
fn jsx() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    assert_eq!(
        compile("", &Options {
            jsx: true,
            ..Default::default()
        })?,
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
fn classic() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    assert_eq!(
        compile("", &Options {
            jsx_runtime: Some(JsxRuntime::Classic),
            ..Default::default()
        })?,
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
fn import_source() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    assert_eq!(
        compile(
            "",
            &Options {
                jsx_import_source: Some("preact".into()),
                ..Default::default()
            }
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
fn pragmas() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    assert_eq!(
        compile("", &Options {
            jsx_runtime: Some(JsxRuntime::Classic),
            pragma: Some("a.b".into()),
            pragma_frag: Some("a.c".into()),
            pragma_import_source: Some("d".into()),
            ..Default::default()
        })?,
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
fn unravel_elements() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    let result = compile("<x>a</x>\n<x>\n  b\n</x>\n", &Default::default())?;
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
fn unravel_expressions() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    assert_eq!(
        compile("{1} {2}", &Default::default())?,
        "import { Fragment as _Fragment, jsx as _jsx, jsxs as _jsxs } from \"react/jsx-runtime\";
function _createMdxContent(props) {
    return _jsxs(_Fragment, { children: [
        1,
        \"\\n\",
        \" \",
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
fn explicit_jsx() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    assert_eq!(
        compile(
            "<h1>asd</h1>
# qwe
",
            &Default::default()
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

// ---------------------------------------------------------------------------
// optimize_static tests
// ---------------------------------------------------------------------------

#[test]
fn optimize_static_default_off() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    // With no optimize_static, output should contain _jsx("h1", ...) calls.
    let result = compile("# Hello\n\nWorld", &Options::default())?;
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
fn optimize_static_astro_style() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    let result = compile(
        "# Hello\n\nWorld",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
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
fn optimize_static_react_style() -> Result<(), tryckeri_mdast::mdx_types::Message> {
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
fn optimize_static_mixed_dynamic() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    // Static content + dynamic MDX component — static parts should be collapsed,
    // dynamic parts should remain as JSX.
    let result = compile(
        "# Hello\n\n<Component />\n\nWorld",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
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
fn optimize_static_ignore_elements() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    let result = compile(
        "# Hello\n\nWorld",
        &Options {
            optimize_static: Some(OptimizeStaticConfig {
                ignore_elements: vec!["h1".into()],
                ..Default::default()
            }),
            ..Default::default()
        },
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
fn optimize_static_sibling_grouping() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    // Multiple consecutive static elements should be grouped into one set:html
    let result = compile(
        "# A\n\n## B\n\n### C",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
    )?;
    // All three headings should be in a single set:html
    assert!(result.contains("<h1>A</h1>"), "should contain h1: {result}");
    assert!(result.contains("<h2>B</h2>"), "should contain h2: {result}");
    assert!(result.contains("<h3>C</h3>"), "should contain h3: {result}");
    // Count occurrences of set:html — should be exactly 1 for fully static content
    let count = result.matches("set:html").count();
    assert_eq!(count, 1, "should have exactly one set:html group: {result}");
    Ok(())
}

#[test]
fn optimize_static_nested_dynamic_prevents_collapse()
-> Result<(), tryckeri_mdast::mdx_types::Message> {
    // A paragraph with an inline expression cannot be collapsed
    let result = compile(
        "# Static\n\nHello {name} world",
        &Options {
            optimize_static: Some(OptimizeStaticConfig::default()),
            ..Default::default()
        },
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
