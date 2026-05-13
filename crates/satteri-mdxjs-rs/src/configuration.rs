//! Configuration.

use crate::mdx_plugin_recma_document::JsxRuntime;

/// Configuration for collapsing static HAST subtrees into raw HTML during
/// MDX compilation.
///
/// When `optimize_static` is set on [`Options`], fully-static HAST subtrees
/// (containing no MDX components, expressions, or dynamic content) are collapsed
/// into a single raw HTML string instead of nested `_jsx()` calls.
///
/// # Examples
///
/// React-style (`dangerouslySetInnerHTML`):
/// ```
/// # use satteri_mdxjs::OptimizeStaticConfig;
/// let config = OptimizeStaticConfig {
///     component: "div".into(),
///     prop: "dangerouslySetInnerHTML".into(),
///     wrap_prop_value: true,
///     ..Default::default()
/// };
/// ```
///
/// Astro-style (`set:html`):
/// ```
/// # use satteri_mdxjs::OptimizeStaticConfig;
/// let config = OptimizeStaticConfig {
///     component: "Fragment".into(),
///     prop: "set:html".into(),
///     ..Default::default()
/// };
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serializable", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serializable", serde(rename_all = "camelCase", default))]
pub struct OptimizeStaticConfig {
    /// The component or element name to wrap collapsed HTML in
    /// (e.g. `"Fragment"`, `"div"`, `"RawHtml"`).
    pub component: String,

    /// The prop/attribute name for the HTML string
    /// (e.g. `"set:html"`, `"dangerouslySetInnerHTML"`, `"html"`).
    pub prop: String,

    /// If `true`, the prop value is wrapped as `{ __html: "..." }` (React-style).
    /// If `false`, the prop value is a plain string literal.
    pub wrap_prop_value: bool,

    /// Element tag names to exclude from collapsing (e.g. `["h1", "p"]`).
    /// These elements will remain as JSX calls even in static subtrees,
    /// useful when they may be overridden by a component mapping at runtime.
    ///
    /// `export const components = { … }` declarations in the MDX source are
    /// auto-detected and their keys merged into this list.
    pub ignore_elements: Vec<String>,
}

impl Default for OptimizeStaticConfig {
    fn default() -> Self {
        Self {
            component: "Fragment".into(),
            prop: "set:html".into(),
            wrap_prop_value: false,
            ignore_elements: vec![],
        }
    }
}

/// Like `Constructs` from `markdown-rs`.
///
/// You can’t use:
///
/// *   `autolink`
/// *   `code_indented`
/// *   `html_flow`
/// *   `html_text`
/// *   `mdx_esm`
/// *   `mdx_expression_flow`
/// *   `mdx_expression_text`
/// *   `mdx_jsx_flow`
/// *   `mdx_jsx_text`
///
// To do: link all docs when `markdown-rs` is stable.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
#[cfg_attr(feature = "serializable", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serializable", serde(rename_all = "camelCase", default))]
pub struct MdxConstructs {
    pub attention: bool,
    pub block_quote: bool,
    pub character_escape: bool,
    pub character_reference: bool,
    pub code_fenced: bool,
    pub code_text: bool,
    pub definition: bool,
    pub frontmatter: bool,
    pub gfm_autolink_literal: bool,
    pub gfm_footnote_definition: bool,
    pub gfm_label_start_footnote: bool,
    pub gfm_strikethrough: bool,
    pub gfm_table: bool,
    pub gfm_task_list_item: bool,
    pub hard_break_escape: bool,
    pub hard_break_trailing: bool,
    pub heading_atx: bool,
    pub heading_setext: bool,
    pub label_start_image: bool,
    pub label_start_link: bool,
    pub label_end: bool,
    pub list_item: bool,
    pub math_flow: bool,
    pub math_text: bool,
    pub thematic_break: bool,
}

impl Default for MdxConstructs {
    /// MDX with `CommonMark`.
    ///
    /// `CommonMark` is a relatively strong specification of how markdown
    /// works.
    /// Most markdown parsers try to follow it.
    ///
    /// For more information, see the `CommonMark` specification:
    /// <https://spec.commonmark.org>.
    fn default() -> Self {
        Self {
            attention: true,
            block_quote: true,
            character_escape: true,
            character_reference: true,
            code_fenced: true,
            code_text: true,
            definition: true,
            frontmatter: false,
            gfm_autolink_literal: false,
            gfm_label_start_footnote: false,
            gfm_footnote_definition: false,
            gfm_strikethrough: false,
            gfm_table: false,
            gfm_task_list_item: false,
            hard_break_escape: true,
            hard_break_trailing: true,
            heading_atx: true,
            heading_setext: true,
            label_start_image: true,
            label_start_link: true,
            label_end: true,
            list_item: true,
            math_flow: false,
            math_text: false,
            thematic_break: true,
        }
    }
}

impl MdxConstructs {
    /// MDX with GFM.
    ///
    /// GFM stands for **GitHub flavored markdown**.
    /// GFM extends `CommonMark` and adds support for autolink literals,
    /// footnotes, strikethrough, tables, and tasklists.
    ///
    /// For more information, see the GFM specification:
    /// <https://github.github.com/gfm/>.
    #[must_use]
    pub fn gfm() -> Self {
        Self {
            gfm_autolink_literal: true,
            gfm_footnote_definition: true,
            gfm_label_start_footnote: true,
            gfm_strikethrough: true,
            gfm_table: true,
            gfm_task_list_item: true,
            ..Self::default()
        }
    }
}

// To do: link all docs when `markdown-rs` is stable.
/// Like `ParseOptions` from `markdown-rs`.
///
/// The constructs you can pass are limited.
///
/// Additionally, you can’t use:
///
/// *   `mdx_expression_parse`
/// *   `mdx_esm_parse`
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serializable", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serializable", serde(rename_all = "camelCase", default))]
pub struct MdxParseOptions {
    pub constructs: MdxConstructs,
    pub gfm_strikethrough_single_tilde: bool,
    pub math_text_single_dollar: bool,
}

impl Default for MdxParseOptions {
    /// MDX with `CommonMark` defaults.
    fn default() -> Self {
        Self {
            constructs: MdxConstructs::default(),
            gfm_strikethrough_single_tilde: true,
            math_text_single_dollar: true,
        }
    }
}

impl MdxParseOptions {
    /// MDX with GFM.
    ///
    /// GFM stands for GitHub flavored markdown.
    /// GFM extends `CommonMark` and adds support for autolink literals,
    /// footnotes, strikethrough, tables, and tasklists.
    ///
    /// For more information, see the GFM specification:
    /// <https://github.github.com/gfm/>
    #[must_use]
    pub fn gfm() -> Self {
        Self {
            constructs: MdxConstructs::gfm(),
            ..Self::default()
        }
    }
}

/// Configuration (optional).
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serializable", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serializable", serde(rename_all = "camelCase", default))]
pub struct Options {
    /// Configuration that describes how to parse from markdown.
    pub parse: MdxParseOptions,

    /// Whether to add extra information to error messages in generated code
    /// (default: `false`).
    ///
    /// When in the automatic JSX runtime, this also enabled its development
    /// functionality.
    pub development: bool,

    // To do: some alternative to generate source maps.
    // SourceMapGenerator
    /// Place to import a provider from (default: `None`, example:
    /// `Some("@mdx-js/react").into()`).
    ///
    /// Useful for runtimes that support context (React, Preact).
    /// The provider must export a `useMDXComponents`, which is called to
    /// access an object of components.
    pub provider_import_source: Option<String>,

    /// Whether to keep JSX (default: `false`).
    ///
    /// The default is to compile JSX away so that the resulting file is
    /// immediately runnable.
    pub jsx: bool,

    /// JSX runtime to use (default: `Some(JsxRuntime::Automatic)`).
    ///
    /// The classic runtime compiles to calls such as `h('p')`, the automatic
    /// runtime compiles to
    /// `import _jsx from '$importSource/jsx-runtime'\n_jsx('p')`.
    pub jsx_runtime: Option<JsxRuntime>,

    /// Place to import automatic JSX runtimes from (`Option<String>`, default:
    /// `Some("react".into())`).
    ///
    /// When in the automatic runtime, this is used to define an import for
    /// `_Fragment`, `_jsx`, and `_jsxs`.
    pub jsx_import_source: Option<String>,

    /// Pragma for JSX (default: `Some("React.createElement".into())`).
    ///
    /// When in the classic runtime, this is used as an identifier for function
    /// calls: `<x />` to `React.createElement('x')`.
    ///
    /// You should most probably define `pragma_frag` and `pragma_import_source`
    /// too when changing this.
    pub pragma: Option<String>,

    /// Pragma for JSX fragments (default: `Some("React.Fragment".into())`).
    ///
    /// When in the classic runtime, this is used as an identifier for
    /// fragments: `<>` to `React.createElement(React.Fragment)`.
    ///
    /// You should most probably define `pragma` and `pragma_import_source`
    /// too when changing this.
    pub pragma_frag: Option<String>,

    /// Where to import the identifier of `pragma` from (default:
    /// `Some("react".into())`).
    ///
    /// When in the classic runtime, this is used to import the `pragma`
    /// function.
    /// To illustrate with an example: when `pragma` is `"a.b"` and
    /// `pragma_import_source` is `"c"`, the following will be generated:
    /// `import a from 'c'`.
    pub pragma_import_source: Option<String>,

    // New:
    /// File path to the source file (example:
    /// `Some("path/to/example.mdx".into())`).
    ///
    /// Used when `development: true` to improve error messages.
    pub filepath: Option<String>,

    /// Static subtree optimization (default: `None`, disabled).
    ///
    /// When enabled, fully-static subtrees are collapsed into raw HTML strings
    /// instead of nested `_jsx()` calls, reducing JS output size and runtime
    /// overhead. See [`OptimizeStaticConfig`] for configuration.
    pub optimize_static: Option<OptimizeStaticConfig>,

    /// Output format (default: `OutputFormat::Program`).
    ///
    /// - `Program`: ES module with `import`/`export` statements.
    /// - `FunctionBody`: Function body that reads runtime from `arguments[0]`
    ///   and returns `{ default: MDXContent, ...exports }`.
    pub output_format: OutputFormat,

    /// Casing for HTML/SVG attribute names on plain (rehype-produced)
    /// elements (default: [`ElementAttributeNameCase::React`]).
    ///
    /// Does not affect attributes on user-written MDX JSX; those are emitted
    /// as the author wrote them.
    pub element_attribute_name_case: ElementAttributeNameCase,

    /// Casing for keys in `style` objects parsed from `style="…"` strings
    /// on plain (rehype-produced) elements (default:
    /// [`StylePropertyNameCase::Dom`]).
    pub style_property_name_case: StylePropertyNameCase,
}

/// Output format for MDX compilation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serializable", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serializable", serde(rename_all = "camelCase"))]
pub enum OutputFormat {
    #[default]
    Program,
    FunctionBody,
}

/// Casing for HTML/SVG attribute names emitted on plain elements (those
/// produced from markdown / rehype, not user-written MDX JSX).
///
/// - `React` (default): `className`, `htmlFor`, `strokeLinecap`, `xmlLang`
/// - `Html`: `class`, `for`, `stroke-linecap`, `xml:lang`
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serializable", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serializable", serde(rename_all = "camelCase"))]
pub enum ElementAttributeNameCase {
    #[default]
    React,
    Html,
}

/// Casing for keys in `style={{…}}` objects parsed from `style="…"`
/// attributes on plain elements.
///
/// - `Dom` (default): `backgroundColor`, `WebkitLineClamp`
/// - `Css`: `background-color`, `-webkit-line-clamp`
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serializable", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serializable", serde(rename_all = "camelCase"))]
pub enum StylePropertyNameCase {
    #[default]
    Dom,
    Css,
}

impl Default for Options {
    /// Default options to use the automatic JSX runtime with React
    /// and handle MDX according to `CommonMark`.
    fn default() -> Self {
        Self {
            parse: MdxParseOptions::default(),
            development: false,
            provider_import_source: None,
            jsx: false,
            jsx_runtime: Some(JsxRuntime::default()),
            jsx_import_source: None,
            pragma: None,
            pragma_frag: None,
            pragma_import_source: None,
            filepath: None,
            optimize_static: None,
            output_format: OutputFormat::default(),
            element_attribute_name_case: ElementAttributeNameCase::default(),
            style_property_name_case: StylePropertyNameCase::default(),
        }
    }
}

impl Options {
    /// MDX with GFM.
    ///
    /// GFM stands for GitHub flavored markdown.
    /// GFM extends `CommonMark` and adds support for autolink literals,
    /// footnotes, strikethrough, tables, and tasklists.
    /// On the compilation side, GFM turns on the GFM tag filter.
    /// The tagfilter is useless, but it’s included here for consistency.
    ///
    /// For more information, see the GFM specification:
    /// <https://github.github.com/gfm/>
    #[must_use]
    pub fn gfm() -> Self {
        Self {
            parse: MdxParseOptions::gfm(),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructs() {
        let constructs = MdxConstructs::default();
        assert!(constructs.attention, "should default to `CommonMark` (1)");
        assert!(
            !constructs.gfm_autolink_literal,
            "should default to `CommonMark` (2)"
        );
        assert!(
            !constructs.frontmatter,
            "should default to `CommonMark` (3)"
        );

        let constructs = MdxConstructs::gfm();
        assert!(constructs.attention, "should support `gfm` shortcut (1)");
        assert!(
            constructs.gfm_autolink_literal,
            "should support `gfm` shortcut (2)"
        );
        assert!(!constructs.frontmatter, "should support `gfm` shortcut (3)");
    }

    #[test]
    fn test_parse_options() {
        let options = MdxParseOptions::default();
        assert!(
            options.constructs.attention,
            "should default to `CommonMark` (1)"
        );
        assert!(
            !options.constructs.gfm_autolink_literal,
            "should default to `CommonMark` (2)"
        );
        assert!(
            !options.constructs.frontmatter,
            "should default to `CommonMark` (3)"
        );

        let options = MdxParseOptions::gfm();
        assert!(
            options.constructs.attention,
            "should support `gfm` shortcut (1)"
        );
        assert!(
            options.constructs.gfm_autolink_literal,
            "should support `gfm` shortcut (2)"
        );
        assert!(
            !options.constructs.frontmatter,
            "should support `gfm` shortcut (3)"
        );
    }

    #[test]
    fn test_options() {
        let options = Options::default();
        assert!(
            options.parse.constructs.attention,
            "should default to `CommonMark` (1)"
        );
        assert!(
            !options.parse.constructs.gfm_autolink_literal,
            "should default to `CommonMark` (2)"
        );
        assert!(
            !options.parse.constructs.frontmatter,
            "should default to `CommonMark` (3)"
        );

        let options = Options::gfm();
        assert!(
            options.parse.constructs.attention,
            "should support `gfm` shortcut (1)"
        );
        assert!(
            options.parse.constructs.gfm_autolink_literal,
            "should support `gfm` shortcut (2)"
        );
        assert!(
            !options.parse.constructs.frontmatter,
            "should support `gfm` shortcut (3)"
        );
    }
}
