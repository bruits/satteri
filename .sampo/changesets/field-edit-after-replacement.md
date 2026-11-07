---
npm/satteri: patch
cargo/satteri-plugin-api: patch
---

Fixed a field set on a node after a visitor returned its replacement being lost, and added a warning when a field edit has no node left to apply to.
