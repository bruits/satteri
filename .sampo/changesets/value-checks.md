---
npm/satteri: patch
---

Changed attribute, property, `data.hProperties`, `align`, heading `depth` and `tagName` values to be checked the same way whichever way they are set, throwing on a value that does not fit instead of dropping or stringifying it.
