---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

With smart punctuation enabled, an unmatched close-flanking double quote — like the inch mark in `24" monitor` — now renders as a closing curly quote instead of an opening one. A double quote after a digit no longer opens a quotation, so dimension notation like `24"x36"` closes throughout.
