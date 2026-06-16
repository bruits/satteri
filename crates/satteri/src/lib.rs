//! `satteri`: high-level Rust API for the Sätteri markdown/MDX pipeline.
//!
//! # Quick start
//!
//! ```
//! let html = satteri::markdown_to_html("# Hello world");
//! assert!(html.contains("<h1>Hello world</h1>"));
//! ```

/// Parse Markdown source and render it directly to HTML.
pub fn markdown_to_html(source: &str) -> String {
    let (arena, _) = satteri_pulldown_cmark::parse(source, satteri_pulldown_cmark::DEFAULT_OPTIONS);
    satteri_ast::mdast_to_html(&arena)
}

/// Compile MDX source directly to JavaScript.
#[cfg(feature = "mdx")]
pub fn compile_mdx(source: &str, options: &satteri_mdxjs::Options) -> Result<String, String> {
    satteri_mdxjs::compile(source, options, satteri_pulldown_cmark::MDX_OPTIONS)
        .map_err(|e| e.to_string())
}
