---
npm/satteri: patch
cargo/satteri-ast: patch
---

Fixed a node moved out of a removed or replaced parent sometimes staying attached to that parent, and edits queued beside it being dropped without a warning.
