---
npm/satteri: minor
---

Nodes retained past their visitor pass now read as the tree looked during that pass instead of unconditionally throwing. Reading node content in-pass (a child node's field, or `ctx.parent()`) pins the pass snapshot; the retention error only remains for a node whose content was never read before the tree changed.

Node objects are shared rather than copied, so treat them as read-only and mutate through the context methods; `structuredClone(node)` is still the cheapest way to keep data around after the compile.
