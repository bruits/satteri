---
cargo/satteri-ast: minor
cargo/satteri-plugin-api: patch
npm/satteri: patch
---

Editing a node that belongs to a different document — a node kept from a previous compile, or an mdast node used in a hast plugin — now fails the compile with `invalid node id`. A few pathological edits now throw `unsupported patch shape`, most notably replacing a node with new content that reuses that same node while another plugin edits something inside it in the same pass, and inserting a sibling next to the root.

Edits to nodes that another plugin removed in the same pass are still just dropped with a warning, and replacing, removing, or wrapping the root keeps working.
