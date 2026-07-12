---
npm/satteri: minor
---

Nodes handed to plugins on the walk path are now frozen: assigning to a node's fields, its `position`, or its `properties`/`attributes` throws a `TypeError` instead of silently corrupting the shared per-pass node cache that every later plugin reads. Node objects were already typed `Readonly` and documented as shared; this makes the contract enforced at runtime. Freezing is designed to stay off the hot path: leaves freeze at materialization, containers when their children are first read, and child arrays stay unfrozen (frozen-array element access is what V8 penalizes), keeping deep-read chains within a few percent of the unfrozen path. Trees returned by `markdownToMdast`/`mdxToMdast`/`markdownToHast`/`mdxToHast` are the caller's own data and remain fully mutable.
