---
cargo/satteri-ast: patch
---

Changed patches to apply one at a time in the order given, through the new `PatchApplier`, with a ref moving its target instead of copying it, `KEEP_CHILDREN_NODE_TYPE` replacing `Patch::Replace`'s `keep_children`, and `REF_KIND_SLOT` removed.
