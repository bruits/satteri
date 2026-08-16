//! WebAssembly bindings for browser and edge runtimes.
//!
//! This crate deliberately avoids NAPI, WASI, filesystem access, and worker
//! threads. It exposes the same Rust pipeline through the WebAssembly host
//! APIs available in browsers and single-threaded edge runtimes.

use satteri_arena::{Arena, Hast};
use wasm_bindgen::prelude::*;

/// compile Markdown to HTML without requiring a JavaScript runtime API.
#[wasm_bindgen]
pub fn markdown_to_html(source: &str) -> String {
    satteri::markdown_to_html(source)
}

/// parse Markdown or MDX and return the serialized HAST arena.
///
/// The returned bytes use Sätteri's existing HAST wire format, so consumers
/// can materialize the same tree shape as the native binding without adding a
/// second AST representation to the project.
#[wasm_bindgen]
pub fn mdx_to_hast(source: &str) -> Result<Vec<u8>, JsValue> {
    let (mdast, errors) =
        satteri_pulldown_cmark::parse_no_positions(source, satteri_pulldown_cmark::MDX_OPTIONS);

    if let Some((offset, message)) = errors.first() {
        return Err(JsValue::from_str(&format!(
            "MDX parse error at byte {offset}: {message}"
        )));
    }

    let hast: Arena<Hast> = satteri_ast::hast::mdast_arena_to_hast_arena(&mdast);
    Ok(hast.to_raw_buffer())
}

/// compile MDX to JavaScript for build-time consumers.
///
/// Runtime consumers should prefer [`mdx_to_hast`] because edge runtimes can
/// prohibit evaluating JavaScript generated from request data.
#[cfg(feature = "build-js")]
#[wasm_bindgen]
pub fn mdx_to_js(source: &str) -> Result<String, JsValue> {
    let options = satteri_mdxjs::Options::default();
    satteri::compile_mdx(source, &options).map_err(|error| JsValue::from_str(&error))
}

#[cfg(test)]
mod tests {
    use super::{markdown_to_html, mdx_to_hast};

    #[test]
    fn markdown_to_html_uses_the_rust_renderer() {
        assert_eq!(
            markdown_to_html("# edge\n\nHello **world**."),
            "<h1>edge</h1>\n<p>Hello <strong>world</strong>.</p>\n"
        );
    }

    #[test]
    fn mdx_to_hast_returns_the_hast_wire_format() {
        let buffer = mdx_to_hast("# edge\n\n<Component />").expect("valid MDX");

        assert_eq!(&buffer[..4], b"MDAR");
        assert!(buffer.len() > 32);
    }
}
