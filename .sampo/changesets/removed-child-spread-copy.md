---
npm/satteri: patch
---

Changed a node removed with `ctx.removeNode` to stay removed for the rest of the pass, even when the plugin also inserts it or lists it in a replacement, with a warning when that happens.
