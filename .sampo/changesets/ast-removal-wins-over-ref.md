---
cargo/satteri-ast: patch
---

Changed patch application so a ref moves its target instead of copying it, a node removed earlier in the batch stays removed whatever refers to it later, and a node referred to twice in one payload fails with `CommandError::NodePlacedTwice`.
