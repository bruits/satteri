---
cargo/satteri-ast: patch
---

Changed patch application so a node removed in the same batch stays removed when a replaced node's content refers to it, unless the ref is a `REF_KIND_RELIST` ref, which moves it.
