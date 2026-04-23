// Conformance tests that pin down behavior for edge-case reference/link inputs
// where the internal Rust test expectations previously diverged from remark.
// Each case here is the remark-authoritative shape; if these start failing,
// it means satteri drifted away from remark, not that remark changed.

import { describe, test } from "vitest";
import {
  assertHtmlConformance,
  assertExtMdastConformance,
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
    assertHtmlConformance(
      "[First try\n----------\nSecond try]: https://rust-lang.org\n",
    );
  });

  test("setext H2 underline breaks refdef label then reference below", () => {
    // regression_test_138: same pattern twice; the second `[first\n-\nsecond]`
    // has no matching definition (first was consumed as H2 + paragraph) so
    // it stays as literal brackets.
    assertHtmlConformance(
      "[first\n-\nsecond]: https://example.com\n\n[first\n-\nsecond]\n",
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
    assertHtmlConformance(
      "My paragraph here.\n\n---\ntitle: example\nanother_field: 0\n---\n",
    );
  });
});

describe("MDAST conformance: edge-case reference parsing", () => {
  test("blank line inside refdef label — no definition node emitted", () => {
    // The parser walks away without a `definition`, leaving two paragraphs.
    assertMdastConformance("[x\\\n\n]: https://rust-lang.org\n");
  });

  test("setext underline breaks label — first line becomes heading", () => {
    assertMdastConformance(
      "[First try\n----------\nSecond try]: https://rust-lang.org\n",
    );
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
    assertExtMdastConformance(
      "---\n\ntitle: example\nanother_field: 0\n---\n",
      ["frontmatter"],
    );
  });
});
