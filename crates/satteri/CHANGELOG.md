# satteri

## 0.2.6 — 2026-06-25

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.3.6, satteri-pulldown-cmark (Cargo)@0.5.6

## 0.2.5 — 2026-06-19

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.3.5, satteri-pulldown-cmark (Cargo)@0.5.5

## 0.2.4 — 2026-06-18

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.4.0, satteri-mdxjs (Cargo)@0.3.4, satteri-pulldown-cmark (Cargo)@0.5.4

## 0.2.3 — 2026-06-11

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.3.3, satteri-pulldown-cmark (Cargo)@0.5.3

## 0.2.2 — 2026-06-08

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.3.2, satteri-mdxjs (Cargo)@0.3.2, satteri-pulldown-cmark (Cargo)@0.5.2

## 0.2.1 — 2026-06-03

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.3.1, satteri-mdxjs (Cargo)@0.3.1, satteri-pulldown-cmark (Cargo)@0.5.1

## 0.2.0 — 2026-06-02

### Minor changes

- [c69e907](https://github.com/bruits/satteri/commit/c69e9073f3f101faf8058f05f6e6fea4466039fe) Adds an `mdx` cargo feature (enabled by default) across the Rust crates. Disabling it compiles out all MDX support. In the future, this will be used to ship a "lite" version of Sätteri for environments where MDX is not needed and bundle size is a concern.
  
  On Linux the native addon drops from ~2.99 MB to ~1.36 MB when disabling MDX. — Thanks @Princesseuh!

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.3.0, satteri-mdxjs (Cargo)@0.3.0, satteri-pulldown-cmark (Cargo)@0.5.0

## 0.1.20 — 2026-05-19

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.2.3

## 0.1.19 — 2026-05-18

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.7, satteri-mdxjs (Cargo)@0.2.2, satteri-pulldown-cmark (Cargo)@0.4.1

## 0.1.18 — 2026-05-18

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.2.1, satteri-pulldown-cmark (Cargo)@0.4.0

## 0.1.17 — 2026-05-18

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.2.0

## 0.1.16 — 2026-05-12

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.6, satteri-mdxjs (Cargo)@0.1.16, satteri-pulldown-cmark (Cargo)@0.3.6

## 0.1.15 — 2026-05-06

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.5, satteri-mdxjs (Cargo)@0.1.15, satteri-pulldown-cmark (Cargo)@0.3.5

## 0.1.14 — 2026-04-30

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.4, satteri-mdxjs (Cargo)@0.1.14, satteri-pulldown-cmark (Cargo)@0.3.4

## 0.1.13 — 2026-04-30

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.3, satteri-mdxjs (Cargo)@0.1.13, satteri-pulldown-cmark (Cargo)@0.3.3

## 0.1.12 — 2026-04-29

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.2, satteri-mdxjs (Cargo)@0.1.12, satteri-pulldown-cmark (Cargo)@0.3.2

## 0.1.11 — 2026-04-29

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.1, satteri-mdxjs (Cargo)@0.1.11, satteri-pulldown-cmark (Cargo)@0.3.1

## 0.1.10 — 2026-04-29

### Patch changes

- [baae3b8](https://github.com/bruits/satteri/commit/baae3b83b56bf0fb4cd0b0d2f376627ff0267b8f) Fixed plugins silently dropping all but the last structural change against a given node. Multiple `insertBefore`/`insertAfter` calls on the same node, or sibling inserts paired with a `removeNode` on that same node, now all apply in the order they were issued.
  
  Combinations that don't have a sensible meaning, like modifying something inside a removed subtree, now report an error instead of silently dropping the change. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.2.0, satteri-mdxjs (Cargo)@0.1.10, satteri-pulldown-cmark (Cargo)@0.3.0

## 0.1.9 — 2026-04-27

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.5, satteri-mdxjs (Cargo)@0.1.9, satteri-pulldown-cmark (Cargo)@0.2.5

## 0.1.8 — 2026-04-27

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.4, satteri-mdxjs (Cargo)@0.1.8, satteri-pulldown-cmark (Cargo)@0.2.4

## 0.1.7 — 2026-04-17

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.1.7, satteri-pulldown-cmark (Cargo)@0.2.3

## 0.1.6 — 2026-04-16

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.1.6, satteri-pulldown-cmark (Cargo)@0.2.2

## 0.1.5 — 2026-04-16

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.3, satteri-mdxjs (Cargo)@0.1.5, satteri-pulldown-cmark (Cargo)@0.2.1

## 0.1.4 — 2026-04-16

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.1.4

## 0.1.3 — 2026-04-15

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.1.3

## 0.1.2 — 2026-04-14

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.2, satteri-mdxjs (Cargo)@0.1.2, satteri-pulldown-cmark (Cargo)@0.2.0
