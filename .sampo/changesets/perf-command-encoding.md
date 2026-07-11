---
cargo/satteri-ast: minor
cargo/satteri-plugin-api: patch
npm/satteri: patch
---

Streamlines the plugin mutation wire on both sides. On the JS side, structural payloads are op-stream-encoded directly into the command buffer (the intermediate op-stream buffer and its per-command copy are gone). On the Rust side, op-stream payloads replay straight into the target arena as orphan subtrees instead of building a throwaway mini-arena per command, which also eliminates the string-ref remap pass since payload strings are allocated into the main pool from the start. `Patch` payloads are now `PatchContent::Tree` (owned trees, used by raw markdown/HTML payloads and the native plugin API) or `PatchContent::Grafted` (pre-replayed subtree roots); a million-round differential fuzzer verifies the two payload paths produce identical results. Two behaviors are stricter than before: a node getter or `toJSON` that invokes any context mutation while a structural payload is being encoded now throws (the single-buffer design cannot interleave commands into an open payload; previously only nested structural compiles were rejected), and a command anchored on a node id from a different or already-renumbered handle now fails the compile with `invalid node id` instead of being silently dropped with a console warning, since a stale id could otherwise alias a freshly grafted payload node.
