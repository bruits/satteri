---
cargo/satteri-pulldown-cmark: minor
cargo/satteri-ast: minor
cargo/satteri-plugin-api: minor
cargo/satteri-napi: minor
npm/satteri: minor
---

Added a `definitionList` feature (off by default) that renders definition lists to `<dl>`/`<dt>`/`<dd>`, in both tight and loose variants. The new `descriptionList` / `descriptionTerm` / `descriptionDetails` nodes are available to plugins, and a `:::` directive fence still parses as a directive when both extensions are enabled.

```text
Apple
:   Pomaceous fruit.
:   A tech company.
```
