---
cargo/satteri-ast: minor
cargo/satteri-plugin-api: minor
cargo/satteri-napi: minor
npm/satteri: patch
---

Nested directives now transform correctly. When a plugin turns a directive into something else (for example a `containerDirective` visitor that renders both an outer `:::note` and a nested `:::tip` as asides), the inner one is transformed too.

Previously this crashed with `patch targets node N inside a removed subtree`. Replacing the outer directive no longer conflicts with transforming something inside it; the inner transform runs against the new shape.
