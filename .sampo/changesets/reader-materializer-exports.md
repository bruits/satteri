---
npm/satteri: patch
---

Added the `satteri/mdast-reader`, `satteri/mdast-materializer`, `satteri/hast-reader` and `satteri/hast-materializer` entry points, which turn a serialized tree into plain objects without loading the native or WASI binding.

Also exported `walkMdastHandle` from the root, the walk the visitors use, so a caller can read a few nodes of a parsed tree, such as the root's children and their positions, without serializing all of it.
