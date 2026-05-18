# satteri-napi

## 0.2.2 — 2026-05-18

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.2.0, satteri-ast (Cargo)@0.2.7, satteri-mdxjs (Cargo)@0.2.2, satteri-plugin-api (Cargo)@0.1.13, satteri-pulldown-cmark (Cargo)@0.4.1

## 0.2.1 — 2026-05-18

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.2.1, satteri-pulldown-cmark (Cargo)@0.4.0

## 0.2.0 — 2026-05-18

### Minor changes

- [f12e64e](https://github.com/bruits/satteri/commit/f12e64e12a5b6cc765252633c16b38f8c21e9282) Added `elementAttributeNameCase` and `stylePropertyNameCase` options. Set `elementAttributeNameCase: "html"` to emit `class`/`for` instead of `className`/`htmlFor`, and `stylePropertyNameCase: "css"` to keep kebab-case keys in `style` objects. Defaults stay React-compatible. — Thanks @Princesseuh!

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.2.0

## 0.1.15 — 2026-05-12

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.6, satteri-mdxjs (Cargo)@0.1.16, satteri-plugin-api (Cargo)@0.1.12, satteri-pulldown-cmark (Cargo)@0.3.6

## 0.1.14 — 2026-05-06

### Patch changes

- [22c4f06](https://github.com/bruits/satteri/commit/22c4f06e8923de01a371db798dbf39022737ad33) Fixes a rare case where plugins could produce corrupted output in very specific situations. — Thanks @Princesseuh!
- Updated dependencies: satteri-arena (Cargo)@0.1.4, satteri-ast (Cargo)@0.2.5, satteri-mdxjs (Cargo)@0.1.15, satteri-plugin-api (Cargo)@0.1.11, satteri-pulldown-cmark (Cargo)@0.3.5

## 0.1.13 — 2026-04-30

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.4, satteri-mdxjs (Cargo)@0.1.14, satteri-plugin-api (Cargo)@0.1.10, satteri-pulldown-cmark (Cargo)@0.3.4

## 0.1.12 — 2026-04-30

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.3, satteri-mdxjs (Cargo)@0.1.13, satteri-plugin-api (Cargo)@0.1.9, satteri-pulldown-cmark (Cargo)@0.3.3

## 0.1.11 — 2026-04-29

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.2, satteri-mdxjs (Cargo)@0.1.12, satteri-plugin-api (Cargo)@0.1.8, satteri-pulldown-cmark (Cargo)@0.3.2

## 0.1.10 — 2026-04-29

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.1, satteri-mdxjs (Cargo)@0.1.11, satteri-plugin-api (Cargo)@0.1.7, satteri-pulldown-cmark (Cargo)@0.3.1

## 0.1.9 — 2026-04-29

### Patch changes

- [baae3b8](https://github.com/bruits/satteri/commit/baae3b83b56bf0fb4cd0b0d2f376627ff0267b8f) Renamed `Options::ENABLE_CONTAINER_EXTENSIONS` to `Options::ENABLE_DIRECTIVE`. If you use this crate directly, update the option name; if you only consume satteri through the npm package or the high-level Rust API, no change is needed (the `features.directive` toggle keeps its name). — Thanks @Princesseuh!
- [baae3b8](https://github.com/bruits/satteri/commit/baae3b83b56bf0fb4cd0b0d2f376627ff0267b8f) Fixed plugins silently dropping all but the last structural change against a given node. Multiple `insertBefore`/`insertAfter` calls on the same node, or sibling inserts paired with a `removeNode` on that same node, now all apply in the order they were issued.
  
  Combinations that don't have a sensible meaning, like modifying something inside a removed subtree, now report an error instead of silently dropping the change. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.2.0, satteri-mdxjs (Cargo)@0.1.10, satteri-plugin-api (Cargo)@0.1.6, satteri-pulldown-cmark (Cargo)@0.3.0

## 0.1.8 — 2026-04-27

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.1.3, satteri-ast (Cargo)@0.1.5, satteri-mdxjs (Cargo)@0.1.9, satteri-plugin-api (Cargo)@0.1.5, satteri-pulldown-cmark (Cargo)@0.2.5

## 0.1.7 — 2026-04-27

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.4, satteri-mdxjs (Cargo)@0.1.8, satteri-plugin-api (Cargo)@0.1.4, satteri-pulldown-cmark (Cargo)@0.2.4

## 0.1.6 — 2026-04-17

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.1.7, satteri-pulldown-cmark (Cargo)@0.2.3

## 0.1.5 — 2026-04-16

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.1.6, satteri-pulldown-cmark (Cargo)@0.2.2

## 0.1.4 — 2026-04-16

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.3, satteri-mdxjs (Cargo)@0.1.5, satteri-plugin-api (Cargo)@0.1.3, satteri-pulldown-cmark (Cargo)@0.2.1

## 0.1.3 — 2026-04-16

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.1.4

## 0.1.2 — 2026-04-15

### Patch changes

- [bfb8968](https://github.com/bruits/satteri/commit/bfb89681df076d683a8c9cf6612b21195b06a566) Added `parseExpression()` to `mdxjsEsm` nodes, allowing ESM import/export statements to be parsed into ESTree ASTs. — Thanks @Princesseuh!
- Updated dependencies: satteri-mdxjs (Cargo)@0.1.3

## 0.1.1 — 2026-04-14

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.2, satteri-mdxjs (Cargo)@0.1.2, satteri-plugin-api (Cargo)@0.1.2, satteri-pulldown-cmark (Cargo)@0.2.0

