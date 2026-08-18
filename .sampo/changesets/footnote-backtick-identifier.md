---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Fixed text being dropped when one paragraph held two footnote references whose identifier contains a backtick, or a `$` with the math feature enabled. The words between the two references, and the second reference itself, are no longer swallowed.
