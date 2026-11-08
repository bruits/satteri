---
cargo/satteri-ast: patch
---

Changed patches to apply one at a time in order, each to the tree the earlier ones left, adding `PatchApplier` and removing `REF_KIND_SLOT`, `CommandError::WrapOnRemovedNode` and `CommandError::ChildPatchOnRemovedNode`.
