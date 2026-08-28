---
npm/satteri: patch
---

Fixed `structuredClone` throwing on a node read from the tree, and stopped internal fields riding along on a clone or a spread copy of one.
