---
cargo/satteri-ast: patch
---

Changed patch errors to name the nodes involved with a `NodeLabel`, adding `ChildrenOnLeaf`, `NodePlacedTwice`, `NodeInsideItself` and `AncestorPlacedInside`, and removing `WrapOnRemovedNode` and `ChildPatchOnRemovedNode`.
