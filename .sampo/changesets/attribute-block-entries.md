---
cargo/satteri-pulldown-cmark: patch
---

Fixed a `{...}` with something other than attributes in it, such as a `{base|reading}` ruby annotation, being read as heading attributes: as in Pandoc, a block counts only if every entry is an id, a class or a key that names an attribute, and stays text otherwise.
