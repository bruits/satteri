---
cargo/satteri-ast: patch
---

Changed child edits on a node replaced by one new node to apply to that node, and wraps and child edits with nowhere to land to be reported as dropped instead of failing, removing `CommandError::WrapOnRemovedNode` and `CommandError::ChildPatchOnRemovedNode`.
