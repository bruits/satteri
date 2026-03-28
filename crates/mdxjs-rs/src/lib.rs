//! Public API of `mdxjs-rs`.
//!
//! *   [`compile()`][] — turn MDX into JavaScript
#![deny(clippy::pedantic)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]

mod configuration;
mod hast_util_to_oxc;
mod mdx_plugin_recma_document;
mod mdx_plugin_recma_jsx_rewrite;
mod oxc;
mod oxc_util_build_jsx;
mod oxc_utils;

use crate::{
    hast_util_to_oxc::{MdxProgram, hast_util_to_oxc},
    mdx_plugin_recma_document::{
        Options as DocumentOptions, mdx_plugin_recma_document as recma_document,
    },
    mdx_plugin_recma_jsx_rewrite::{
        Options as RewriteOptions, mdx_plugin_recma_jsx_rewrite as recma_jsx_rewrite,
    },
    oxc::serialize,
    oxc_util_build_jsx::{Options as BuildOptions, oxc_util_build_jsx},
};
use oxc_allocator::Allocator;
use oxc_span::Span;
use rustc_hash::FxHashSet;
use tryckeri_mdast::mdx_types::{self as message, Location};

pub use crate::configuration::{MdxConstructs, MdxParseOptions, OptimizeStaticConfig, Options};
pub use crate::mdx_plugin_recma_document::JsxRuntime;

/// Turn MDX into JavaScript.
///
/// ## Examples
///
/// ```
/// use mdxjs::compile;
/// # fn main() -> Result<(), tryckeri_mdast::mdx_types::Message> {
///
/// let result = compile("# Hi!", &Default::default())?;
/// assert!(result.contains("function _createMdxContent"));
/// # Ok(())
/// # }
/// ```
///
/// ## Errors
///
/// This project errors for many different reasons, such as syntax errors in
/// the MDX format or misconfiguration.
pub fn compile(value: &str, options: &Options) -> Result<String, message::Message> {
    let normalised;
    let value = if value.contains('\t') {
        normalised = expand_tabs(value);
        &normalised as &str
    } else {
        value
    };
    let (arena, mdx_errors) = parser::parse(value, &parser::ParseOptions::mdx());
    if let Some((offset, msg)) = mdx_errors.first() {
        let point = byte_offset_to_point(value, *offset);
        return Err(message::Message {
            place: Some(Box::new(message::Place::Point(point))),
            reason: msg.clone(),
            rule_id: Box::new("unexpected-character".into()),
            source: Box::new("mdx-jsx".into()),
        });
    }
    let mdast_buf = arena.to_raw_buffer();
    compile_arena_bytes(&mdast_buf, options)
}

/// Compile a raw MDAST binary buffer (as produced by the NAPI layer) to JavaScript.
///
/// This is the main compilation path: MDAST binary → HAST binary → OXC → JS.
/// All other compile functions route through this.
///
/// ## Errors
///
/// Returns an error if the buffer is malformed or compilation fails.
pub fn compile_arena_bytes(buf: &[u8], options: &Options) -> Result<String, message::Message> {
    // Extract source text from MDAST buffer for position resolution.
    let mdast_view =
        tryckeri_mdast::MdastArena::from_raw_buffer(buf).map_err(|e| message::Message {
            reason: format!("invalid MDAST buffer: {e:?}"),
            place: None,
            rule_id: Box::new(String::new()),
            source: Box::new("mdxjs".into()),
        })?;
    let source = mdast_view.get_source().to_string();

    let hast_buf = tryckeri_hast::mdast_to_hast_buffer(buf).map_err(|e| message::Message {
        reason: format!("invalid MDAST buffer: {e:?}"),
        place: None,
        rule_id: Box::new(String::new()),
        source: Box::new("mdxjs".into()),
    })?;
    compile_hast_buffer_with_source(&hast_buf, options, source.as_bytes())
}

/// Compile a HAST binary buffer (with MDX node types) to JavaScript.
///
/// This is the split-pipeline entry point: takes a HAST binary buffer
/// and runs hast → OXC → JS directly from the binary format.
///
/// ## Errors
///
/// Returns an error if the buffer is malformed or compilation fails.
pub fn compile_hast_buffer(buf: &[u8], options: &Options) -> Result<String, message::Message> {
    compile_hast_buffer_with_source(buf, options, &[])
}

/// Compile a HAST binary buffer to JavaScript, with source text for position resolution.
///
/// ## Errors
///
/// Returns an error if the buffer is malformed or compilation fails.
pub fn compile_hast_buffer_with_source(
    buf: &[u8],
    options: &Options,
    source: &[u8],
) -> Result<String, message::Message> {
    let view = tryckeri_mdast::MdastArena::from_raw_buffer(buf).map_err(|e| message::Message {
        reason: format!("invalid HAST buffer: {e:?}"),
        place: None,
        rule_id: Box::new(String::new()),
        source: Box::new("mdxjs".into()),
    })?;

    let allocator = Allocator::default();
    let location = Location::new(source);
    let mut explicit_jsxs = FxHashSet::default();
    let mut program = hast_util_to_oxc(
        &view,
        options.filepath.clone(),
        Some(&location),
        &mut explicit_jsxs,
        &allocator,
        options.optimize_static.as_ref(),
    )?;
    mdx_plugin_recma_document(&mut program, options, Some(&location), &allocator)?;
    mdx_plugin_recma_jsx_rewrite(
        &mut program,
        options,
        Some(&location),
        &explicit_jsxs,
        &allocator,
    )?;
    Ok(serialize(&program.program))
}

/// Wrap the ES AST nodes coming from hast into a whole document.
///
/// ## Errors
///
/// This functions errors for double layouts (default exports).
pub fn mdx_plugin_recma_document<'a>(
    program: &mut MdxProgram<'a>,
    options: &Options,
    location: Option<&Location>,
    allocator: &'a Allocator,
) -> Result<(), message::Message> {
    let document_options = DocumentOptions {
        pragma: options.pragma.clone(),
        pragma_frag: options.pragma_frag.clone(),
        pragma_import_source: options.pragma_import_source.clone(),
        jsx_import_source: options.jsx_import_source.clone(),
        jsx_runtime: options.jsx_runtime,
    };
    recma_document(program, &document_options, location, allocator)
}

/// Rewrite JSX in an MDX file so that components can be passed in and provided.
/// Also compiles JSX to function calls unless `options.jsx` is true.
///
/// ## Errors
///
/// This functions errors for incorrect JSX runtime configuration *inside*
/// MDX files and problems with OXC (broken JS syntax).
#[allow(clippy::implicit_hasher)]
pub fn mdx_plugin_recma_jsx_rewrite<'a>(
    program: &mut MdxProgram<'a>,
    options: &Options,
    location: Option<&Location>,
    explicit_jsxs: &FxHashSet<Span>,
    allocator: &'a Allocator,
) -> Result<(), message::Message> {
    let rewrite_options = RewriteOptions {
        development: options.development,
        provider_import_source: options.provider_import_source.clone(),
    };

    recma_jsx_rewrite(
        program,
        &rewrite_options,
        location,
        explicit_jsxs,
        allocator,
    );

    if !options.jsx {
        let build_options = BuildOptions {
            development: options.development,
        };

        oxc_util_build_jsx(program, &build_options, location, allocator)?;
    }

    Ok(())
}

/// Convert a byte offset in source text to a `Point` (line, column, offset).
fn byte_offset_to_point(value: &str, offset: usize) -> message::Point {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in value.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    message::Point::new(line, col, offset)
}

/// Expand tab characters to spaces for indentation purposes.
///
/// `markdown-rs` and `micromark` handle tabs inside list items differently:
/// micromark treats a tab as continuation whitespace for the list item,
/// while `markdown-rs` can interpret it as a code-indented block boundary.
/// Normalising leading tabs to spaces before parsing avoids this discrepancy.
fn expand_tabs(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for line in value.split('\n') {
        let mut col = 0usize;
        let chars = line.chars().peekable();
        let mut in_indent = true;
        for ch in chars {
            if in_indent && ch == '\t' {
                let spaces = 4 - (col % 4);
                for _ in 0..spaces {
                    out.push(' ');
                }
                col += spaces;
            } else {
                if ch != ' ' {
                    in_indent = false;
                }
                out.push(ch);
                col += 1;
            }
        }
        out.push('\n');
    }
    if !value.ends_with('\n') {
        out.pop();
    }
    out
}
