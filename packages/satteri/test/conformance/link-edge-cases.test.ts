// Conformance tests that pin down behavior for edge-case reference/link inputs
// where the internal Rust test expectations previously diverged from remark.
// Each case here is the remark-authoritative shape; if these start failing,
// it means satteri drifted away from remark, not that remark changed.

import { describe, test } from "vitest";
import {
  assertHtmlConformance,
  assertExtMdastConformance,
  assertExtMdastConformanceNoPosition,
  assertMdastConformance,
} from "./helpers.js";

describe("HTML conformance: malformed reference definitions fall back to paragraphs", () => {
  test("blank line inside refdef label — bare URL autolinks in trailing paragraph", () => {
    // regression_test_119: the blank line breaks the would-be `[x\...]:` label,
    // so `]: https://...` is plain text and GFM autolinks the URL.
    assertHtmlConformance("[x\\\n\n]: https://rust-lang.org\n");
  });

  test("setext H2 underline breaks refdef label", () => {
    // regression_test_123: `----------` between `[First try` and `Second try]:`
    // converts the first line to an H2; the leftover becomes a paragraph
    // where the bare URL autolinks.
    assertHtmlConformance("[First try\n----------\nSecond try]: https://rust-lang.org\n");
  });

  test("setext H2 underline breaks refdef label then reference below", () => {
    // regression_test_138: same pattern twice; the second `[first\n-\nsecond]`
    // has no matching definition (first was consumed as H2 + paragraph) so
    // it stays as literal brackets.
    assertHtmlConformance("[first\n-\nsecond]: https://example.com\n\n[first\n-\nsecond]\n");
  });
});

describe("MDAST conformance: autolink-literal vs directive inside broken link labels", () => {
  test("unmatched `[` + `:port` after URL host: directive wins, URL stays text", () => {
    // From docs/src/content/docs/ru/guides/testing.mdx: an inline `[...]`
    // attempt with nested code spans never resolves into a link, so remark
    // keeps the `:4321` as a textDirective and doesn't autolink
    // `http://localhost:4321` in the post-transform pass (domain has no `.`).
    // Satteri has to mirror both choices or the resulting mdast diverges.
    assertExtMdastConformance('[``x``:"http://localhost:4321"`](https://a.com/) z `foo`.', [
      "directive",
    ]);
  });

  test("unmatched `[` in directive: no autolink, no port merge", () => {
    // Minimal shape of the same bug: the leading `[` in a sibling text node
    // is what flips remark into the stricter no-autolink path.
    assertExtMdastConformance('[`a`:"http://localhost:4321" end', ["directive"]);
  });
});

describe("MDAST conformance: GFM autolink-literal trim-back split", () => {
  test("unclosed `[` + URL: remark splits `),` from the trailing text", () => {
    // From docs/src/content/docs/de/guides/cms/index.mdx inside `:::tip`:
    // `Hello [label(https://host/path), rest` — remark emits the trimmed-back
    // `),` as its own text node when an earlier unclosed `[` is present.
    // Position-stripped: remark's post-transform autolink nodes don't carry
    // a position for the synthesized link, while ours do.
    assertExtMdastConformanceNoPosition(
      "Hello [von der Community gepflegte Integrationen(https://astro.build/integrations/?search=cms), um.",
      [],
    );
  });
});

describe("HTML conformance: autolink literal rejects control characters", () => {
  test("angle-bracket autolink with embedded BEL — literal text, no link", () => {
    // regression_test_48: `<http://\x07>` — the `<...>` autolink form rejects
    // the control char, and GFM's autolink-literal post-pass must also bail
    // so the sequence stays as literal `&lt;http://\x07&gt;`. Previously our
    // post-pass kept reading past control chars and turned the tail into
    // `<a href="http://%07%3E">http://\x07></a>`, diverging from remark.
    assertHtmlConformance("<http://\x07>\n");
  });
});

describe("HTML conformance: deflist-shaped input without the deflist extension", () => {
  test("`* item\\n\\n  : body` renders as a loose list with a `:`-prefixed paragraph", () => {
    // regression_test_202: pulldown-cmark's deflist extension produces a
    // nested `<p><p>…</p></p>` here. Our AST layer doesn't expose deflist
    // and remark has no deflist support, so the conformance shape is the
    // plain two-paragraph list item both engines agree on with deflist off.
    assertHtmlConformance("* def this\n\n  : def text def text\n");
  });
});

describe("HTML conformance: malformed reference definition labels with footnote", () => {
  test("`[label[^fn]]: URL` — invalid refdef label, URL autolinks in trailing paragraph", () => {
    // footnotes_test_16: a label containing an unescaped `[…]` isn't a valid
    // CommonMark reference definition, so the line stays as a paragraph with
    // the footnote ref and a GFM autolinked URL after `]: `.
    assertHtmlConformance(
      "My [otherlink[^c]].\n\n[^c]: foo.\n\n[otherlink[^c]]: https://example.com/path\n",
    );
  });
});

describe("HTML conformance: malformed inline links fall back to paragraphs", () => {
  test("nested sublist marker breaks `[text](url)` across lines", () => {
    // regression_test_153: the `- -` nested list marker aborts the link, so
    // the trailing `](url)` is text and GFM autolinks the URL.
    assertHtmlConformance("- [foo\n  - -\n  baz](https://example.com)\n");
  });

  test("parens nesting beyond pulldown-cmark's balance limit rejects the link", () => {
    // regression_test_197: the first `[30](...)` has 30 nested parens and
    // still parses; the second with 40 exceeds the balance limit so its
    // `](url)` stays as text with the URL autolinked.
    assertHtmlConformance(
      "[30](https://rust.org/something%3A((((((((((((((((((((((((((((((())))))))))))))))))))))))))))))))\n[40](https://rust.org/something%3A((((((((((((((((((((((((((((((((((((((((())))))))))))))))))))))))))))))))))))))))))\n",
    );
  });

  test("fenced code block inside a list item splits a `[text](url)` link", () => {
    // regression_test_205: the ` ```rust ``` ` fence interrupts inline
    // parsing, leaving the `[...](https://...)` split across block boundaries.
    assertHtmlConformance(
      "- Item definition [it\n  ```rust\n  ```\n  stuff](https://example.com)\n",
    );
  });
});

describe("HTML conformance: YAML metadata block edge cases", () => {
  test("YAML frontmatter with leading blank line consumes the whole block", () => {
    // metadata_blocks_test_4: `---\n\ntitle:...\n---\n` — with frontmatter
    // enabled, the block parses as a `yaml` node and renders no HTML.
    assertHtmlConformance("---\n\ntitle: example\nanother_field: 0\n---\n");
  });

  test("`---` after a paragraph isn't a frontmatter start", () => {
    // metadata_blocks_test_6: frontmatter only opens at offset 0, so the
    // inner `---` fence is read as a thematic break and the `title:`/`---`
    // tail becomes a setext H2.
    assertHtmlConformance("My paragraph here.\n\n---\ntitle: example\nanother_field: 0\n---\n");
  });
});

describe("MDAST conformance: edge-case reference parsing", () => {
  test("blank line inside refdef label — no definition node emitted", () => {
    // The parser walks away without a `definition`, leaving two paragraphs.
    assertMdastConformance("[x\\\n\n]: https://rust-lang.org\n");
  });

  test("setext underline breaks label — first line becomes heading", () => {
    assertMdastConformance("[First try\n----------\nSecond try]: https://rust-lang.org\n");
  });

  test("fenced code block inside list item breaks inline link", () => {
    assertMdastConformance(
      "- Item definition [it\n  ```rust\n  ```\n  stuff](https://example.com)\n",
    );
  });

  // Note: `- [foo\n  - -\n  baz](url)` renders the same HTML as remark (the
  // HTML test above checks that), but positions inside the nested-list
  // sub-tree currently differ. Not covered as mdast conformance until that
  // gap is closed.

  test("YAML frontmatter with leading blank line — one yaml node at root", () => {
    assertExtMdastConformance("---\n\ntitle: example\nanother_field: 0\n---\n", ["frontmatter"]);
  });
});
