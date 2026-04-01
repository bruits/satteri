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

// ---------------------------------------------------------------------------
// Direct rendering (no handle needed)
// ---------------------------------------------------------------------------

/// Parse Markdown source and return HTML string directly.
#[napi]
pub fn parse_to_html(source: String) -> Result<String> {
    let (arena, _) = tryckeri_parser::parse(&source, &tryckeri_parser::ParseOptions::default());
    Ok(tryckeri_hast::mdast_to_html(&arena))
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

/// A subscription passed from JS.
#[napi(object)]
pub struct JsSubscription {
    pub node_type: u8,
    pub tag_filter: Vec<String>,
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
    Ok(External::new(Mutex::new(hast_view)))
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
    Ok(External::new(Mutex::new(hast_view)))
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
    Ok(External::new(Mutex::new(view)))
}

/// Parse MDX source and convert to HAST. Returns an opaque handle.
#[napi]
pub fn create_mdx_hast_handle(source: String) -> Result<ArenaHandle> {
    let (mdast, _) = tryckeri_parser::parse(&source, &tryckeri_parser::ParseOptions::mdx());
    let hast_buf = tryckeri_hast::mdast_to_hast_buffer(&mdast.to_raw_buffer())
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    let view = tryckeri_mdast::MdastArena::from_raw_buffer(&hast_buf)
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    Ok(External::new(Mutex::new(view)))
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
    tryckeri_mdxjs::simplify_plain_mdx_nodes(&mut arena, &ignore);

    tryckeri_mdxjs::compile_hast_arena(&arena, &opts)
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
