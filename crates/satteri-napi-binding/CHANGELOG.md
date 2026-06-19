# satteri-napi

## 0.4.4 — 2026-06-19

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.3.5, satteri-pulldown-cmark (Cargo)@0.5.5

## 0.4.3 — 2026-06-18

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.4.0, satteri-mdxjs (Cargo)@0.3.4, satteri-plugin-api (Cargo)@0.3.0, satteri-pulldown-cmark (Cargo)@0.5.4

## 0.4.2 — 2026-06-11

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.3.3, satteri-pulldown-cmark (Cargo)@0.5.3

## 0.4.1 — 2026-06-08

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.3.2, satteri-mdxjs (Cargo)@0.3.2, satteri-plugin-api (Cargo)@0.2.2, satteri-pulldown-cmark (Cargo)@0.5.2

## 0.4.0 — 2026-06-03

### Minor changes

- [5b45ec8](https://github.com/bruits/satteri/commit/5b45ec89862fd675070006ec7b8c3c64bee408ed) Disabled math parsing by default; pass `math: true` to re-enable inline `$...$` and display `$$...$$` math. — Thanks @Princesseuh!

### Patch changes

- [c91de73](https://github.com/bruits/satteri/commit/c91de73b75420934819c4488101aa9589be7f39c) Made HAST plugins match MDAST when a transform targets a node removed or replaced earlier in the same pass: the stranded transform is now dropped with a warning instead of throwing a fatal error. — Thanks @Princesseuh!
- [c91de73](https://github.com/bruits/satteri/commit/c91de73b75420934819c4488101aa9589be7f39c) Fixed `ctx.wrapNode()` dropping content: the wrapper's own children are now kept after the wrapped node, and `prependChild`/`appendChild` calls on a node in the same pass it is wrapped are applied instead of being silently dropped. — Thanks @Princesseuh!
- [c91de73](https://github.com/bruits/satteri/commit/c91de73b75420934819c4488101aa9589be7f39c) Fixed a crash when a plugin returned a replacement node whose children included the node being visited (for example, wrapping a heading in a `<div>` that contains it). — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.3.1, satteri-mdxjs (Cargo)@0.3.1, satteri-plugin-api (Cargo)@0.2.1, satteri-pulldown-cmark (Cargo)@0.5.1

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
- [8d84807](https://github.com/bruits/satteri/commit/8d84807fe572950f47f0017f68a3b753dd9e90c3) Adds granular `features.math` control. `singleDollarTextMath: false` keeps single-`$` constructs as literal text (so prose can carry currency like "$50 to $100") while `$$ ... $$` still parses as display math.
  
  ```ts
  markdownToHtml(source, {
    features: { math: { singleDollarTextMath: false } },
  });
  ```
   — Thanks @Princesseuh!
- [b8d8fa8](https://github.com/bruits/satteri/commit/b8d8fa8d56cfef1e1c35a5a37e9c61ed421d7bac) Nested directives now transform correctly. When a plugin turns a directive into something else (for example a `containerDirective` visitor that renders both an outer `:::note` and a nested `:::tip` as asides), the inner one is transformed too — in a single pass.
  
  A node returned from a visitor that passes existing children through (e.g. `{ ...node, children: [...node.children] }`) now keeps those children's identity, so a transform queued on a nested one in the same pass still applies. Previously this crashed with `patch targets node N inside a removed subtree`.
  
  Note: a visitor's own freshly-built nodes are not re-walked by that same visitor. Produce their final shape directly, or hand off to a later plugin (which sees the materialized tree). — Thanks @Princesseuh!
- [c69e907](https://github.com/bruits/satteri/commit/c69e9073f3f101faf8058f05f6e6fea4466039fe) Adds an `mdx` cargo feature (enabled by default) across the Rust crates. Disabling it compiles out all MDX support. In the future, this will be used to ship a "lite" version of Sätteri for environments where MDX is not needed and bundle size is a concern.
  
  On Linux the native addon drops from ~2.99 MB to ~1.36 MB when disabling MDX. — Thanks @Princesseuh!

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.2.1, satteri-ast (Cargo)@0.3.0, satteri-mdxjs (Cargo)@0.3.0, satteri-plugin-api (Cargo)@0.2.0, satteri-pulldown-cmark (Cargo)@0.5.0

## 0.2.3 — 2026-05-19

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.2.3

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

