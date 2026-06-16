# satteri-ast

## 0.3.2 — 2026-06-08

### Patch changes

- [f41d32f](https://github.com/bruits/satteri/commit/f41d32f590e7763f7ba8199aead1e563503c8a9a) Fixes `ctx.setProperty(node, "children", [...])`, which used to throw an error. You can now set a node's children directly, and any other properties you set on the same node still take effect. — Thanks @Princesseuh!
- [67ac7b0](https://github.com/bruits/satteri/commit/67ac7b06aa270c22664cfa3c7a11d6bf37495529) Fixes `ctx.textContent()` not including inline math. A heading like `# Energy $E=mc^2$` would only return `Energy ` instead of `Energy E=mc^2`. — Thanks @Princesseuh!
- [67ac7b0](https://github.com/bruits/satteri/commit/67ac7b06aa270c22664cfa3c7a11d6bf37495529) Fixes several kinds of nodes getting mangled when a plugin would move or duplicate them. — Thanks @Princesseuh!
- [7979f1e](https://github.com/bruits/satteri/commit/7979f1ec93695a8b700272f75be967bdba29452b) Fixes a crash when a plugin replaces a node with a tree containing an empty text node in a document that has non-ASCII characters (e.g. `é`). — Thanks @HiDeoo for your first contribution 🎉!

## 0.3.1 — 2026-06-03

### Patch changes

- [c91de73](https://github.com/bruits/satteri/commit/c91de73b75420934819c4488101aa9589be7f39c) Fixed `ctx.wrapNode()` dropping content: the wrapper's own children are now kept after the wrapped node, and `prependChild`/`appendChild` calls on a node in the same pass it is wrapped are applied instead of being silently dropped. — Thanks @Princesseuh!
- [c91de73](https://github.com/bruits/satteri/commit/c91de73b75420934819c4488101aa9589be7f39c) Fixed a crash when a plugin returned a replacement node whose children included the node being visited (for example, wrapping a heading in a `<div>` that contains it). — Thanks @Princesseuh!

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
- [b8d8fa8](https://github.com/bruits/satteri/commit/b8d8fa8d56cfef1e1c35a5a37e9c61ed421d7bac) Nested directives now transform correctly. When a plugin turns a directive into something else (for example a `containerDirective` visitor that renders both an outer `:::note` and a nested `:::tip` as asides), the inner one is transformed too — in a single pass.
  
  A node returned from a visitor that passes existing children through (e.g. `{ ...node, children: [...node.children] }`) now keeps those children's identity, so a transform queued on a nested one in the same pass still applies. Previously this crashed with `patch targets node N inside a removed subtree`.
  
  Note: a visitor's own freshly-built nodes are not re-walked by that same visitor. Produce their final shape directly, or hand off to a later plugin (which sees the materialized tree). — Thanks @Princesseuh!
- [c69e907](https://github.com/bruits/satteri/commit/c69e9073f3f101faf8058f05f6e6fea4466039fe) Adds an `mdx` cargo feature (enabled by default) across the Rust crates. Disabling it compiles out all MDX support. In the future, this will be used to ship a "lite" version of Sätteri for environments where MDX is not needed and bundle size is a concern.
  
  On Linux the native addon drops from ~2.99 MB to ~1.36 MB when disabling MDX. — Thanks @Princesseuh!

### Patch changes

- [c69e907](https://github.com/bruits/satteri/commit/c69e9073f3f101faf8058f05f6e6fea4466039fe) Fixes Markdown plugins that return raw Markdown or HTML (`{ raw }` / `{ rawHtml }`) sometimes inserting unnecessary nested `root` nodes into the MDAST tree. — Thanks @Princesseuh!
- [d6badad](https://github.com/bruits/satteri/commit/d6badad93105125904caeded0907f0c094b58fbd) Fixes `position` property always returning `undefined` on hast nodes. — Thanks @Princesseuh!
- Updated dependencies: satteri-arena (Cargo)@0.2.1

## 0.2.7 — 2026-05-18

### Patch changes

- [43b5d8e](https://github.com/bruits/satteri/commit/43b5d8ed221591de11cf19008be09413425c9612) Fix URL percent-encoding to re-encode bare `%` that isn't a valid escape, and resolve duplicate-identifier reference definitions by source position (first-wins matches remark). — Thanks @Princesseuh!
- Updated dependencies: satteri-arena (Cargo)@0.2.0

## 0.2.6 — 2026-05-12

### Patch changes

- [4a189f7](https://github.com/bruits/satteri/commit/4a189f77bdf55ab7b238810673ef88e6374d02a5) Fixed plugin-inserted MDX JSX elements compiling as literal HTML tags instead of routing through `_components`, which prevented user overrides via the `components` prop. — Thanks @Princesseuh!

## 0.2.5 — 2026-05-06

### Patch changes

- [22c4f06](https://github.com/bruits/satteri/commit/22c4f06e8923de01a371db798dbf39022737ad33) Fixes a rare case where plugins could produce corrupted output in very specific situations. — Thanks @Princesseuh!
- Updated dependencies: satteri-arena (Cargo)@0.1.4

## 0.2.4 — 2026-04-30

### Patch changes

- [80d21c8](https://github.com/bruits/satteri/commit/80d21c8b9bc7f7cb2f86c170d4fafac0d5d2a3b7) Fix a crash when an MDAST plugin returns a tree containing a directive
  (`containerDirective` / `leafDirective` / `textDirective`) and the surrounding
  document contains multi-byte text (e.g. Devanagari, CJK). — Thanks @Princesseuh!

## 0.2.3 — 2026-04-30

### Patch changes

- [8e7642c](https://github.com/bruits/satteri/commit/8e7642cde7aa2c1b0e0b9a7676666f2c990ca7da) Fixed compilation crashing with `invalid type: map, expected a sequence` when an MDAST plugin returned a tree containing a directive node (`containerDirective`, `leafDirective`, `textDirective`). Directive children now round-trip through plugins correctly. — Thanks @Princesseuh!

## 0.2.2 — 2026-04-29

### Patch changes

- [bf7c5a0](https://github.com/bruits/satteri/commit/bf7c5a0bb9865f8147ea6b0815558df3ece0de08) Fixed SVG attributes names (e.g. `viewBox`, `fillOpacity`) being converted to lowercase when set on elements from JS plugins — Thanks @Princesseuh!

## 0.2.1 — 2026-04-29

### Patch changes

- [467bdf9](https://github.com/bruits/satteri/commit/467bdf9b523b1ff1f560499c4d4c769e9c888166) Fixed plugin-set `data` being lost or corrupted on MDAST and HAST nodes in certain cases. — Thanks @Princesseuh!

## 0.2.0 — 2026-04-29

### Minor changes

- [baae3b8](https://github.com/bruits/satteri/commit/baae3b83b56bf0fb4cd0b0d2f376627ff0267b8f) MDAST plugins can now set `data.hName`, `data.hProperties`, and `data.hChildren` on a node and have Sätteri render the corresponding HAST element, matching the rehype idiom.
  
  This is especially useful for rendering directives, given a `containerDirective`, an `hName` of `"aside"` and `hProperties` of `{ className: ["note"] }`, satteri will emit `<aside class="note">…</aside>`. — Thanks @Princesseuh!
- [baae3b8](https://github.com/bruits/satteri/commit/baae3b83b56bf0fb4cd0b0d2f376627ff0267b8f) Fixed plugins silently dropping all but the last structural change against a given node. Multiple `insertBefore`/`insertAfter` calls on the same node, or sibling inserts paired with a `removeNode` on that same node, now all apply in the order they were issued.
  
  Combinations that don't have a sensible meaning, like modifying something inside a removed subtree, now report an error instead of silently dropping the change. — Thanks @Princesseuh!

## 0.1.5 — 2026-04-27

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.1.3

## 0.1.4 — 2026-04-27

### Patch changes

- [f632abf](https://github.com/bruits/satteri/commit/f632abf4ac516f1c8bb3fc713f8894cab9be5d8f) Various MDX parsing fixes:
  
  - Fixed non-ASCII content in MDX expressions/JSX inside containers (blockquotes, lists) being corrupted due to byte-by-byte char casting.
  - Fixed MDX-only paragraphs inside blockquotes not being unraveled (producing spurious `<p>` wrappers).
  - Fixed multiple JSX elements on one line only rendering the first element.
  - Multiple other cases of small inconsistencies with `@mdxjs/mdx`, notably in whitespace handling and node positions. — Thanks @Princesseuh!
- [5736ca4](https://github.com/bruits/satteri/commit/5736ca45dd3eaf703e6d573f19274b42f1ca6cb9) Fixes many output inconsistencies with remark across Markdown, GFM, and MDX parsing, mostly found by extensive property-based fuzz testing. Notable areas: GFM bare-URL detection, MDX JSX flow vs inline classification, footnote numbering and section ordering, directive label inline parsing, list spread/tight handling, and reference link spans. — Thanks @Princesseuh!

## 0.1.3 — 2026-04-16

### Patch changes

- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed script and style element contents being entity-escaped, which produced invalid output (e.g. `&lt;` inside `<script>`) — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed HAST property names not being mapped to their HTML attribute names during rendering (e.g. `className` now renders as `class`, `htmlFor` as `for`) — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed source positions being dropped for most node types during mdast-to-hast conversion, so hast plugins now see accurate positions across the tree — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed code blocks missing trailing newlines when using hast plugins — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed footnote references and definitions not being rendered when using hast plugins — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed table column alignment being dropped when using hast plugins — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed `code.value` in the MDAST tree including a trailing newline for well-formed fenced code blocks, which diverged from `remark-parse`. MDAST plugins inspecting `node.value` now see the same bytes as remark. — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed task list classes and checkbox inputs being missing when using hast plugins — Thanks @Princesseuh!

## 0.1.2 — 2026-04-14

### Patch changes

- [893ef59](https://github.com/bruits/satteri/commit/893ef59125e5969f34650ee27c919f1fae29fe62) Fix MDX import/export and expression handling to match the behavior of the original JavaScript implementation:
  
  - Fix `mdxjsEsm` nodes not being delivered to HAST plugin visitors
  - Fix multiline `export` blocks (e.g. objects, arrays) being truncated
  - Fix expression boundaries for edge cases involving comments, template literals, regex, and JSX
  - Report errors for unclosed MDX expressions — Thanks @Princesseuh!
- [ecaeb2c](https://github.com/bruits/satteri/commit/ecaeb2ce18cbe6a7dc46d19bc49a32aa7114a2c5) Add position data to hast nodes. Position information was already stored in the Rust arena during mdast-to-hast conversion, but was never exposed to the JavaScript side. — Thanks @Princesseuh!

