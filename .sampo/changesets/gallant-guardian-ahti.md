---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-ast: patch
cargo/satteri-mdxjs: patch
npm/satteri: patch
---

Fixes many output inconsistencies with remark across Markdown, GFM, and MDX parsing, mostly found by extensive property-based fuzz testing. Notable areas: GFM bare-URL detection, MDX JSX flow vs inline classification, footnote numbering and section ordering, directive label inline parsing, list spread/tight handling, and reference link spans.
