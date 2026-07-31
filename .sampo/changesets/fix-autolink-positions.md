---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed GFM autolinks losing their `position` when they follow a `](…)` that never becomes a link, such as `[link [ref] text](https://example.com)`.
