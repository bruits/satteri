---
npm/satteri: minor
---

Added `htmlToHast`, which parses an HTML string into a HAST tree (elements, text, comments, doctype) using html5ever's spec-compliant tree builder, in document mode: the result is a `root` wrapping the implied `<html>` subtree.

```ts
import { htmlToHast } from "satteri";

const tree = htmlToHast("<p>hi</p>");
// { type: "root", children: [{ type: "element", tagName: "html", ... }] }
```
