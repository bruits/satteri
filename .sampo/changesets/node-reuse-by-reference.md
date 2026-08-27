---
npm/satteri: patch
cargo/satteri-ast: patch
---

Fixed a node passed to `insertBefore`, `insertAfter`, `prependChild` or `appendChild` losing changes other visitors made to it in the same pass, and fixed content disappearing when a plugin moved a node by inserting it elsewhere and then removing it.
