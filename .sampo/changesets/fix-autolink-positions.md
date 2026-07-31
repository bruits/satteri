---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed GFM autolinks losing their `position` inside a `](…)` that never becomes a link, such as `[link [ref] text](https://example.com)` when `[ref]` is a defined reference.
