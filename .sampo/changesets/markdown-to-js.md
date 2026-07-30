---
cargo/satteri-mdxjs: patch
cargo/satteri-napi: minor
npm/satteri: minor
---

Added `markdownToJs`, the plain-Markdown counterpart to `mdxToJs` — MDX syntax like `{...}` stays literal text.

```ts
import { markdownToJs } from "satteri";

const { code } = markdownToJs("Hello {world}");
```

HTML in the source is dropped. Pass `features: { rawHtml: true }` to parse it into real elements instead.
