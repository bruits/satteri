---
cargo/satteri-ast: patch
---

Changed placement errors to describe the nodes involved with a `NodeLabel`, adding `CommandError::NodeInsideItself` and `AncestorPlacedInside` in place of the matching `UnsupportedPatchShape` errors.
