---
cargo/satteri-ast: patch
---

Changed placement errors to describe the nodes involved with a `NodeLabel`, adding `CommandError::NodeInsideItself`, `AncestorPlacedInside` and `CircularPlacement` in place of the matching `UnsupportedPatchShape` errors.
