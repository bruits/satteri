---
cargo/satteri-ast: patch
cargo/satteri-mdxjs: patch
npm/satteri: patch
---

Fixed list-valued properties on HAST elements: numeric items (like `coords`) no longer disappear, lists are separated by comma or space according to the schema of the element they sit in, and a comma-separated attribute parsed from HTML round-trips unchanged.
