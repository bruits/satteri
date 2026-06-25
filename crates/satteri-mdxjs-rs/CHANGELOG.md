# satteri-mdxjs

## 0.3.6 — 2026-06-25

### Patch changes

- Updated dependencies: satteri-pulldown-cmark (Cargo)@0.5.6

## 0.3.5 — 2026-06-19

### Patch changes

- [855379c](https://github.com/bruits/satteri/commit/855379c7eb018e9c5acc69daa7a63f27dbb79e7f) MDX parse errors now carry a source line and column. Previously, errors in `import`/`export` blocks dropped the position entirely, and errors in `{…}` expressions and JSX attributes were reported as a bare byte offset, so downstream tooling reported an unknown location. JSX attribute and spread expression errors now point at the offending attribute rather than the element's opening `<`. — Thanks @Princesseuh!
- Updated dependencies: satteri-pulldown-cmark (Cargo)@0.5.5

## 0.3.4 — 2026-06-18

### Patch changes

- [d6e28f4](https://github.com/bruits/satteri/commit/d6e28f45623a37a74e694cb75e5a6e916c220677) Fixes a parse error when an MDX expression uses top-level `await`, such as `<Card data={await getData()} />`. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.4.0, satteri-pulldown-cmark (Cargo)@0.5.4

## 0.3.3 — 2026-06-11

### Patch changes

- Updated dependencies: satteri-pulldown-cmark (Cargo)@0.5.3

## 0.3.2 — 2026-06-08

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.3.2, satteri-pulldown-cmark (Cargo)@0.5.2

## 0.3.1 — 2026-06-03

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.3.1, satteri-pulldown-cmark (Cargo)@0.5.1

## 0.3.0 — 2026-06-02

### Minor changes

- [8d84807](https://github.com/bruits/satteri/commit/8d84807fe572950f47f0017f68a3b753dd9e90c3) Adds granular `features.gfm` control. Footnotes can now be customized without requiring a plugin. `backContent` and `backLabel` each accept either a string template or a JS callback `(referenceNumber, rerunIndex) => string` for cases that need to branch on the index.
  
  ```ts
  // Disable footnotes, keep the rest of GFM.
  markdownToHtml(source, { features: { gfm: { footnotes: false } } });
  
  // String templates.
  markdownToHtml(source, {
    features: {
      gfm: {
        footnotes: {
          label: "Notes de bas de page",
          backContent: "↑",
          backLabel: "Retour à la référence {reference}",
        },
      },
    },
  });
  
  // Callbacks for per-backref control.
  markdownToHtml(source, {
    features: {
      gfm: {
        footnotes: {
          backLabel: (n, k) => (k > 1 ? `Retour ${n}-${k}` : `Retour ${n}`),
          backContent: (_n, k) => (k === 1 ? "↑" : `↑${k}`),
        },
      },
    },
  });
  ```
  
  In a string template, `{reference}` expands to the footnote number on the first backref and to `number-K` on repeated backrefs to the same definition. Template mode also appends `<sup>K</sup>` after the back content on reruns; callback mode skips the auto-sup and lets the callback return the final content. — Thanks @Princesseuh!

### Patch changes

- [18f269f](https://github.com/bruits/satteri/commit/18f269f216a8e46240f3e7d71ca52c99aee9a709) Fixed inline `style` custom properties (`--*`) being lowercased in MDX, which broke `var()` references to case-sensitive names like `--tmLabel` — Thanks @Princesseuh!
- Updated dependencies: satteri-arena (Cargo)@0.2.1, satteri-ast (Cargo)@0.3.0, satteri-pulldown-cmark (Cargo)@0.5.0

## 0.2.3 — 2026-05-19

### Patch changes

- [befcaf0](https://github.com/bruits/satteri/commit/befcaf044787316c7f86a98667719a41d79da849) Fix a crash when an MDX file defines a component with `export const`, `export function`, or `export class` and then uses it as a JSX tag. Previously the component would be treated as if it had to come from `props.components`, and rendering threw "Expected component X to be defined" unless you also passed it in. It now resolves to the locally-defined component as expected. — Thanks @Princesseuh!

## 0.2.2 — 2026-05-18

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.2.0, satteri-ast (Cargo)@0.2.7, satteri-pulldown-cmark (Cargo)@0.4.1

## 0.2.1 — 2026-05-18

### Patch changes

- Updated dependencies: satteri-pulldown-cmark (Cargo)@0.4.0

## 0.2.0 — 2026-05-18

### Minor changes

- [f12e64e](https://github.com/bruits/satteri/commit/f12e64e12a5b6cc765252633c16b38f8c21e9282) Added `elementAttributeNameCase` and `stylePropertyNameCase` options. Set `elementAttributeNameCase: "html"` to emit `class`/`for` instead of `className`/`htmlFor`, and `stylePropertyNameCase: "css"` to keep kebab-case keys in `style` objects. Defaults stay React-compatible. — Thanks @Princesseuh!

### Patch changes

- [f12e64e](https://github.com/bruits/satteri/commit/f12e64e12a5b6cc765252633c16b38f8c21e9282) Fixed MDX files that declare a layout via `export { default } from ...` or `export default` not rendering at runtime. — Thanks @Princesseuh!

## 0.1.16 — 2026-05-12

### Patch changes

- [4a189f7](https://github.com/bruits/satteri/commit/4a189f77bdf55ab7b238810673ef88e6374d02a5) Fixed plugin-inserted MDX JSX elements compiling as literal HTML tags instead of routing through `_components`, which prevented user overrides via the `components` prop. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.2.6, satteri-pulldown-cmark (Cargo)@0.3.6

## 0.1.15 — 2026-05-06

### Patch changes

- [22c4f06](https://github.com/bruits/satteri/commit/22c4f06e8923de01a371db798dbf39022737ad33) Fixes a rare case where plugins could produce corrupted output in very specific situations. — Thanks @Princesseuh!
- Updated dependencies: satteri-arena (Cargo)@0.1.4, satteri-ast (Cargo)@0.2.5, satteri-pulldown-cmark (Cargo)@0.3.5

## 0.1.14 — 2026-04-30

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.4, satteri-pulldown-cmark (Cargo)@0.3.4

## 0.1.13 — 2026-04-30

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.3, satteri-pulldown-cmark (Cargo)@0.3.3

## 0.1.12 — 2026-04-29

### Patch changes

- [bf7c5a0](https://github.com/bruits/satteri/commit/bf7c5a0bb9865f8147ea6b0815558df3ece0de08) Fixed SVG attributes names (e.g. `viewBox`, `fillOpacity`) being converted to lowercase when set on elements from JS plugins — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.2.2, satteri-pulldown-cmark (Cargo)@0.3.2

## 0.1.11 — 2026-04-29

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.1, satteri-pulldown-cmark (Cargo)@0.3.1

## 0.1.10 — 2026-04-29

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.0, satteri-pulldown-cmark (Cargo)@0.3.0

## 0.1.9 — 2026-04-27

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.1.3, satteri-ast (Cargo)@0.1.5, satteri-pulldown-cmark (Cargo)@0.2.5

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

