---
npm/satteri: minor
---

The `filename` option (and the `ctx.filename` it surfaced to plugins) is now `fileURL` and only accepts a `URL` instead of a string. Create one with `new URL('path/to/file', import.meta.url)`, convert a file path with `pathToFileURL('path/to/file')`, or pass an existing file URL directly.

This change was made to avoid normalization issues across operating systems, enable the support of virtual paths and just generally promote a more consistent format over raw strings.
