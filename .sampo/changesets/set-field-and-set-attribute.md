---
cargo/satteri-plugin-api: minor
npm/satteri: minor
---

Added `ctx.setField` for a node's own fields and `ctx.setAttribute` for `attributes` entries, so plugins can now rename elements, MDX JSX elements and directives, and set directive attributes. `ctx.setProperty` keeps setting HAST element properties; using it for fields or MDX JSX attributes is deprecated, and it is deprecated outright on MDAST.
