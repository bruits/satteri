---
npm/satteri: minor
---

Added a `rawHtml` feature that reparses raw HTML embedded in Markdown into real HAST nodes. Enable it with `features: { rawHtml: true }` on any entry point; it is applied during the MDAST→HAST conversion, so `markdownToHast`, `markdownToHtml`, and the plugin pipelines all reparse identically, and hast plugins always see the reparsed elements.

The whole tree is reparsed through the HTML parser, so a tag opened in one raw block and closed in another is resolved against the surrounding Markdown. Attributes are normalized into typed hast properties (`class` → `className: [...]`, `disabled` → `true`, `tabindex` → number, `data-foo-bar` → `dataFooBar`). `htmlToHast` normalizes properties the same way.

MDX nodes are passed through the reparse rather than dropped: each JSX element/expression is preserved in place while the surrounding raw HTML is still resolved around it. So `mdxToHast(source, { features: { rawHtml: true } })` keeps its MDX content.

```ts
import { markdownToHast } from "satteri";

const tree = markdownToHast(`<div class="note">\n\n**hi**\n\n</div>`, {
  features: { rawHtml: true },
});
// <div> is a real element wrapping <p><strong>hi</strong></p>
```
