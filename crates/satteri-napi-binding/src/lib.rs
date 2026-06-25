#![deny(clippy::all)]
// type_complexity: napi-derive generates code from the literal field types
// to drive its TS-binding output, so the `Option<Either<String, FunctionRef<…>>>`
// pattern can't be aliased away. Allow it crate-wide.
#![allow(clippy::type_complexity)]

use napi::bindgen_prelude::*;
use napi_derive::napi;

// Parsing feature flags (JS-facing)

/// Granular smart-punctuation toggles.
#[napi(object)]
pub struct JsSmartPunctuationOptions {
    /// Replace straight quotes with curly/smart quotes. Default: true.
    pub quotes: Option<bool>,
    /// Replace `--`/`---` with en-dash/em-dash. Default: true.
    pub dashes: Option<bool>,
    /// Replace `...` with ellipsis (`…`). Default: true.
    pub ellipses: Option<bool>,
}

/// Granular math toggles, nested under `features.math`.
#[napi(object)]
pub struct JsMathOptions {
    /// Treat single-dollar runs (`$ ... $`) as inline math. Default: true.
    /// Set `false` to keep single `$` as literal text (prose with currency)
    /// while still parsing double-dollar (`$$ ... $$`) display math.
    pub single_dollar_text_math: Option<bool>,
}

/// Granular GFM toggles, nested under `features.gfm`. The footnote i18n
/// strings (label, back-content, back-label) travel separately via the
/// `JsConvertOptions` argument on conversion entry points; the JS package
/// extracts them from `features.gfm.footnotes` before calling in.
#[napi(object)]
pub struct JsGfmOptions {
    /// Enable GFM footnotes (`[^id]`). Default: true. Set `false` to drop
    /// footnote parsing while keeping the rest of the GFM bundle.
    pub footnotes: Option<bool>,
}

/// Feature toggles for the Markdown/MDX parser, passed from JavaScript.
#[napi(object)]
pub struct JsFeatures {
    /// GFM: tables, footnotes, strikethrough, task lists. Default: true.
    pub gfm: Option<bool>,
    /// Granular GFM control (overrides `gfm`).
    pub gfm_options: Option<JsGfmOptions>,
    /// Frontmatter: YAML (`--- ... ---`) and TOML (`+++ ... +++`). Default: true.
    pub frontmatter: Option<bool>,
    /// Math blocks and inline math (`$$ ... $$`, `$ ... $`). Default: false.
    pub math: Option<bool>,
    /// Granular math control (overrides `math`).
    pub math_options: Option<JsMathOptions>,
    /// Heading attributes (`# text { #id .class }`). Default: false.
    pub heading_attributes: Option<bool>,
    /// Colon-delimited container directive blocks (`:::`). Default: false.
    pub directive: Option<bool>,
    /// Superscript (`^super^`). Default: false.
    pub superscript: Option<bool>,
    /// Subscript (`~sub~`). Default: false.
    pub subscript: Option<bool>,
    /// Obsidian-style wikilinks (`[[link]]`). Default: false.
    pub wikilinks: Option<bool>,
    /// Smart punctuation: all categories on. Default: false.
    pub smart_punctuation: Option<bool>,
    /// Granular smart-punctuation control (overrides `smart_punctuation`).
    pub smart_punctuation_options: Option<JsSmartPunctuationOptions>,
}

fn features_to_options(features: Option<JsFeatures>, mdx: bool) -> satteri_pulldown_cmark::Options {
    use satteri_pulldown_cmark::Options;

    let f = features.unwrap_or(JsFeatures {
        gfm: None,
        gfm_options: None,
        frontmatter: None,
        math: None,
        math_options: None,
        heading_attributes: None,
        directive: None,
        superscript: None,
        subscript: None,
        wikilinks: None,
        smart_punctuation: None,
        smart_punctuation_options: None,
    });

    let mut opts = Options::empty();

    let (gfm_enabled, footnotes_enabled) = match &f.gfm_options {
        Some(g) => (f.gfm.unwrap_or(true), g.footnotes.unwrap_or(true)),
        None => (f.gfm.unwrap_or(true), true),
    };
    if gfm_enabled {
        opts |= Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_GFM;
        if footnotes_enabled {
            opts |= Options::ENABLE_FOOTNOTES;
        }
    }
    if f.frontmatter.unwrap_or(true) {
        opts |= Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
            | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS;
    }
    // Math is on when `math: true` or when a granular options object is passed
    // (the object overrides the umbrella toggle). `single_dollar_text_math`
    // then picks between umbrella-mode (single + multi) and multi-only: opting
    // out of single-dollar sets the multi-dollar sub-flag directly so the
    // parser skips lone `$` entirely.
    if f.math_options.is_some() || f.math.unwrap_or(false) {
        match f
            .math_options
            .as_ref()
            .and_then(|m| m.single_dollar_text_math)
        {
            Some(false) => opts |= Options::ENABLE_MATH_MULTI_DOLLAR,
            _ => opts |= Options::ENABLE_MATH,
        }
    }
    if f.heading_attributes.unwrap_or(false) {
        opts |= Options::ENABLE_HEADING_ATTRIBUTES;
    }
    if f.directive.unwrap_or(false) {
        opts |= Options::ENABLE_DIRECTIVE;
    }
    if f.superscript.unwrap_or(false) {
        opts |= Options::ENABLE_SUPERSCRIPT;
    }
    if f.subscript.unwrap_or(false) {
        opts |= Options::ENABLE_SUBSCRIPT;
    }
    if f.wikilinks.unwrap_or(false) {
        opts |= Options::ENABLE_WIKILINKS;
    }
    if let Some(sp) = f.smart_punctuation_options {
        if sp.quotes.unwrap_or(true) {
            opts |= Options::ENABLE_SMART_QUOTES;
        }
        if sp.dashes.unwrap_or(true) {
            opts |= Options::ENABLE_SMART_DASHES;
        }
        if sp.ellipses.unwrap_or(true) {
            opts |= Options::ENABLE_SMART_ELLIPSES;
        }
    } else if f.smart_punctuation.unwrap_or(false) {
        opts |= Options::ENABLE_SMART_PUNCTUATION;
    }
    if mdx {
        opts |= Options::ENABLE_MDX;
    }
    opts
}

// MDX compilation options (JS-facing)

/// Static optimization config passed from JavaScript.
#[cfg(feature = "mdx")]
#[napi(object)]
pub struct JsOptimizeStaticConfig {
    /// Component/element name to wrap collapsed HTML in (e.g. "Fragment", "div").
    pub component: String,
    /// Prop name for the HTML string (e.g. "set:html", "dangerouslySetInnerHTML").
    pub prop: String,
    /// If true, prop value is wrapped as `{ __html: "..." }` (React-style).
    pub wrap_prop_value: Option<bool>,
    /// Element tag names to exclude from collapsing.
    pub ignore_elements: Option<Vec<String>>,
}

/// MDX compile options passed from JavaScript.
#[cfg(feature = "mdx")]
#[napi(object)]
pub struct JsMdxOptions {
    /// Static subtree optimization. If provided, static subtrees are collapsed
    /// into raw HTML strings using the specified component and prop.
    pub optimize_static: Option<JsOptimizeStaticConfig>,
    /// Place to import automatic JSX runtimes from (e.g. "react", "preact").
    /// Default: "react".
    pub jsx_import_source: Option<String>,
    /// Whether to keep JSX instead of compiling it away. Default: false.
    pub jsx: Option<bool>,
    /// JSX runtime: "automatic" (default) or "classic".
    pub jsx_runtime: Option<String>,
    /// Whether to add extra info to error messages and use the development
    /// JSX runtime. Default: false.
    pub development: Option<bool>,
    /// Place to import a provider from (e.g. "@mdx-js/react").
    pub provider_import_source: Option<String>,
    /// Pragma for JSX in classic runtime (default: "React.createElement").
    pub pragma: Option<String>,
    /// Pragma for JSX fragments in classic runtime (default: "React.Fragment").
    pub pragma_frag: Option<String>,
    /// Where to import the pragma from in classic runtime (default: "react").
    pub pragma_import_source: Option<String>,
    /// Output format: "program" (default) or "function-body".
    pub output_format: Option<String>,
    /// Casing for HTML/SVG attribute names on plain (rehype-produced)
    /// elements. "react" (default) emits `className`, `htmlFor`, etc.;
    /// "html" emits `class`, `for`, `stroke-linecap`, etc.
    pub element_attribute_name_case: Option<String>,
    /// Casing for keys in `style` objects parsed from `style="…"` strings.
    /// "dom" (default) emits `{backgroundColor: …}`; "css" emits
    /// `{"background-color": …}`.
    pub style_property_name_case: Option<String>,
}

/// MDAST→HAST conversion options passed from JavaScript.
///
/// Input-only: `object_to_js = false` because `FunctionRef` only crosses
/// JS → Rust. A `JsConvertOptions` never gets serialized back to JS.
#[napi(object, object_to_js = false)]
pub struct JsConvertOptions {
    /// `<h2>` label opening the footnotes section. Default: `"Footnotes"`.
    pub footnote_label: Option<String>,
    /// Backref `<a>` content. Default: `"\u{21a9}"` (↩).
    pub footnote_back_content: Option<Either<String, FunctionRef<FnArgs<(u32, u32)>, String>>>,
    /// Backref `aria-label`. The token `{reference}` is replaced with the
    /// footnote number (`1`) or `number-K` (`1-2`) for repeated references.
    /// Default: `"Back to reference {reference}"`.
    pub footnote_back_label: Option<Either<String, FunctionRef<FnArgs<(u32, u32)>, String>>>,
}

fn js_backref_to_rust(
    env: Env,
    v: Either<String, FunctionRef<FnArgs<(u32, u32)>, String>>,
) -> satteri_ast::hast::Backref {
    match v {
        Either::A(s) => satteri_ast::hast::Backref::Template(s),
        Either::B(f) => satteri_ast::hast::Backref::Callback(Box::new(move |n, k| {
            // Fail-soft: callback errors → empty string. Conversion can't
            // return Result, and panicking would unwind across the FFI
            // boundary into UB.
            f.borrow_back(&env)
                .and_then(|callable| callable.call(FnArgs::from((n as u32, k as u32))))
                .unwrap_or_default()
        })),
    }
}

fn js_convert_options_to_rust(
    env: Env,
    opts: Option<JsConvertOptions>,
) -> satteri_ast::hast::ConvertOptions {
    let mut out = satteri_ast::hast::ConvertOptions::default();
    if let Some(js) = opts {
        if let Some(v) = js.footnote_label {
            out.footnote_label = v;
        }
        if let Some(v) = js.footnote_back_content {
            out.footnote_back_content = js_backref_to_rust(env, v);
        }
        if let Some(v) = js.footnote_back_label {
            out.footnote_back_label = js_backref_to_rust(env, v);
        }
    }
    out
}

#[cfg(feature = "mdx")]
fn js_options_to_rust(opts: Option<JsMdxOptions>) -> satteri_mdxjs::Options {
    let mut options = satteri_mdxjs::Options::default();
    if let Some(js) = opts {
        if let Some(config) = js.optimize_static {
            options.optimize_static = Some(satteri_mdxjs::OptimizeStaticConfig {
                component: config.component,
                prop: config.prop,
                wrap_prop_value: config.wrap_prop_value.unwrap_or(false),
                ignore_elements: config.ignore_elements.unwrap_or_default(),
            });
        }
        if let Some(src) = js.jsx_import_source {
            options.jsx_import_source = Some(src);
        }
        if let Some(val) = js.jsx {
            options.jsx = val;
        }
        if let Some(rt) = js.jsx_runtime {
            options.jsx_runtime = Some(match rt.as_str() {
                "classic" => satteri_mdxjs::JsxRuntime::Classic,
                _ => satteri_mdxjs::JsxRuntime::Automatic,
            });
        }
        if let Some(val) = js.development {
            options.development = val;
        }
        if let Some(src) = js.provider_import_source {
            options.provider_import_source = Some(src);
        }
        if let Some(val) = js.pragma {
            options.pragma = Some(val);
        }
        if let Some(val) = js.pragma_frag {
            options.pragma_frag = Some(val);
        }
        if let Some(val) = js.pragma_import_source {
            options.pragma_import_source = Some(val);
        }
        if let Some(fmt) = js.output_format {
            options.output_format = match fmt.as_str() {
                "function-body" => satteri_mdxjs::OutputFormat::FunctionBody,
                _ => satteri_mdxjs::OutputFormat::Program,
            };
        }
        if let Some(case) = js.element_attribute_name_case {
            options.element_attribute_name_case = match case.as_str() {
                "html" => satteri_mdxjs::ElementAttributeNameCase::Html,
                _ => satteri_mdxjs::ElementAttributeNameCase::React,
            };
        }
        if let Some(case) = js.style_property_name_case {
            options.style_property_name_case = match case.as_str() {
                "css" => satteri_mdxjs::StylePropertyNameCase::Css,
                _ => satteri_mdxjs::StylePropertyNameCase::Dom,
            };
        }
    }
    options
}

// MDX compilation

/// Compile MDX source directly to JavaScript.
#[cfg(feature = "mdx")]
#[napi]
pub fn compile_mdx(
    env: Env,
    source: String,
    options: Option<JsMdxOptions>,
    features: Option<JsFeatures>,
    convert_options: Option<JsConvertOptions>,
) -> Result<String> {
    let opts = js_options_to_rust(options);
    let parse_opts = features_to_options(features, true);
    let convert_opts = js_convert_options_to_rust(env, convert_options);
    satteri_mdxjs::compile_with_convert_options(&source, &opts, parse_opts, &convert_opts)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

// Direct rendering (no handle needed)

/// Parse Markdown source and return HTML string directly.
#[napi]
pub fn parse_to_html(
    env: Env,
    source: String,
    features: Option<JsFeatures>,
    convert_options: Option<JsConvertOptions>,
) -> Result<String> {
    let opts = features_to_options(features, false);
    let convert_opts = js_convert_options_to_rust(env, convert_options);
    let (arena, _) = satteri_pulldown_cmark::parse(&source, opts);
    Ok(satteri_ast::mdast_to_html_with_options(
        &arena,
        &convert_opts,
    ))
}

// Handle-based API: arena stays in Rust, no buffer copies to JS.
//
// MDAST and HAST arenas use distinct external types (`MdastHandle` /
// `HastHandle`) so napi-rs catches mismatches on kind-sensitive entry
// points like `render_handle` (HAST-only) or `apply_commands_to_mdast_handle`
// (MDAST-only) at runtime via External TypeId checks.
//
// Kind-agnostic ops (read source, drop, (de)serialize, get/set node data)
// take `Either<&MdastHandle, &HastHandle>` so JS callers don't need
// duplicated entry points for operations whose Rust body is identical
// across kinds.

use napi::bindgen_prelude::Either;
use std::sync::Mutex;

use satteri_arena::{Hast, Mdast};

type MdastHandle = External<Mutex<satteri_arena::Arena<Mdast>>>;
type HastHandle = External<Mutex<satteri_arena::Arena<Hast>>>;
type AnyHandle<'a> = Either<&'a MdastHandle, &'a HastHandle>;

fn make_parse_fn(mdx: bool, parse_options: u32) -> impl Fn(&str) -> satteri_arena::Arena<Mdast> {
    move |source: &str| -> satteri_arena::Arena<Mdast> {
        let opts = satteri_pulldown_cmark::Options::from_bits_truncate(parse_options);
        let (mut parsed, _errors) = satteri_pulldown_cmark::parse(source, opts);
        parsed.mdx = mdx;
        parsed.parse_options = parse_options;
        parsed
    }
}

/// A subscription passed from JS.
#[napi(object)]
pub struct JsSubscription {
    pub node_type: u8,
    pub tag_filter: Vec<String>,
}

/// Parse markdown source into an MDAST arena handle.
#[napi]
pub fn create_mdast_handle(source: String, features: Option<JsFeatures>) -> Result<MdastHandle> {
    let opts = features_to_options(features, false);
    let (mut arena, _) = satteri_pulldown_cmark::parse(&source, opts);
    arena.mdx = false;
    arena.parse_options = opts.bits();
    Ok(External::new(Mutex::new(arena)))
}

/// Parse MDX source into an MDAST arena handle.
#[cfg(feature = "mdx")]
#[napi]
pub fn create_mdx_mdast_handle(
    source: String,
    features: Option<JsFeatures>,
) -> Result<MdastHandle> {
    let opts = features_to_options(features, true);
    let (mut arena, mdx_errors) = satteri_pulldown_cmark::parse(&source, opts);
    if let Some((offset, msg)) = mdx_errors.first() {
        return Err(napi::Error::from_reason(
            satteri_mdxjs::parse_error_to_message(&source, *offset, msg).to_string(),
        ));
    }
    arena.mdx = true;
    arena.parse_options = opts.bits();
    Ok(External::new(Mutex::new(arena)))
}

/// Frontmatter extracted from an MDAST arena.
#[napi(object)]
pub struct JsFrontmatter {
    /// Either `"yaml"` or `"toml"`.
    pub kind: String,
    /// Raw frontmatter content between the delimiters (no `---`/`+++` lines).
    pub value: String,
}

/// Return the first YAML or TOML frontmatter block in the MDAST arena, if any.
/// Walks the root node's direct children and returns the first yaml/toml literal.
#[napi]
pub fn get_mdast_frontmatter(handle: &MdastHandle) -> Result<Option<JsFrontmatter>> {
    use satteri_arena::StringRef;
    use satteri_ast::mdast::node::MdastNodeType;

    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    if arena.is_empty() {
        return Ok(None);
    }
    let root = arena.get_node(0);
    let children_start = root.children_start as usize;
    let children_end = children_start + root.children_count as usize;
    for i in children_start..children_end {
        let child_id = arena.children[i];
        let node = arena.get_node(child_id);
        let kind = match MdastNodeType::from_u8(node.node_type) {
            Some(MdastNodeType::Yaml) => "yaml",
            Some(MdastNodeType::Toml) => "toml",
            _ => continue,
        };
        let type_data = arena.get_type_data(child_id);
        if type_data.len() < 8 {
            continue;
        }
        let sr = StringRef::from_bytes(&type_data[0..8]);
        let value = arena.get_str(sr).to_string();
        return Ok(Some(JsFrontmatter {
            kind: kind.to_string(),
            value,
        }));
    }
    Ok(None)
}

/// Serialize a handle's arena to the wire-format buffer JS instantiates a
/// reader from. The kind tag in the header tells the JS side whether to
/// pick `MdastReader` or `HastReader`.
#[napi]
pub fn serialize_handle(handle: AnyHandle) -> Result<Uint8Array> {
    let buf = match handle {
        Either::A(h) => h
            .lock()
            .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?
            .to_raw_buffer(),
        Either::B(h) => h
            .lock()
            .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?
            .to_raw_buffer(),
    };
    Ok(Uint8Array::new(buf))
}

/// Get the source string from a handle. Kind-agnostic: source is the
/// original markdown/MDX input and is identical across MDAST and HAST.
#[napi]
pub fn get_handle_source(handle: AnyHandle) -> Result<String> {
    let s = match handle {
        Either::A(h) => h
            .lock()
            .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?
            .source()
            .to_string(),
        Either::B(h) => h
            .lock()
            .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?
            .source()
            .to_string(),
    };
    Ok(s)
}

/// Set the `data` blob (JSON bytes) for a node. Works for both MDAST and
/// HAST handles — `node_data` is a per-node JSON blob with no kind-specific
/// shape on the Rust side.
#[napi]
pub fn set_node_data(handle: AnyHandle, node_id: u32, json: Uint8Array) -> Result<()> {
    match handle {
        Either::A(h) => h
            .lock()
            .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?
            .set_node_data(node_id, json.to_vec()),
        Either::B(h) => h
            .lock()
            .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?
            .set_node_data(node_id, json.to_vec()),
    }
    Ok(())
}

/// Walk an MDAST handle's arena and return matched nodes as a flat binary buffer.
#[napi]
pub fn walk_mdast_handle(
    handle: &MdastHandle,
    subscriptions: Vec<JsSubscription>,
) -> Result<Uint8Array> {
    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    let subs: Vec<satteri_ast::walk::Subscription> = subscriptions
        .into_iter()
        .map(|s| satteri_ast::walk::Subscription {
            node_type: s.node_type,
            tag_filter: s.tag_filter,
        })
        .collect();
    Ok(Uint8Array::new(satteri_ast::walk::walk_mdast(
        &arena, &subs,
    )))
}

/// Apply a command buffer to an MDAST handle in-place. Returns how many patches
/// were dropped because their target lived inside a subtree this pass removed or
/// replaced (see the lenient note below); the JS pipeline warns when non-zero.
#[napi]
pub fn apply_commands_to_mdast_handle(
    handle: &MdastHandle,
    command_buf: Uint8Array,
) -> Result<u32> {
    let mut arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    let mdx = arena.mdx;
    let parse_markdown = make_parse_fn(mdx, arena.parse_options);
    let owned = std::mem::replace(
        &mut *arena,
        satteri_arena::Arena::<Mdast>::new(String::new()),
    );
    // Lenient: a patch stranded inside a subtree the same pass replaced or
    // removed is dropped rather than fatal — the plugin discarded that subtree,
    // so a transform queued on a node within it is moot. A passed-through child
    // keeps its identity (via `_ref`) and so is never stranded this way.
    let options = satteri_plugin_api::MdastCommandOptions {
        escape_raw_html_braces: mdx,
    };
    let (new_arena, dropped) = satteri_plugin_api::apply_mdast_commands_lenient_with_options(
        owned,
        &command_buf,
        &parse_markdown,
        options,
    )
    .map_err(|e| napi::Error::from_reason(format!("command error: {e}")))?;
    *arena = new_arena;
    Ok(dropped.len() as u32)
}

/// Convert an MDAST handle to a HAST handle. The MDAST handle is consumed (emptied).
#[napi]
pub fn convert_mdast_to_hast_handle(
    env: Env,
    handle: &MdastHandle,
    convert_options: Option<JsConvertOptions>,
) -> Result<HastHandle> {
    let mut arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    let mdx = arena.mdx;
    let parse_options = arena.parse_options;
    let convert_opts = js_convert_options_to_rust(env, convert_options);
    let owned = std::mem::replace(
        &mut *arena,
        satteri_arena::Arena::<Mdast>::new(String::new()),
    );
    let mut hast = satteri_ast::hast::mdast_arena_to_hast_arena_with_options(&owned, &convert_opts);
    hast.mdx = mdx;
    hast.parse_options = parse_options;
    Ok(External::new(Mutex::new(hast)))
}

/// Apply MDAST commands and convert to HAST handle in one step.
/// The MDAST handle is consumed (emptied).
#[napi]
pub fn apply_commands_and_convert_to_hast_handle(
    env: Env,
    handle: &MdastHandle,
    command_buf: Uint8Array,
    convert_options: Option<JsConvertOptions>,
) -> Result<HastHandle> {
    let mut arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    let mdx = arena.mdx;
    let parse_options = arena.parse_options;
    let parse_markdown = make_parse_fn(mdx, parse_options);
    let convert_opts = js_convert_options_to_rust(env, convert_options);
    let owned = std::mem::replace(
        &mut *arena,
        satteri_arena::Arena::<Mdast>::new(String::new()),
    );
    let options = satteri_plugin_api::MdastCommandOptions {
        escape_raw_html_braces: mdx,
    };
    let mutated = satteri_plugin_api::apply_mdast_commands_with_options(
        owned,
        &command_buf,
        &parse_markdown,
        options,
    )
    .map_err(|e| napi::Error::from_reason(format!("command error: {e}")))?;
    let mut hast_arena =
        satteri_ast::hast::mdast_arena_to_hast_arena_with_options(&mutated, &convert_opts);
    hast_arena.mdx = mdx;
    hast_arena.parse_options = parse_options;
    Ok(External::new(Mutex::new(hast_arena)))
}

/// Parse markdown source and convert to HAST. Returns an opaque handle.
/// The arena stays in Rust memory, no buffer is copied to JS.
#[napi]
pub fn create_hast_handle(
    env: Env,
    source: String,
    features: Option<JsFeatures>,
    convert_options: Option<JsConvertOptions>,
) -> Result<HastHandle> {
    let opts = features_to_options(features, false);
    let convert_opts = js_convert_options_to_rust(env, convert_options);
    let (mut mdast, _) = satteri_pulldown_cmark::parse(&source, opts);
    mdast.parse_options = opts.bits();
    let mut hast = satteri_ast::hast::mdast_arena_to_hast_arena_with_options(&mdast, &convert_opts);
    hast.mdx = false;
    hast.parse_options = opts.bits();
    Ok(External::new(Mutex::new(hast)))
}

/// Parse MDX source and convert to HAST. Returns an opaque handle.
#[cfg(feature = "mdx")]
#[napi]
pub fn create_mdx_hast_handle(
    env: Env,
    source: String,
    features: Option<JsFeatures>,
    convert_options: Option<JsConvertOptions>,
) -> Result<HastHandle> {
    let opts = features_to_options(features, true);
    let convert_opts = js_convert_options_to_rust(env, convert_options);
    let (mut mdast, mdx_errors) = satteri_pulldown_cmark::parse(&source, opts);
    if let Some((offset, msg)) = mdx_errors.first() {
        return Err(napi::Error::from_reason(
            satteri_mdxjs::parse_error_to_message(&source, *offset, msg).to_string(),
        ));
    }
    mdast.parse_options = opts.bits();
    let mut hast = satteri_ast::hast::mdast_arena_to_hast_arena_with_options(&mdast, &convert_opts);
    hast.mdx = true;
    hast.parse_options = opts.bits();
    Ok(External::new(Mutex::new(hast)))
}

/// Walk a HAST handle's arena and return matched nodes as a flat binary buffer.
#[napi]
pub fn walk_handle(handle: &HastHandle, subscriptions: Vec<JsSubscription>) -> Result<Uint8Array> {
    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    let subs: Vec<satteri_ast::walk::Subscription> = subscriptions
        .into_iter()
        .map(|s| satteri_ast::walk::Subscription {
            node_type: s.node_type,
            tag_filter: s.tag_filter,
        })
        .collect();
    Ok(Uint8Array::new(satteri_ast::walk::walk_hast(&arena, &subs)))
}

/// Apply a command buffer to a HAST handle's arena in-place. Returns how many
/// patches were dropped because their target lived inside a subtree this pass
/// removed or replaced (see the lenient note below); the JS pipeline warns when
/// non-zero. Mirrors `apply_commands_to_mdast_handle`.
#[napi]
pub fn apply_commands_to_handle(handle: &HastHandle, command_buf: Uint8Array) -> Result<u32> {
    let mut arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;

    let owned = std::mem::replace(
        &mut *arena,
        satteri_arena::Arena::<Hast>::new(String::new()),
    );
    // Lenient: a patch stranded inside a subtree the same pass replaced or
    // removed is dropped rather than fatal — the plugin discarded that subtree,
    // so a transform queued on a node within it is moot. A passed-through child
    // keeps its identity (via `_ref`) and so is never stranded this way.
    let (new_arena, dropped) = satteri_plugin_api::apply_hast_commands_lenient(owned, &command_buf)
        .map_err(|e| napi::Error::from_reason(format!("command error: {e}")))?;
    *arena = new_arena;
    Ok(dropped.len() as u32)
}

/// Render a HAST handle's arena to HTML. Does not consume the handle.
#[napi]
pub fn render_handle(handle: &HastHandle) -> Result<String> {
    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    Ok(satteri_ast::hast::hast_arena_to_html(&arena))
}

/// Compile a HAST handle's arena to MDX JavaScript. Does not consume the handle.
#[cfg(feature = "mdx")]
#[napi]
pub fn compile_handle(handle: &HastHandle, options: Option<JsMdxOptions>) -> Result<String> {
    let mut arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    let opts = js_options_to_rust(options);

    // Simplify plain MDX JSX elements (lowercase, no attrs) into HAST elements
    // so they can be collapsed by optimizeStatic.
    let ignore = opts
        .optimize_static
        .as_ref()
        .map(|c| c.ignore_elements.clone())
        .unwrap_or_default();
    satteri_mdxjs::simplify_plain_mdx_nodes(&mut arena, &ignore);

    satteri_mdxjs::compile_hast_arena(&arena, &opts)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

/// Parse a JavaScript expression and return its ESTree-compatible AST as a JSON string.
/// Returns null if parsing fails. The JS layer calls JSON.parse (faster than serde_json → NAPI).
#[cfg(feature = "mdx")]
#[napi]
pub fn parse_expression(source: String) -> Option<String> {
    satteri_mdxjs::parse_expression_to_estree_json(&source)
}

/// Parse ESM (import/export statements) and return ESTree-compatible AST as JSON.
#[cfg(feature = "mdx")]
#[napi]
pub fn parse_esm(source: String) -> Option<String> {
    satteri_mdxjs::parse_esm_to_estree_json(&source)
}

/// Read the node_data JSON blob for a node. Returns null if none is set.
/// Works for both MDAST and HAST handles.
#[napi]
pub fn get_node_data(handle: AnyHandle, node_id: u32) -> Option<String> {
    let bytes = match handle {
        Either::A(h) => h.lock().ok()?.get_node_data(node_id)?.to_vec(),
        Either::B(h) => h.lock().ok()?.get_node_data(node_id)?.to_vec(),
    };
    String::from_utf8(bytes).ok()
}

/// Collect the concatenated text content of a HAST node and all its descendants.
/// Walks entirely in Rust, no per-child NAPI round-trips.
#[napi]
pub fn text_content_handle(handle: &HastHandle, node_id: u32) -> Result<String> {
    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    Ok(satteri_ast::hast::text_content(&arena, node_id))
}

/// Options for `mdast_text_content_handle`, matching `mdast-util-to-string`.
#[napi(object)]
pub struct JsTextContentOptions {
    /// Include `alt` text from image nodes. Default: true.
    pub include_image_alt: Option<bool>,
    /// Include `value` from HTML nodes. Default: true.
    pub include_html: Option<bool>,
}

/// Collect the concatenated text content of an MDAST node and all its descendants.
/// Mirrors `mdast-util-to-string`: collects value from text nodes, alt from images.
#[napi]
pub fn mdast_text_content_handle(
    handle: &MdastHandle,
    node_id: u32,
    options: Option<JsTextContentOptions>,
) -> Result<String> {
    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    let opts = satteri_ast::mdast::TextContentOptions {
        include_image_alt: options
            .as_ref()
            .and_then(|o| o.include_image_alt)
            .unwrap_or(true),
        include_html: options
            .as_ref()
            .and_then(|o| o.include_html)
            .unwrap_or(true),
    };
    Ok(satteri_ast::mdast::text_content_with_options(
        &arena, node_id, &opts,
    ))
}

/// Release a handle's arena memory. The handle becomes empty but remains
/// valid (subsequent calls are no-ops or return empty results).
#[napi]
pub fn drop_handle(handle: AnyHandle) -> Result<()> {
    match handle {
        Either::A(h) => {
            let mut arena = h
                .lock()
                .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
            *arena = satteri_arena::Arena::<Mdast>::new(String::new());
        }
        Either::B(h) => {
            let mut arena = h
                .lock()
                .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
            *arena = satteri_arena::Arena::<Hast>::new(String::new());
        }
    }
    Ok(())
}
