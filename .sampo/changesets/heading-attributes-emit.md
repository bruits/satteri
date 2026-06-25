---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Fixes `headingAttributes` silently dropping parsed attributes. Explicit `id=`/`class=`
now merge with the `#`/`.` shorthands instead of emitting duplicates, and attribute
values can be quoted (`{data-x="a b"}`) to include spaces.
