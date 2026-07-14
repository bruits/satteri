---
npm/satteri: minor
---

Plugins now have to opt into source positions per plugin with `options: { position: true }`. As such, `node.position` is `undefined` in a visitor unless that plugin (or another plugin in the same pipeline) opts in.

The reason for doing so is mostly performance, tracking positions is inherently expensive and cause a lot more node data to be transfered across Rust and JS despite very little plugins actually requiring positions in the first place. Note that this does not affect positions inside errors.
