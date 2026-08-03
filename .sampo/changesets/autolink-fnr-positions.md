---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Added source positions to GFM autolinks that previously had none, such as a bare URL following an unclosed `[`. remark leaves those nodes position-less; Sätteri now reports the exact source they came from.
