---
npm/satteri: patch
---

Changed a node removed with `ctx.removeNode` to stay removed when it is also replaced, or when its parent is returned as a copy or replaced with content that lists it again, with a warning when a plugin lists a node it removed.
