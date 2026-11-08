---
cargo/satteri-ast: patch
---

Changed patches to apply one at a time in order, each to the tree the earlier ones left, adding `PatchApplier` and `KEEP_CHILDREN_NODE_TYPE` in place of `Patch::Replace`'s `keep_children`, and removing `REF_KIND_SLOT`, `CommandError::WrapOnRemovedNode` and `CommandError::ChildPatchOnRemovedNode`.
