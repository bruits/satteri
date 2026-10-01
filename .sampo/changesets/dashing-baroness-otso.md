---
npm/satteri: patch
---

Fixed `ctx.setField` for structured fields, including MDX JSX and directive attributes, table alignment, and HAST element properties. For example, `ctx.setField(node, "attributes", [])` now clears an MDX JSX element's attributes while preserving its children.
