---
npm/satteri: patch
cargo/satteri-ast: patch
---

Add position data to hast nodes. Position information was already stored in the Rust arena during mdast-to-hast conversion, but was never exposed to the JavaScript side.
