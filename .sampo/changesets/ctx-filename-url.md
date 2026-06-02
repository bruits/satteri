---
npm/satteri: minor
---

`ctx.filename` and the corresponding `filename` option on the various entrypoints now only accepts URLs instead of strings. It is intended that you create a file URL using `new URL('path/to/file', import.meta.url)`, convert a file path using `pathToFileURL('path/to/file')`, or similar.

This change was made to avoid normalization issues across operating systems, enable the support of virtual paths and just generally promote a more consistent format over raw strings.
