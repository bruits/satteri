---
cargo/satteri-ast: minor
cargo/satteri-mdxjs: minor
cargo/satteri-napi: minor
npm/satteri: minor
---

Adds granular `features.gfm` control. Footnotes can now be customized without requiring a plugin. `backContent` and `backLabel` each accept either a string template or a JS callback `(referenceNumber, rerunIndex) => string` for cases that need to branch on the index.

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

In a string template, `{reference}` expands to the footnote number on the first backref and to `number-K` on repeated backrefs to the same definition. Template mode also appends `<sup>K</sup>` after the back content on reruns; callback mode skips the auto-sup and lets the callback return the final content.
