# satteri-plugin-api

## 0.1.13 — 2026-05-18

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.2.0, satteri-ast (Cargo)@0.2.7

## 0.1.12 — 2026-05-12

### Patch changes

- [4a189f7](https://github.com/bruits/satteri/commit/4a189f77bdf55ab7b238810673ef88e6374d02a5) Fixed plugin-inserted MDX JSX elements compiling as literal HTML tags instead of routing through `_components`, which prevented user overrides via the `components` prop. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.2.6

## 0.1.11 — 2026-05-06

### Patch changes

- [22c4f06](https://github.com/bruits/satteri/commit/22c4f06e8923de01a371db798dbf39022737ad33) Fixes a rare case where plugins could produce corrupted output in very specific situations. — Thanks @Princesseuh!
- Updated dependencies: satteri-arena (Cargo)@0.1.4, satteri-ast (Cargo)@0.2.5

## 0.1.10 — 2026-04-30

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.4

## 0.1.9 — 2026-04-30

### Patch changes

- [8e7642c](https://github.com/bruits/satteri/commit/8e7642cde7aa2c1b0e0b9a7676666f2c990ca7da) Fixed compilation crashing with `invalid type: map, expected a sequence` when an MDAST plugin returned a tree containing a directive node (`containerDirective`, `leafDirective`, `textDirective`). Directive children now round-trip through plugins correctly. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.2.3

## 0.1.8 — 2026-04-29

### Patch changes

- [bf7c5a0](https://github.com/bruits/satteri/commit/bf7c5a0bb9865f8147ea6b0815558df3ece0de08) Fixed numeric property values (e.g. `width: 16`, `start: 5`) being silently dropped when set on elements from JS plugins. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.2.2

## 0.1.7 — 2026-04-29

### Patch changes

- [467bdf9](https://github.com/bruits/satteri/commit/467bdf9b523b1ff1f560499c4d4c769e9c888166) Fixed plugin-set `data` being lost or corrupted on MDAST and HAST nodes in certain cases. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.2.1

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

