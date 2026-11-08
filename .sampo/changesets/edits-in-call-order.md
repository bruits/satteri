---
npm/satteri: patch
cargo/satteri-plugin-api: patch
---

Changed a plugin's edits, its visitors' return values included, to apply in the order they are made, each to the tree the earlier ones left, fixing edits that were lost or failed the compile when the same plugin also replaced, wrapped or returned the node.
