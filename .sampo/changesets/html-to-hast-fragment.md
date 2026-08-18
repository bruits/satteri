---
cargo/satteri-ast: minor
cargo/satteri-napi: minor
npm/satteri: minor
---

Added `{ fragment: true }` to `htmlToHast`, which parses the string as a fragment so the returned `root` holds its own top-level nodes instead of an implied `<html>`/`<head>`/`<body>`.

```ts
import { htmlToHast } from "satteri";

const tree = htmlToHast("<p>hi</p>", { fragment: true });
// { type: "root", children: [{ type: "element", tagName: "p", ... }] }
```
