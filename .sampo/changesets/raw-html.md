---
npm/satteri: minor
---

Added a `rawHtml` feature that reparses raw HTML embedded in Markdown into real HAST nodes — the equivalent of `rehype-raw`. Enable it with `markdownToHast(source, { features: { rawHtml: true } })`.

The whole tree is reparsed through the HTML parser, so a tag opened in one raw block and closed in another is resolved against the surrounding Markdown. Attributes are normalized into hast properties (`class` → `className: [...]`, `disabled` → `true`, `tabindex` → number, `data-foo-bar` → `dataFooBar`) using the same `property-information` tables as `hast-util-from-html`, so the output matches `rehype-raw`. `htmlToHast` now normalizes properties the same way.

```ts
import { markdownToHast } from "satteri";

const tree = markdownToHast(`<div class="note">\n\n**hi**\n\n</div>`, {
  features: { rawHtml: true },
});
// <div> is a real element wrapping <p><strong>hi</strong></p>
```
