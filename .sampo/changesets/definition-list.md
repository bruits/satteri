---
cargo/satteri-pulldown-cmark: minor
cargo/satteri-ast: minor
cargo/satteri-plugin-api: patch
cargo/satteri-napi: minor
npm/satteri: minor
---

Adds a `definitionList` feature (off by default) that renders definition lists to `<dl>`/`<dt>`/`<dd>`.

New `descriptionList` / `descriptionTerm` / `descriptionDetails` nodes are available to plugins when this option is enabled.

```text
Apple
:   Pomaceous fruit.
:   A tech company.
```
