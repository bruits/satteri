---
cargo/satteri-ast: patch
cargo/satteri-plugin-api: patch
npm/satteri: patch
---

Fixed `wrapNode()` silently misplacing or dropping the node when given a parent that cannot hold children — an `html` node, raw content, a leaf-shaped custom node, or a void element like `<img>`. These now throw an error explaining what to use instead, and `parentNode` only accepts parent-capable nodes in TypeScript, so most of them are caught before running.

Wrapping in a container is unchanged: `blockquote` and friends, a HAST element, an MDX JSX element, a custom node declaring a `children` array, and the root itself all keep working.
