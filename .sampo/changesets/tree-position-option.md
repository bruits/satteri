---
npm/satteri: minor
cargo/satteri-napi: minor
---

Added `position: false` to `markdownToMdast`, `mdxToMdast`, `markdownToHast`, and `mdxToHast`, which skips recording `node.position`. On a 1 MB document that halves both the time to build a tree and the memory it occupies, so it is worth passing whenever nothing downstream reads positions.

```ts
const tree = markdownToMdast(source, { position: false });
```
