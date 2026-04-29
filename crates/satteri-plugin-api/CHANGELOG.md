# satteri-plugin-api

## 0.1.6 — 2026-04-29

### Patch changes

- [baae3b8](https://github.com/bruits/satteri/commit/baae3b83b56bf0fb4cd0b0d2f376627ff0267b8f) Fixed a crash when an MDAST plugin called `ctx.setProperty(node, "data", …)` on certain specific node types (e.g. `paragraph`, `blockquote`, `delete`). The call now succeeds and the data round-trips through the conversion pipeline as expected. — Thanks @Princesseuh!
- [baae3b8](https://github.com/bruits/satteri/commit/baae3b83b56bf0fb4cd0b0d2f376627ff0267b8f) Fixed plugins silently dropping all but the last structural change against a given node. Multiple `insertBefore`/`insertAfter` calls on the same node, or sibling inserts paired with a `removeNode` on that same node, now all apply in the order they were issued.
  
  Combinations that don't have a sensible meaning, like modifying something inside a removed subtree, now report an error instead of silently dropping the change. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.2.0

## 0.1.5 — 2026-04-27

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.1.3, satteri-ast (Cargo)@0.1.5

## 0.1.4 — 2026-04-27

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.4

## 0.1.3 — 2026-04-16

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.3

## 0.1.2 — 2026-04-14

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.2

