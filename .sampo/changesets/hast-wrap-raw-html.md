---
cargo/satteri-ast: minor
cargo/satteri-plugin-api: minor
cargo/satteri-napi: patch
npm/satteri: minor
---

Adds `{ rawHtml }` support to `wrapNode()` in HAST plugins: the HTML is parsed and the node is wrapped in the resulting element.
