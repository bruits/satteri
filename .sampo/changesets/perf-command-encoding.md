---
cargo/satteri-ast: minor
cargo/satteri-plugin-api: patch
npm/satteri: patch
---

Plugin mutations are encoded and applied with fewer copies on both sides of the native boundary.

Two behaviors are stricter: a node getter or `toJSON` that triggers a context mutation while its node is being encoded now throws, and a mutation anchored on a stale node id (retained from another or an already-mutated tree) now fails the compile with `invalid node id` instead of being dropped with a console warning.
