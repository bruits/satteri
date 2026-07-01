---
npm/satteri: minor
---

Added `htmlToHast`, which parses an HTML string into a HAST tree (elements, text, comments, doctype) using html5ever's spec-compliant tree builder, mirroring `hast-util-from-html` in document mode.

```ts
import { htmlToHast } from "satteri";

const tree = htmlToHast("<p>hi</p>");
// { type: "root", children: [{ type: "element", tagName: "html", ... }] }
```
