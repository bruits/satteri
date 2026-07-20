---
npm/satteri: minor
---

Added a `rawHtml` feature that reparses raw HTML embedded in Markdown into real HAST nodes — the equivalent of `rehype-raw`. Enable it with `features: { rawHtml: true }` on any entry point; it is applied during the MDAST→HAST conversion, so `markdownToHast`, `markdownToHtml`, and the plugin pipelines all reparse identically, and hast plugins see the reparsed elements (matching `rehype-raw`'s position in a unified pipeline).

The whole tree is reparsed through the HTML parser, so a tag opened in one raw block and closed in another is resolved against the surrounding Markdown. Attributes are normalized into hast properties (`class` → `className: [...]`, `disabled` → `true`, `tabindex` → number, `data-foo-bar` → `dataFooBar`) using the same `property-information` tables as `hast-util-from-html`, so the output matches `rehype-raw`. `htmlToHast` now normalizes properties the same way.

MDX nodes are passed through the reparse rather than dropped, mirroring `rehype-raw`'s `passThrough`: each JSX element/expression is preserved in place while the surrounding raw HTML is still resolved around it. So `mdxToHast(source, { features: { rawHtml: true } })` keeps its MDX content.

```ts
import { markdownToHast } from "satteri";

const tree = markdownToHast(`<div class="note">\n\n**hi**\n\n</div>`, {
  features: { rawHtml: true },
});
// <div> is a real element wrapping <p><strong>hi</strong></p>
```
