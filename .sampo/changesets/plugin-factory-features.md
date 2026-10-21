---
npm/satteri: minor
---

Added `features` to the plugin factory context, so a plugin can compile a fragment with the same syntax as the document: `markdownToMdast(fragment, { features: ctx.features })`.
