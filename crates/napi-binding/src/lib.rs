#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;

// ---------------------------------------------------------------------------
// MDX compilation options (JS-facing)
// ---------------------------------------------------------------------------

/// Static optimization config passed from JavaScript.
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
#[napi(object)]
pub struct JsMdxOptions {
    /// Static subtree optimization. If provided, static subtrees are collapsed
    /// into raw HTML strings using the specified component and prop.
    pub optimize_static: Option<JsOptimizeStaticConfig>,
}

fn js_options_to_rust(opts: Option<JsMdxOptions>) -> mdxjs::Options {
    let mut options = mdxjs::Options::default();
    if let Some(js) = opts {
        if let Some(config) = js.optimize_static {
            options.optimize_static = Some(mdxjs::OptimizeStaticConfig {
                component: config.component,
                prop: config.prop,
                wrap_prop_value: config.wrap_prop_value.unwrap_or(false),
                ignore_elements: config.ignore_elements.unwrap_or_default(),
            });
        }
    }
    options
}

// ---------------------------------------------------------------------------
// MDX compilation
// ---------------------------------------------------------------------------

/// Compile MDX source directly to JavaScript.
#[napi]
pub fn compile_mdx(source: String, options: Option<JsMdxOptions>) -> Result<String> {
    let opts = js_options_to_rust(options);
    mdxjs::compile(&source, &opts).map_err(|e| napi::Error::from_reason(e.to_string()))
}

/// Compile a pre-parsed MDAST binary buffer to MDX JavaScript output.
#[napi]
pub fn compile_mdx_from_buffer(buf: Uint8Array, options: Option<JsMdxOptions>) -> Result<String> {
    let opts = js_options_to_rust(options);
    mdxjs::compile_arena_bytes(&buf, &opts).map_err(|e| napi::Error::from_reason(e.to_string()))
}

// ---------------------------------------------------------------------------
// New parser (pulldown-cmark based)
// ---------------------------------------------------------------------------

/// Parse Markdown/MDX source and return a raw binary MdastArena buffer.
#[napi]
pub fn parse_to_buffer(source: String) -> Result<Uint8Array> {
    let (arena, _) = parser::parse(&source, &parser::ParseOptions::default());
    Ok(Uint8Array::new(arena.to_raw_buffer()))
}

/// Parse MDX source and return a raw binary MdastArena buffer (MDX mode).
#[napi]
pub fn parse_mdx_to_buffer(source: String) -> Result<Uint8Array> {
    let (arena, _) = parser::parse(&source, &parser::ParseOptions::mdx());
    Ok(Uint8Array::new(arena.to_raw_buffer()))
}

/// Parse Markdown source and return a HAST binary buffer.
#[napi]
pub fn parse_to_hast_buffer(source: String) -> Result<Uint8Array> {
    let (arena, _) = parser::parse(&source, &parser::ParseOptions::default());
    let hast_buf = tryckeri_hast::mdast_to_hast_buffer(&arena.to_raw_buffer())
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    Ok(Uint8Array::new(hast_buf))
}

/// Parse MDX source and return a HAST binary buffer (MDX mode).
#[napi]
pub fn parse_mdx_to_hast_buffer(source: String) -> Result<Uint8Array> {
    let (arena, _) = parser::parse(&source, &parser::ParseOptions::mdx());
    let hast_buf = tryckeri_hast::mdast_to_hast_buffer(&arena.to_raw_buffer())
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    Ok(Uint8Array::new(hast_buf))
}

/// Parse Markdown source and return HTML string directly.
#[napi]
pub fn parse_to_html(source: String) -> Result<String> {
    let (arena, _) = parser::parse(&source, &parser::ParseOptions::default());
    Ok(tryckeri_hast::mdast_to_html(&arena))
}

/// Parse MDX source and return HTML string directly.
#[napi]
pub fn parse_mdx_to_html(source: String) -> Result<String> {
    let (arena, _) = parser::parse(&source, &parser::ParseOptions::mdx());
    Ok(tryckeri_hast::mdast_to_html(&arena))
}

// ---------------------------------------------------------------------------
// HAST utilities (parser-agnostic)
// ---------------------------------------------------------------------------

/// Convert an existing MDAST binary buffer to a HAST binary buffer.
/// Works for both Markdown and MDX — MDX nodes are converted to MDX HAST types.
#[napi]
pub fn mdast_buffer_to_hast_buffer(buf: Uint8Array) -> Result<Uint8Array> {
    let hast_buf = tryckeri_hast::mdast_to_hast_buffer(&buf)
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    Ok(Uint8Array::new(hast_buf))
}

/// Convert a HAST binary buffer to an HTML string.
#[napi]
pub fn hast_buffer_to_html_str(buf: Uint8Array) -> Result<String> {
    tryckeri_hast::hast_buffer_to_html(&buf).map_err(|e| napi::Error::from_reason(format!("{e:?}")))
}

/// Compile a HAST binary buffer (with MDX node types) to JavaScript.
/// This is the split-pipeline entry point for MDX: after MDAST→HAST conversion
/// and any HAST plugin mutations, this function does the final hast→JS step.
#[napi]
pub fn compile_hast_buffer_to_js(buf: Uint8Array, options: Option<JsMdxOptions>) -> Result<String> {
    let opts = js_options_to_rust(options);
    mdxjs::compile_hast_buffer(&buf, &opts).map_err(|e| napi::Error::from_reason(e.to_string()))
}

// ---------------------------------------------------------------------------
// Mutation application
// ---------------------------------------------------------------------------

/// Apply a binary command buffer of mutations to an MDAST arena buffer.
///
/// The command buffer is produced by the JS `CommandBuffer` class (see
/// `command-buffer.ts`). It encodes remove, setProperty, insert, replace,
/// and other structural mutations in a compact binary format.
///
/// Returns a new MDAST arena buffer with all mutations applied.
#[napi]
pub fn apply_mutations(arena_buf: Uint8Array, command_buf: Uint8Array) -> Result<Uint8Array> {
    // Deserialize the arena from its binary buffer
    let view = tryckeri_mdast::MdastArena::from_raw_buffer(&arena_buf)
        .map_err(|e| napi::Error::from_reason(format!("invalid arena buffer: {e:?}")))?;
    let arena = view.to_arena();

    // Provide the real parser as the markdown parsing callback
    let parse_markdown = |source: &str| -> tryckeri_mdast::MdastArena {
        let (parsed, _errors) = parser::parse(source, &parser::ParseOptions::mdx());
        parsed
    };

    let new_arena = tryckeri_mdast::apply_commands(&arena, &command_buf, &parse_markdown)
        .map_err(|e| napi::Error::from_reason(format!("command error: {e}")))?;

    Ok(Uint8Array::new(new_arena.to_raw_buffer()))
}

// ---------------------------------------------------------------------------
// Buffer metadata
// ---------------------------------------------------------------------------

/// Return metadata about the MdastNode struct size and buffer format version.
#[napi(object)]
pub struct BufferFormat {
    pub node_struct_size: u32,
    pub version: u32,
    pub magic: String,
}

#[napi]
pub fn get_buffer_format() -> BufferFormat {
    BufferFormat {
        node_struct_size: tryckeri_mdast::NODE_STRUCT_SIZE as u32,
        version: 1,
        magic: "MDAR".to_string(),
    }
}
