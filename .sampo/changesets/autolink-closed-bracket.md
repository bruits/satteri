---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed two bare URLs separated by a `]` being merged into one over-long link, as in `[www.a.com]www.b.com`.
