---
cargo/satteri-mdxjs: patch
cargo/satteri-plugin-api: patch
npm/satteri: minor
---

Plugins now splice strings with a single shape, `{ raw: string, mdxExpressions?: boolean }`, accepted by visitor return values and every structural mutator (`replace`, `insertBefore`, `insertAfter`, `prependChild`, `appendChild`, `wrapNode`). The string is re-parsed in place of the node.

`mdxExpressions` (default `true`) controls what `{…}` means when the document is MDX: live expressions by default, or literal text with `mdxExpressions: false` — the right choice when injecting generated HTML whose braces are not expressions, like a Mermaid decision node `C{JWT valid?}` or math renderer output. Plain Markdown has no expressions, so the option is a no-op there.

`{ rawHtml: string }` is deprecated; it keeps working and behaves exactly like `{ raw, mdxExpressions: false }`.

```ts
defineMdastPlugin({
  code(node) {
    if (node.lang !== "mermaid") return;
    return { raw: renderMermaid(node.value), mdxExpressions: false };
  },
});
```
