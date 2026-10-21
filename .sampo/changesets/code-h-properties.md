---
cargo/satteri-ast: patch
npm/satteri: patch
---

Fixed `data.hName`, `data.hProperties` and `data.hChildren` on a code block applying to its `<pre>` instead of its `<code>`, as `mdast-util-to-hast` does: they now rename, add attributes to, or replace the contents of the `<code>`, and an `hProperties` class replaces the language class.
