---
cargo/satteri-ast: minor
cargo/satteri-plugin-api: patch
npm/satteri: patch
---

Structural plugin mutations now apply in place instead of rebuilding the whole tree, so mutation cost scales with the number of edits rather than the document size (3 edits on a 115KB document drop from ~160µs to under 50µs).

A few pathological transform combinations that previously resolved now throw an `unsupported patch shape` error, most notably replacing a node with content that reuses the node itself while another transform in the same pass edits one of its descendants, and inserting a sibling before or after the root node. Replacing, removing, or wrapping the root itself keeps working.
