---
cargo/satteri-pulldown-cmark: minor
cargo/satteri-napi: patch
npm/satteri: patch
---

Arena pooling now covers the whole plugin pipeline instead of only the no-plugin fast paths, making small-document compiles with plugins 5-7% faster while keeping memory flat under sustained workloads.
