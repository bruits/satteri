---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-ast: patch
cargo/satteri-plugin-api: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Adds a `definitionList` feature (off by default) that renders definition lists to `<dl>`/`<dt>`/`<dd>`. 

New `descriptionList` / `descriptionTerm` / `descriptionDetails` nodes are available to plugins when this option is enabled.

```text
Apple
:   Pomaceous fruit.
:   A tech company.
```
