---
npm/satteri: patch
---

Exposes a `ctx.sourceFormat` property on both mdast and hast plugin contexts. It is `"markdown"` when the plugin runs during a Markdown compile (`markdownToHtml`) and `"mdx"` during an MDX compile (`mdxToJs`), letting a plugin shared between both pipelines branch on which format it is handling.
