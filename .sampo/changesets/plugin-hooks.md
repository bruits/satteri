---
npm/satteri: minor
---

Added `before` and `after` lifecycle hooks to MDAST and HAST plugins: each receives the document root and runs exactly once per document (even an empty one), before or after the plugin's visitors — so a plugin can always emit per-document output like a table-of-contents export.
