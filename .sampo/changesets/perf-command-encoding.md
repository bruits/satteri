---
cargo/satteri-ast: minor
cargo/satteri-plugin-api: patch
npm/satteri: patch
---

Streamlines the plugin mutation wire on both sides. On the JS side, structural payloads are op-stream-encoded directly into the command buffer (the intermediate op-stream buffer and its per-command copy are gone). On the Rust side, op-stream payloads replay straight into the target arena as orphan subtrees instead of building a throwaway mini-arena per command, which also eliminates the string-ref remap pass since payload strings are allocated into the main pool from the start. `Patch` payloads are now `PatchContent::Tree` (owned trees, used by raw markdown/HTML payloads and the native plugin API) or `PatchContent::Grafted` (pre-replayed subtree roots); a million-round differential fuzzer verifies the two payload paths produce identical results.
