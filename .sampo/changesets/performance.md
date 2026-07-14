---
cargo/satteri-arena: minor
cargo/satteri-pulldown-cmark: minor
cargo/satteri-ast: minor
cargo/satteri-plugin-api: patch
cargo/satteri-napi: minor
npm/satteri: patch
---

Faster across the board: parsing is ~10% cheaper, editing the tree from plugins now costs proportionally to how much you change rather than how big the document is (3 edits on a 115KB document: ~160µs → under 50µs), reading nodes inside plugins is 40-75% faster, and memory stays flat under sustained workloads.
