---
cargo/satteri-napi: minor
npm/satteri: minor
---

Improved compile performance when a `features` object is passed to `markdownToHtml`, `mdxToJs`, or `markdownToJs`: roughly 1 µs less per call, about 25% faster on a small document.

Changed `createMdastHandle`, `createMdxMdastHandle`, `createHastHandle`, and `createMdxHastHandle` to take the public `Features` shape. Granular options move from the flat `gfmOptions`, `mathOptions`, and `smartPunctuationOptions` fields into the `gfm`, `math`, and `smartPunctuation` ones:

```ts
createMdastHandle(source, { smartPunctuationOptions: { quotes: false } }); // before
createMdastHandle(source, { smartPunctuation: { quotes: false } }); // after
```
