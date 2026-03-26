#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;

// ---------------------------------------------------------------------------
// MDX compilation
// ---------------------------------------------------------------------------

/// Compile MDX source directly to JavaScript.
#[napi]
pub fn compile_mdx(source: String) -> Result<String> {
    mdxjs::compile(&source, &mdxjs::Options::default())
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

/// Compile a pre-parsed MDAST binary buffer to MDX JavaScript output.
#[napi]
pub fn compile_mdx_from_buffer(buf: Buffer) -> Result<String> {
    mdxjs::compile_arena_bytes(&buf, &mdxjs::Options::default())
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

/// Compile MDX source to JavaScript with syntax highlighting via syntect.
/// `theme` is a syntect theme name (e.g. "base16-ocean.dark", "InspiredGitHub").
#[napi]
pub fn compile_mdx_highlighted(source: String, theme: String) -> Result<String> {
    let opts = mdxjs::Options {
        syntax_highlight_theme: Some(theme),
        ..Default::default()
    };
    mdxjs::compile(&source, &opts).map_err(|e| napi::Error::from_reason(e.to_string()))
}

/// Compile a pre-parsed MDAST binary buffer with syntax highlighting.
#[napi]
pub fn compile_mdx_from_buffer_highlighted(buf: Buffer, theme: String) -> Result<String> {
    let opts = mdxjs::Options {
        syntax_highlight_theme: Some(theme),
        ..Default::default()
    };
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
pub fn parse_to_hast_buffer(source: String) -> Result<Buffer> {
    let (arena, _) = parser::parse(&source, &parser::ParseOptions::default());
    let hast_buf = tryckeri_hast::mdast_to_hast_buffer(&arena.to_raw_buffer())
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    Ok(Buffer::from(hast_buf))
}

/// Parse MDX source and return a HAST binary buffer (MDX mode).
#[napi]
pub fn parse_mdx_to_hast_buffer(source: String) -> Result<Buffer> {
    let (arena, _) = parser::parse(&source, &parser::ParseOptions::mdx());
    let hast_buf = tryckeri_hast::mdast_to_hast_buffer(&arena.to_raw_buffer())
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    Ok(Buffer::from(hast_buf))
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
#[napi]
pub fn mdast_buffer_to_hast_buffer(buf: Buffer) -> Result<Buffer> {
    let hast_buf = tryckeri_hast::mdast_to_hast_buffer(&buf)
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?;
    Ok(Buffer::from(hast_buf))
}

/// Convert a HAST binary buffer to an HTML string.
#[napi]
pub fn hast_buffer_to_html_str(buf: Buffer) -> Result<String> {
    tryckeri_hast::hast_buffer_to_html(&buf).map_err(|e| napi::Error::from_reason(format!("{e:?}")))
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
pub fn apply_mutations(
    arena_buf: Buffer,
    command_buf: Buffer,
) -> Result<Uint8Array> {
    // Deserialize the arena from its binary buffer
    let view = mdast_arena::MdastArena::from_raw_buffer(&arena_buf)
        .map_err(|e| napi::Error::from_reason(format!("invalid arena buffer: {e:?}")))?;
    let arena = view.to_arena();

    // Provide the real parser as the markdown parsing callback
    let parse_markdown = |source: &str| -> mdast_arena::MdastArena {
        let (parsed, _errors) = parser::parse(source, &parser::ParseOptions::mdx());
        parsed
    };

    let new_arena = mdast_arena::apply_commands(&arena, &command_buf, &parse_markdown)
        .map_err(|e| napi::Error::from_reason(format!("command error: {e}")))?;

    Ok(Uint8Array::new(new_arena.to_raw_buffer()))
}

// ---------------------------------------------------------------------------
// Buffer metadata
// ---------------------------------------------------------------------------

/// Return metadata about the ArenaNode struct size and buffer format version.
#[napi(object)]
pub struct BufferFormat {
    pub node_struct_size: u32,
    pub version: u32,
    pub magic: String,
}

#[napi]
pub fn get_buffer_format() -> BufferFormat {
    BufferFormat {
        node_struct_size: mdast_arena::NODE_STRUCT_SIZE as u32,
        version: 1,
        magic: "MDAR".to_string(),
    }
}

