---
cargo/satteri-ast: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Added `{ fragment: true }` to `htmlToHast`, which parses the string as a fragment so the returned `root` holds its own top-level nodes instead of an implied `<html>`/`<head>`/`<body>`.

Pass `space: "svg"` alongside it to read the fragment as foreign content, so `<circle />` self-closes and camel-cased tags like `clipPath` keep their casing instead of parsing as unknown HTML elements.

```ts
import { htmlToHast } from "satteri";

const tree = htmlToHast("<p>hi</p>", { fragment: true });
// { type: "root", children: [{ type: "element", tagName: "p", ... }] }

const icon = htmlToHast(`<circle cx="1" />`, { fragment: true, space: "svg" });
```
