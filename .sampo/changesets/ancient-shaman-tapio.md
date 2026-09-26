---
cargo/satteri-arena: minor
cargo/satteri-ast: minor
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
cargo/satteri: patch
npm/satteri: patch
---

Improved Markdown parsing, HAST conversion, and HTML rendering performance for prose, tables, links, autolinks, code spans, and strong emphasis. Reduced unnecessary allocations when compiling MDX with configured ignored elements, and fixed panics and incorrect strong-emphasis output when an autolink immediately follows an inline link.

Changed Rust tree APIs to use `Document` for both borrowed and owned source text, with `NodePosition` arguments for construction and position setters. JavaScript APIs and wire layouts are unchanged.
