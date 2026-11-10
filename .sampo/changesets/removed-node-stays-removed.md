---
npm/satteri: patch
---

Changed a node removed with `ctx.removeNode` to stay removed for the rest of the pass, with a warning if the plugin also inserts it or lists it in a replacement.
