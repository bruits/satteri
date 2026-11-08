---
npm/satteri: patch
cargo/satteri-plugin-api: patch
---

Changed a plugin's edits to apply in the order it makes them, each to the tree the earlier ones left, with a visitor's return value as its last edit.
