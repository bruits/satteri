---
npm/satteri: minor
---

`markdownToHtml` and `mdxToJs` now return an object instead of a bare string. The first field carries the rendered output (`html`, or `code` for MDX), and a new `frontmatter` field exposes the first YAML or TOML frontmatter block in the document, or `null` if none.

```js
// Before
const html = markdownToHtml(source);

// After
const { html, frontmatter } = markdownToHtml(source);
```

This makes it easier to then pass the frontmatter to a YAML / TOML library of your choice, without needing to extract it using a plugin.
