---
npm/satteri: patch
cargo/satteri-plugin-api: patch
---

Fixed edits on a replaced node being lost or failing the compile: a field set on it or a child added to it now lands on its replacement whether it came before or after, and an edit with nowhere to land logs a warning.
