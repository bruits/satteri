# satteri-plugin-api

## 0.2.2 — 2026-06-08

### Patch changes

- [f41d32f](https://github.com/bruits/satteri/commit/f41d32f590e7763f7ba8199aead1e563503c8a9a) Fixes `ctx.setProperty(node, "children", [...])`, which used to throw an error. You can now set a node's children directly, and any other properties you set on the same node still take effect. — Thanks @Princesseuh!
- [67ac7b0](https://github.com/bruits/satteri/commit/67ac7b06aa270c22664cfa3c7a11d6bf37495529) Fixes several kinds of nodes getting mangled when a plugin would move or duplicate them. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.3.2

## 0.2.1 — 2026-06-03

### Patch changes

- [c91de73](https://github.com/bruits/satteri/commit/c91de73b75420934819c4488101aa9589be7f39c) Made HAST plugins match MDAST when a transform targets a node removed or replaced earlier in the same pass: the stranded transform is now dropped with a warning instead of throwing a fatal error. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.3.1

## 0.2.0 — 2026-06-02

### Minor changes

- [b8d8fa8](https://github.com/bruits/satteri/commit/b8d8fa8d56cfef1e1c35a5a37e9c61ed421d7bac) Nested directives now transform correctly. When a plugin turns a directive into something else (for example a `containerDirective` visitor that renders both an outer `:::note` and a nested `:::tip` as asides), the inner one is transformed too — in a single pass.
  
  A node returned from a visitor that passes existing children through (e.g. `{ ...node, children: [...node.children] }`) now keeps those children's identity, so a transform queued on a nested one in the same pass still applies. Previously this crashed with `patch targets node N inside a removed subtree`.
  
  Note: a visitor's own freshly-built nodes are not re-walked by that same visitor. Produce their final shape directly, or hand off to a later plugin (which sees the materialized tree). — Thanks @Princesseuh!
- [c69e907](https://github.com/bruits/satteri/commit/c69e9073f3f101faf8058f05f6e6fea4466039fe) Adds an `mdx` cargo feature (enabled by default) across the Rust crates. Disabling it compiles out all MDX support. In the future, this will be used to ship a "lite" version of Sätteri for environments where MDX is not needed and bundle size is a concern.
  
  On Linux the native addon drops from ~2.99 MB to ~1.36 MB when disabling MDX. — Thanks @Princesseuh!

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.2.1, satteri-ast (Cargo)@0.3.0

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

