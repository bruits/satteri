---
cargo/satteri-arena: patch
cargo/satteri-plugin-api: patch
npm/satteri: patch
---

Fixed nodes a plugin passes back losing their `position`: a spread copy of a node, or a custom node given a position, now keeps it for the plugins after it.
