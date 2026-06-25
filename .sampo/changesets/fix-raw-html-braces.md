---
cargo/satteri-plugin-api: minor
npm/satteri: patch
---

Fixes Markdown plugins returning `rawHtml` with literal `{` or `}` rendering those braces as MDX escape fragments in `markdownToHtml`.
