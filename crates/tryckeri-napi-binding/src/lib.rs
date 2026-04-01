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

fn js_options_to_rust(opts: Option<JsMdxOptions>) -> tryckeri_mdxjs::Options {
    let mut options = tryckeri_mdxjs::Options::default();
    if let Some(js) = opts {
        if let Some(config) = js.optimize_static {
            options.optimize_static = Some(tryckeri_mdxjs::OptimizeStaticConfig {
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
    tryckeri_mdxjs::compile(&source, &opts).map_err(|e| napi::Error::from_reason(e.to_string()))
}

/// Compile a pre-parsed MDAST binary buffer to MDX JavaScript output.
#[napi]
pub fn compile_mdx_from_buffer(buf: Uint8Array, options: Option<JsMdxOptions>) -> Result<String> {
    let opts = js_options_to_rust(options);
    tryckeri_mdxjs::compile_arena_bytes(&buf, &opts)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

// ---------------------------------------------------------------------------
// New parser (pulldown-cmark based)
// ---------------------------------------------------------------------------

/// Parse Markdown/MDX source and return a raw binary MdastArena buffer.
#[napi]
pub fn parse_to_buffer(source: String) -> Result<Uint8Array> {
    let (arena, _) = tryckeri_parser::parse(&source, &tryckeri_parser::ParseOptions::default());
    Ok(Uint8Array::new(arena.to_raw_buffer()))
}

/// Parse MDX source and return a raw binary MdastArena buffer (MDX mode).
#[napi]
pub fn parse_mdx_to_buffer(source: String) -> Result<Uint8Array> {
    let (arena, _) = tryckeri_parser::parse(&source, &tryckeri_parser::ParseOptions::mdx());
    Ok(Uint8Array::new(arena.to_raw_buffer()))
}

/// Parse Markdown source and return a HAST binary buffer.
#[napi]
pub fn parse_to_hast_buffer(source: String) -> Result<Uint8Array> {
    let (arena, _) = tryckeri_parser::parse(&source, &tryckeri_parser::ParseOptions::default());
    let hast_buf = tryckeri_hast::mdast_to_hast_buffer(&arena.to_raw_buffer())
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    Ok(Uint8Array::new(hast_buf))
}

/// Parse MDX source and return a HAST binary buffer (MDX mode).
#[napi]
pub fn parse_mdx_to_hast_buffer(source: String) -> Result<Uint8Array> {
    let (arena, _) = tryckeri_parser::parse(&source, &tryckeri_parser::ParseOptions::mdx());
    let hast_buf = tryckeri_hast::mdast_to_hast_buffer(&arena.to_raw_buffer())
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    Ok(Uint8Array::new(hast_buf))
}

/// Parse Markdown source and return HTML string directly.
#[napi]
pub fn parse_to_html(source: String) -> Result<String> {
    let (arena, _) = tryckeri_parser::parse(&source, &tryckeri_parser::ParseOptions::default());
    Ok(tryckeri_hast::mdast_to_html(&arena))
}

/// Parse MDX source and return HTML string directly.
#[napi]
pub fn parse_mdx_to_html(source: String) -> Result<String> {
    let (arena, _) = tryckeri_parser::parse(&source, &tryckeri_parser::ParseOptions::mdx());
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
    tryckeri_mdxjs::compile_hast_buffer(&buf, &opts)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
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
    let view = tryckeri_mdast::MdastArena::from_raw_buffer(&arena_buf)
        .map_err(|e| napi::Error::from_reason(format!("invalid arena buffer: {e:?}")))?;
    let parse_markdown = make_parse_fn();
    let new_arena = tryckeri_mdast::apply_commands(view.to_arena(), &command_buf, &parse_markdown)
        .map_err(|e| napi::Error::from_reason(format!("command error: {e}")))?;
    Ok(Uint8Array::new(new_arena.to_raw_buffer()))
}

// ---------------------------------------------------------------------------
// Selective walk (Rust-side tree walk with filtered subscriptions)
// ---------------------------------------------------------------------------

/// A subscription passed from JS.
#[napi(object)]
pub struct JsSubscription {
    pub node_type: u8,
    pub tag_filter: Vec<String>,
}

/// Walk the arena and return matched nodes as a flat binary buffer.
/// Returns a single Uint8Array — JS reads it with DataView, no per-node allocation.
#[napi]
pub fn walk_and_collect(
    arena_buf: Uint8Array,
    subscriptions: Vec<JsSubscription>,
) -> Result<Uint8Array> {
    let view = tryckeri_mdast::MdastArena::from_raw_buffer(&arena_buf)
        .map_err(|e| napi::Error::from_reason(format!("invalid arena buffer: {e:?}")))?;

    let subs: Vec<tryckeri_mdast::Subscription> = subscriptions
        .into_iter()
        .map(|s| tryckeri_mdast::Subscription {
            node_type: s.node_type,
            tag_filter: s.tag_filter,
        })
        .collect();

    Ok(Uint8Array::new(tryckeri_mdast::walk_and_collect(
        &view, &subs,
    )))
}

// ---------------------------------------------------------------------------
// Fused pipeline steps
// ---------------------------------------------------------------------------

/// Apply MDAST mutations and convert to HAST buffer in one step.
#[napi]
pub fn apply_mutations_and_convert_to_hast(
    arena_buf: Uint8Array,
    command_buf: Uint8Array,
) -> Result<Uint8Array> {
    let view = tryckeri_mdast::MdastArena::from_raw_buffer(&arena_buf)
        .map_err(|e| napi::Error::from_reason(format!("invalid arena buffer: {e:?}")))?;

    let parse_markdown = make_parse_fn();

    let arena = tryckeri_mdast::apply_commands(view.to_arena(), &command_buf, &parse_markdown)
        .map_err(|e| napi::Error::from_reason(format!("command error: {e}")))?;

    Ok(Uint8Array::new(tryckeri_hast::mdast_arena_to_hast_buffer(
        &arena,
    )))
}

/// Apply mutations and render to HTML in one step — no serialize→deserialize round-trip.
#[napi]
pub fn apply_mutations_and_render_html(
    arena_buf: Uint8Array,
    command_buf: Uint8Array,
) -> Result<String> {
    let view = tryckeri_mdast::MdastArena::from_raw_buffer(&arena_buf)
        .map_err(|e| napi::Error::from_reason(format!("invalid arena buffer: {e:?}")))?;

    let parse_markdown = make_parse_fn();

    let arena = tryckeri_mdast::apply_commands(view.to_arena(), &command_buf, &parse_markdown)
        .map_err(|e| napi::Error::from_reason(format!("command error: {e}")))?;

    Ok(tryckeri_hast::hast_arena_to_html(&arena))
}

/// Apply mutations and compile to MDX JS in one step — no serialize→deserialize round-trip.
#[napi]
pub fn apply_mutations_and_compile_js(
    arena_buf: Uint8Array,
    command_buf: Uint8Array,
    options: Option<JsMdxOptions>,
) -> Result<String> {
    let view = tryckeri_mdast::MdastArena::from_raw_buffer(&arena_buf)
        .map_err(|e| napi::Error::from_reason(format!("invalid arena buffer: {e:?}")))?;

    let parse_markdown = make_parse_fn();

    let arena = tryckeri_mdast::apply_commands(view.to_arena(), &command_buf, &parse_markdown)
        .map_err(|e| napi::Error::from_reason(format!("command error: {e}")))?;

    // MDX compiler still needs the binary format — serialize once
    let raw = arena.to_raw_buffer();
    let opts = js_options_to_rust(options);
    tryckeri_mdxjs::compile_hast_buffer(&raw, &opts)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

// ---------------------------------------------------------------------------
// Handle-based API — arena stays in Rust, no buffer copies to JS
// ---------------------------------------------------------------------------

use std::sync::Mutex;

type ArenaHandle = External<Mutex<tryckeri_mdast::MdastArena>>;

fn make_parse_fn() -> impl Fn(&str) -> tryckeri_mdast::MdastArena {
    |source: &str| -> tryckeri_mdast::MdastArena {
        let (parsed, _errors) =
            tryckeri_parser::parse(source, &tryckeri_parser::ParseOptions::mdx());
        parsed
    }
}

// ── MDAST handles ──────────────────────────────────────────────────────────

/// Parse markdown source into an MDAST arena handle.
#[napi]
pub fn create_mdast_handle(source: String) -> Result<ArenaHandle> {
    let (arena, _) = tryckeri_parser::parse(&source, &tryckeri_parser::ParseOptions::default());
    Ok(External::new(Mutex::new(arena)))
}

/// Parse MDX source into an MDAST arena handle.
#[napi]
pub fn create_mdx_mdast_handle(source: String) -> Result<ArenaHandle> {
    let (arena, _) = tryckeri_parser::parse(&source, &tryckeri_parser::ParseOptions::mdx());
    Ok(External::new(Mutex::new(arena)))
}

/// Serialize an MDAST handle to a binary buffer (read-only snapshot for JS visitor).
#[napi]
pub fn serialize_mdast_handle(handle: &ArenaHandle) -> Result<Uint8Array> {
    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    Ok(Uint8Array::new(arena.to_raw_buffer()))
}

/// Get the source string from an MDAST handle.
#[napi]
pub fn get_handle_source(handle: &ArenaHandle) -> Result<String> {
    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    Ok(arena.source().to_string())
}

/// Set the `data` blob (JSON bytes) for a node in the handle's arena.
#[napi]
pub fn set_node_data(handle: &ArenaHandle, node_id: u32, json: Uint8Array) -> Result<()> {
    let mut arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    arena.set_node_data(node_id, json.to_vec());
    Ok(())
}

/// Get the `data` blob (JSON bytes) for a node in the handle's arena.
/// Returns null if no data is set.
#[napi]
pub fn get_node_data(handle: &ArenaHandle, node_id: u32) -> Result<Option<Uint8Array>> {
    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    Ok(arena
        .get_node_data(node_id)
        .map(|d| Uint8Array::new(d.to_vec())))
}

/// Walk an MDAST handle's arena and return matched nodes as a flat binary buffer.
#[napi]
pub fn walk_mdast_handle(
    handle: &ArenaHandle,
    subscriptions: Vec<JsSubscription>,
) -> Result<Uint8Array> {
    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    let subs: Vec<tryckeri_mdast::Subscription> = subscriptions
        .into_iter()
        .map(|s| tryckeri_mdast::Subscription {
            node_type: s.node_type,
            tag_filter: s.tag_filter,
        })
        .collect();
    Ok(Uint8Array::new(tryckeri_mdast::walk_and_collect_with_mode(
        &*arena,
        &subs,
        tryckeri_mdast::WalkMode::Mdast,
    )))
}

/// Apply a command buffer to an MDAST handle in-place.
#[napi]
pub fn apply_commands_to_mdast_handle(handle: &ArenaHandle, command_buf: Uint8Array) -> Result<()> {
    let mut arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    let parse_markdown = make_parse_fn();
    let owned = std::mem::replace(&mut *arena, tryckeri_mdast::MdastArena::new(String::new()));
    let new_arena = tryckeri_mdast::apply_commands(owned, &command_buf, &parse_markdown)
        .map_err(|e| napi::Error::from_reason(format!("command error: {e}")))?;
    *arena = new_arena;
    Ok(())
}

/// Convert an MDAST handle to a HAST handle. The MDAST handle is consumed (emptied).
#[napi]
pub fn convert_mdast_to_hast_handle(handle: &ArenaHandle) -> Result<ArenaHandle> {
    let mut arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    let owned = std::mem::replace(&mut *arena, tryckeri_mdast::MdastArena::new(String::new()));
    let hast_buf = tryckeri_hast::mdast_arena_to_hast_buffer(&owned);
    let hast_view = tryckeri_mdast::MdastArena::from_raw_buffer(&hast_buf)
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    Ok(External::new(Mutex::new(hast_view.to_arena())))
}

/// Apply MDAST commands and convert to HAST handle in one step.
/// The MDAST handle is consumed (emptied).
#[napi]
pub fn apply_commands_and_convert_to_hast_handle(
    handle: &ArenaHandle,
    command_buf: Uint8Array,
) -> Result<ArenaHandle> {
    let mut arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    let parse_markdown = make_parse_fn();
    let owned = std::mem::replace(&mut *arena, tryckeri_mdast::MdastArena::new(String::new()));
    let mutated = tryckeri_mdast::apply_commands(owned, &command_buf, &parse_markdown)
        .map_err(|e| napi::Error::from_reason(format!("command error: {e}")))?;
    let hast_buf = tryckeri_hast::mdast_arena_to_hast_buffer(&mutated);
    let hast_view = tryckeri_mdast::MdastArena::from_raw_buffer(&hast_buf)
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    Ok(External::new(Mutex::new(hast_view.to_arena())))
}

// ── HAST handles ───────────────────────────────────────────────────────────

/// Parse markdown source and convert to HAST. Returns an opaque handle.
/// The arena stays in Rust memory — no buffer is copied to JS.
#[napi]
pub fn create_hast_handle(source: String) -> Result<ArenaHandle> {
    let (mdast, _) = tryckeri_parser::parse(&source, &tryckeri_parser::ParseOptions::default());
    let hast_buf = tryckeri_hast::mdast_to_hast_buffer(&mdast.to_raw_buffer())
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    let view = tryckeri_mdast::MdastArena::from_raw_buffer(&hast_buf)
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    Ok(External::new(Mutex::new(view.to_arena())))
}

/// Wrap an existing HAST binary buffer as an opaque handle.
#[napi]
pub fn create_hast_handle_from_buffer(buf: Uint8Array) -> Result<ArenaHandle> {
    let view = tryckeri_mdast::MdastArena::from_raw_buffer(&buf)
        .map_err(|e| napi::Error::from_reason(format!("invalid buffer: {e:?}")))?;
    Ok(External::new(Mutex::new(view.to_arena())))
}

/// Parse MDX source and convert to HAST. Returns an opaque handle.
#[napi]
pub fn create_mdx_hast_handle(source: String) -> Result<ArenaHandle> {
    let (mdast, _) = tryckeri_parser::parse(&source, &tryckeri_parser::ParseOptions::mdx());
    let hast_buf = tryckeri_hast::mdast_to_hast_buffer(&mdast.to_raw_buffer())
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    let view = tryckeri_mdast::MdastArena::from_raw_buffer(&hast_buf)
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    Ok(External::new(Mutex::new(view.to_arena())))
}

/// Walk a handle's arena and return matched nodes as a flat binary buffer.
#[napi]
pub fn walk_handle(handle: &ArenaHandle, subscriptions: Vec<JsSubscription>) -> Result<Uint8Array> {
    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    let subs: Vec<tryckeri_mdast::Subscription> = subscriptions
        .into_iter()
        .map(|s| tryckeri_mdast::Subscription {
            node_type: s.node_type,
            tag_filter: s.tag_filter,
        })
        .collect();
    Ok(Uint8Array::new(tryckeri_mdast::walk_and_collect(
        &*arena, &subs,
    )))
}

/// Apply a command buffer to a handle's arena in-place. No serialize/deserialize.
#[napi]
pub fn apply_commands_to_handle(handle: &ArenaHandle, command_buf: Uint8Array) -> Result<()> {
    let mut arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;

    let parse_markdown = make_parse_fn();

    // apply_commands takes ownership, so swap out the arena
    let owned = std::mem::replace(&mut *arena, tryckeri_mdast::MdastArena::new(String::new()));
    let new_arena = tryckeri_mdast::apply_commands(owned, &command_buf, &parse_markdown)
        .map_err(|e| napi::Error::from_reason(format!("command error: {e}")))?;
    *arena = new_arena;
    Ok(())
}

/// Serialize a handle's arena to a binary buffer (for fallback paths like transformRoot).
#[napi]
pub fn serialize_handle(handle: &ArenaHandle) -> Result<Uint8Array> {
    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    Ok(Uint8Array::new(arena.to_raw_buffer()))
}

/// Render a handle's HAST arena to HTML. Does not consume the handle.
#[napi]
pub fn render_handle(handle: &ArenaHandle) -> Result<String> {
    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    Ok(tryckeri_hast::hast_arena_to_html(&arena))
}

/// Compile a handle's HAST arena to MDX JavaScript. Does not consume the handle.
#[napi]
pub fn compile_handle(handle: &ArenaHandle, options: Option<JsMdxOptions>) -> Result<String> {
    let arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    let raw = arena.to_raw_buffer();
    let opts = js_options_to_rust(options);
    tryckeri_mdxjs::compile_hast_buffer(&raw, &opts)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

/// Release the arena memory held by a handle. The handle becomes empty
/// but remains valid (subsequent calls are no-ops or return empty results).
#[napi]
pub fn drop_handle(handle: &ArenaHandle) -> Result<()> {
    let mut arena = handle
        .lock()
        .map_err(|e| napi::Error::from_reason(format!("lock: {e}")))?;
    *arena = tryckeri_mdast::MdastArena::new(String::new());
    Ok(())
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
