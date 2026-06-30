---
npm/satteri: minor
---

Plugins now opt into source positions per plugin with `options: { position: true }`. Source-position tracking is off by default (skipping ~15% of parse), so `node.position` is `undefined` in a visitor unless that plugin — or another plugin in the same pipeline — opts in. Plugins that read `node.position` must add the flag.
