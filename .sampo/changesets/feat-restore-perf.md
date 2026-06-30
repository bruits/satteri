---
cargo/satteri-arena: minor
cargo/satteri-ast: minor
cargo/satteri-pulldown-cmark: minor
cargo/satteri-mdxjs: patch
cargo/satteri-plugin-api: patch
npm/satteri: patch
---

Improves performance across the pipeline: faster Markdown parsing and
source-position tracking, faster HTML attribute rendering, and fewer
allocations when compiling MDX to JavaScript.
