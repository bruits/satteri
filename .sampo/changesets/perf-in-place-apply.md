---
cargo/satteri-ast: minor
cargo/satteri-plugin-api: patch
npm/satteri: patch
---

Structural plugin commands now apply in place, replacing the whole-arena rebuild: mutation cost is proportional to the number of edits rather than the document size (3 edits on a 115KB document drop from ~160µs to under 50µs). The in-place path covers replacements (both keep-children forms), removals, sibling inserts, child-list edits, wraps, root child edits, and the full node-passthrough ref algebra including duplication and self-references; its equivalence to the old rebuild was certified by a million-round differential fuzzer before the rebuild was deleted. Breaking: `satteri_ast::rebuild::{rebuild, rebuild_lenient}` are removed, `apply_patches_in_place` returns `Result<ApplyResult, CommandError>` directly, and a handful of degenerate re-entrant patch shapes (payload refs naming their own anchor's ancestors, ref-dependency cycles, Replace/Remove/Wrap/sibling-inserts anchored on the root) now fail with the new `CommandError::UnsupportedPatchShape` instead of being resolved by the rebuild's recursive splicer. No shape exercised by the plugin test suites is affected.
