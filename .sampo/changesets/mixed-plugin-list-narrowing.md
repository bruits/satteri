---
npm/satteri: patch
---

Fixed `markdownToHtml`, `mdxToJs` and `markdownToJs` being typed as synchronous when a plugin list mixed sync and async plugins. The result is now correctly typed as a `Promise`.
