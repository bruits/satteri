# satteri-property-info

## 0.2.0 — 2026-08-18

### Minor changes

- [d8639d6](https://github.com/bruits/satteri/commit/d8639d64efa50f2adf2f88f6a4928559d2a30836) Added a `rawHtml` feature that reparses raw HTML embedded in Markdown into real HAST nodes. Enable it with `features: { rawHtml: true }` on any entry point; it is applied during the MDAST→HAST conversion, so `markdownToHast`, `markdownToHtml`, and the plugin pipelines all reparse identically, and hast plugins always see the reparsed elements.
  
  The whole tree is reparsed through the HTML parser, so a tag opened in one raw block and closed in another is resolved against the surrounding Markdown. Attributes are normalized into typed hast properties (`class` → `className: [...]`, `disabled` → `true`, `tabindex` → number, `data-foo-bar` → `dataFooBar`). `htmlToHast` normalizes properties the same way.
  
  MDX nodes are passed through the reparse rather than dropped: each JSX element/expression is preserved in place while the surrounding raw HTML is still resolved around it. So `mdxToHast(source, { features: { rawHtml: true } })` keeps its MDX content.
  
  ```ts
  import { markdownToHast } from "satteri";
  
  const tree = markdownToHast(`<div class="note">\n\n**hi**\n\n</div>`, {
    features: { rawHtml: true },
  });
  // <div> is a real element wrapping <p><strong>hi</strong></p>
  ```
   — Thanks @IEvangelist for your first contribution 🎉!

### Patch changes

- [f868e26](https://github.com/bruits/satteri/commit/f868e26e8c07a5e30b90b16b554835f73f37d0c0) Fixed React-cased SVG property names like `strokeLinecap` and `strokeLinejoin` leaking into HTML output as-is instead of serializing as `stroke-linecap` / `stroke-linejoin`. — Thanks @gtritchie!
- [0d26ea6](https://github.com/bruits/satteri/commit/0d26ea6d68a29d4de8419423e030076244348c22) Changed the minimum supported Rust version to 1.85, as these crates now build on the 2024 edition. — Thanks @Princesseuh!

