---
npm/satteri: minor
---

Nodes handed to plugins are shared and now frozen: writing to a node's fields, `position`, `properties`/`attributes`, or `children` throws a `TypeError` instead of silently corrupting what later plugins see — go through the context methods to make changes.

Keeping a node around after your visitor ran now works: it reads as the tree looked at that moment, instead of always throwing. The error only remains if you never read the node's content before the tree changed. Trees returned by `markdownToMdast`/`mdxToMdast`/`markdownToHast`/`mdxToHast` are your own data and stay fully mutable.
