---
npm/satteri: patch
---

Changed attribute, property, `data.hProperties`, `align`, heading `depth` and `tagName` values that do not fit, and children added to a node that cannot hold any, such as text, to throw instead of being dropped or stringified, whichever way they are set.
