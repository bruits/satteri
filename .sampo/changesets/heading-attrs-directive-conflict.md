---
cargo/satteri-pulldown-cmark: patch
---

Fixed heading attribute blocks conflicting with text directives when both
features are enabled. A trailing directive keeps its own `{...}` attributes
instead of having them absorbed by the heading (`## H :badge[x]{variant=y}`),
and a heading attribute block is now recognized whether it sits before or after
a trailing directive (`## H {#id} :badge[x]` and `## H :badge[x] {#id}`).
