---
cargo/satteri-ast: patch
npm/satteri: patch
---

Fixed `wrapNode()` silently misplacing or dropping the node when given a parent that cannot hold children (an `html` node or raw content); it now throws an error explaining what to use instead.
