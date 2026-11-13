---
cargo/satteri-pulldown-cmark: patch
---

Improved performance by compacting simple Markdown emphasis during arena parsing: same optimization as `SourceStrong`, but for `Emphasis` as `SourceEmphasis`.
