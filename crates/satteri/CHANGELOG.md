# satteri

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
