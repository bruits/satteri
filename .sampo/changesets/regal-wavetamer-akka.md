---
npm/satteri: patch
cargo/satteri-ast: patch
---

Fixed HAST list properties retaining numeric and empty items across plugin passes, SVG list separators below HTML integration points, and Unicode whitespace preservation during HTML serialization. `hastToHtml` now ignores node metadata and skips MDX nodes in lite builds too.
