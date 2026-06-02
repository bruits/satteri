---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Directive labels now render full Markdown. `:::note[Custom **strong** Label]` shows bold text instead of literal `**` markers. Emphasis, links, inline code, and (in MDX) components and expressions all work inside a label now, on container, leaf, and text directives. Previously a label only understood inline code.

Directives that end with an HTML block also close cleanly now. A `:::note` whose last line before the closing fence is `</details>` no longer leaks a stray `:::` into the output.
