# satteri-mdxjs

## 0.1.8 — 2026-04-27

### Patch changes

- [5736ca4](https://github.com/bruits/satteri/commit/5736ca45dd3eaf703e6d573f19274b42f1ca6cb9) Fixes many output inconsistencies with remark across Markdown, GFM, and MDX parsing, mostly found by extensive property-based fuzz testing. Notable areas: GFM bare-URL detection, MDX JSX flow vs inline classification, footnote numbering and section ordering, directive label inline parsing, list spread/tight handling, and reference link spans. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.1.4, satteri-pulldown-cmark (Cargo)@0.2.4

## 0.1.7 — 2026-04-17

### Patch changes

- [11ffcfc](https://github.com/bruits/satteri/commit/11ffcfca6c8486a3744e37e0c19e78100925323e) Fixed unclosed `{` in a paragraph silently consuming later blocks as an MDX expression, and fixed literal `{` inside code spans being falsely reported as an unclosed MDX expression — Thanks @Princesseuh!
- Updated dependencies: satteri-pulldown-cmark (Cargo)@0.2.3

## 0.1.6 — 2026-04-16

### Patch changes

- [6f9f66f](https://github.com/bruits/satteri/commit/6f9f66fa75722c0b58f50783b5ac85fefd53a157) Fixed JSX inside MDX expression bodies, JSX inside `.map()` callbacks or other expressions is now compiled to `_jsx()` calls instead of being dropped or emitted as raw JSX — Thanks @Princesseuh!
- Updated dependencies: satteri-pulldown-cmark (Cargo)@0.2.2

## 0.1.5 — 2026-04-16

### Patch changes

- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed hyphenated JSX element names (e.g. `<my-widget>`) written explicitly in MDX being incorrectly routed through the components provider and producing invalid JavaScript — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.1.3, satteri-pulldown-cmark (Cargo)@0.2.1

## 0.1.4 — 2026-04-16

### Patch changes

- [ae83450](https://github.com/bruits/satteri/commit/ae83450e535f965d45be64aa83bc12806acb827b) Fixed optimizeStatic silently collapsing elements that have runtime component overrides via `export const components` — Thanks @Princesseuh!

## 0.1.3 — 2026-04-15

### Patch changes

- [bfb8968](https://github.com/bruits/satteri/commit/bfb89681df076d683a8c9cf6612b21195b06a566) Added `parseExpression()` to `mdxjsEsm` nodes, allowing ESM import/export statements to be parsed into ESTree ASTs. — Thanks @Princesseuh!

## 0.1.2 — 2026-04-14

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.2, satteri-pulldown-cmark (Cargo)@0.2.0

