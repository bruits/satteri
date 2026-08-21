---
npm/satteri: patch
---

Added plugin support to the tree functions: `markdownToMdast` and `mdxToMdast` accept `mdastPlugins`, and `markdownToHast` and `mdxToHast` accept both `mdastPlugins` and `hastPlugins`, alongside `fileURL` and `data` as the compile functions already do.

```ts
const tree = markdownToHast(source, {
  mdastPlugins: [myMdastPlugin],
  hastPlugins: [myHastPlugin],
});
```

As with `markdownToHtml`, an async plugin makes the call return a promise.
