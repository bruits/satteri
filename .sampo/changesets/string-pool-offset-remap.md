---
npm/satteri: patch
---

Improved `markdownToMdast` and `markdownToHast` performance on documents containing non-ASCII characters, which previously fell off a fast path and cost more than twice as much to decode.
