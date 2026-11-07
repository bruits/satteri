---
cargo/satteri-ast: patch
---

Changed patch application so a ref moves its target instead of copying it, a node removed in the same batch stays removed whatever refers to it, and a node referred to twice fails with `CommandError::NodePlacedTwice`.
