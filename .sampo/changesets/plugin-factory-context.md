---
npm/satteri: minor
npm/vite-plugin-satteri: minor
---

Added a way to run a plugin only on some documents: a plugin factory now receives the file's `fileURL`, `sourceFormat`, `source` and `data`, and can return `null`, `undefined` or `false` to be left out for that document. Those skip values are also accepted anywhere a plugin entry can appear.

```js
const onlyChangelogs = (ctx) =>
  ctx.fileURL?.pathname.endsWith("/CHANGELOG.md") ? rewriteVersions : null;

markdownToHtml(source, { mdastPlugins: [onlyChangelogs, myPlugin] });
```

Anything else in a plugin list now fails with an error naming the option and what it expected.
