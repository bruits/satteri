// Conformance tests that pin down behavior for edge-case reference/link inputs
// where the internal Rust test expectations previously diverged from remark.
// Each case here is the remark-authoritative shape; if these start failing,
// it means satteri drifted away from remark, not that remark changed.

import { describe, test, expect } from "vitest";
import {
  assertHtmlConformance,
  assertExtMdastConformance,
  assertMdastConformance,
  satteriMdast,
} from "./helpers.js";
import type { Link, Paragraph, Root } from "mdast";

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
    assertExtMdastConformance(
      "Hello [von der Community gepflegte Integrationen(https://astro.build/integrations/?search=cms), um.",
      [],
    );
  });
});

describe("MDAST conformance: autolinks in a `](…)` that never becomes a link (#187)", () => {
  test("inner shortcut reference deactivates the opener", () => {
    assertExtMdastConformance("[[x]](https://x.y)\n\n[x]: /", []);
  });

  test("the eslint/markdown `no-bare-urls` case", () => {
    assertExtMdastConformance(
      "[link that [is-a-valid] link](https://example.com)\n\n[is-a-valid]: https://example.com\n",
      [],
    );
  });

  test("inner inline link deactivates the opener", () => {
    assertExtMdastConformance("[[a](/b)](https://x.y)", []);
    assertExtMdastConformance("[x [y](/z) w](https://q.r)", []);
  });

  test("full reference consumes the `]`, so `(…)` stays text", () => {
    assertExtMdastConformance("[foo][bar](https://x.y)\n\n[bar]: /\n", []);
  });

  test("www, email and bare-scheme triggers in the same position", () => {
    assertExtMdastConformance("[[x]](www.x.y)\n\n[x]: /", []);
    assertExtMdastConformance("[[x]](a@b.co)\n\n[x]: /", []);
    assertExtMdastConformance("[[x]](https://x.y) and www.z.w\n\n[x]: /", []);
  });

  test("control: the bracket pair does resolve, so no autolink survives", () => {
    assertExtMdastConformance("[a](https://x.y)", []);
    assertExtMdastConformance("[a](www.x.y)", []);
    assertExtMdastConformance("[a](a@b.co)", []);
    assertExtMdastConformance("![a](https://x.y)", []);
    assertExtMdastConformance("[a](https://x.y)y", []);
    assertExtMdastConformance("[a](https://x.y)_y_", []);
    assertExtMdastConformance("[a](https://x.y 'ti')", []);
  });

  test("control: ordinary autolinks keep their positions", () => {
    assertExtMdastConformance("www.x.y", []);
    assertExtMdastConformance("https://x.y", []);
    assertExtMdastConformance("a@b.com", []);
    assertExtMdastConformance("see https://x.y/p, and www.z.w.", []);
  });

  test("an autolink overrunning the candidate `)` takes the construct path", () => {
    // `[x]` resolves as a reference link, leaving the outer `[` unmatched, so
    // `(https://x.y)x` is never a destination and the URL runs past the `)`.
    const md = "[[x]](https://x.y)x\n\n[x]: /";
    assertMdastConformance(md);
    const findLink = (tree: unknown) => {
      const paragraph = (tree as Root).children[0] as Paragraph;
      return paragraph.children.find((child) => child.type === "link") as Link;
    };
    const actual = findLink(satteriMdast(md));
    expect(actual.url).toBe("https://x.y)x");
    expect(md.slice(actual.position!.start.offset, actual.position!.end.offset)).toBe(
      (actual.children[0] as { value: string }).value,
    );
  });
});

describe("HTML conformance: literal-autolink trigger inside a pointed autolink (#93)", () => {
  // A `www.`/`http(s)://` literal-autolink trigger sitting inside a
  // CommonMark pointed autolink `<scheme:…>` must defer to the autolink
  // construct. Previously the literal scanner fired on the `www.` mid-URL,
  // creating a link overlapping the resolved `<…>`, which swallowed the
  // trailing `)` and a following backslash hard break.
  test("`www.` mid-URL + backslash hard break: clean autolink, `)`, and `<br>`", () => {
    assertHtmlConformance("(<https://www.example.com/page>)\\\nnext line\n");
  });

  test("control: same input without `www.` (already worked)", () => {
    assertHtmlConformance("(<https://example.com/page>)\\\nnext line\n");
  });

  test("`www.` mid-URL + soft break", () => {
    assertHtmlConformance("(<https://www.example.com/page>)\nnext line\n");
  });

  test("`www.` mid-URL + trailing-spaces hard break", () => {
    assertHtmlConformance("(<https://www.example.com/page>)  \nnext line\n");
  });

  test("bare pointed autolink with `www.` host", () => {
    assertHtmlConformance("<https://www.example.com/page>\n");
  });

  test("literal www. after the pointed autolink still autolinks", () => {
    assertHtmlConformance("<https://www.example.com/page> and www.other.com\n");
  });
});

describe("MDAST conformance: literal-autolink trigger inside a pointed autolink (#93)", () => {
  test("`www.` mid-URL + backslash hard break: single link node, no overlap", () => {
    assertMdastConformance("(<https://www.example.com/page>)\\\nnext line\n");
  });
});

describe("HTML conformance: literal-autolink trigger inside an inline HTML construct (#93)", () => {
  // The same precedence as the pointed-autolink case: any `<…>` the second
  // pass resolves owns a `www.` trigger inside it. An inline HTML tag/comment
  // keeps the trigger raw, so no spurious overlapping link is created and the
  // trailing backslash hard break survives.
  test("`www.` in a tag attribute value + backslash hard break", () => {
    assertHtmlConformance("text <img alt=www.foo.com>\\\nnext line\n");
  });

  test("`www.` in an open tag with attributes + backslash hard break", () => {
    assertHtmlConformance("see <a href=www.example.com>x</a>\\\nnext line\n");
  });

  test("`www.` inside a tag-like span treated as raw HTML", () => {
    assertHtmlConformance("<not an autolink www.x.com>\\\nnext line\n");
  });

  test("control: `www.` after a closed tag still autolinks (incl. trailing `\\`)", () => {
    assertHtmlConformance("a < b www.foo.com\\\nnext line\n");
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

// GFM autolink-literal conformance with remark-gfm. Each group pins a class
// of behavior that satteri's hand-rolled scanner must match micromark's
// tokenizer + mdast-util find-and-replace on.
describe("HTML conformance: GFM autolink literals vs remark-gfm", () => {
  test("`www.` needs no second dot (micromark GH#279)", () => {
    assertHtmlConformance("www.localhost\n");
    assertHtmlConformance("http://localhost\n");
    assertHtmlConformance("www.localhost, then more\n");
  });

  test("scheme match is case-insensitive, original case preserved", () => {
    assertHtmlConformance("HTTP://example.com\n");
    assertHtmlConformance("HtTpS://Example.COM/Path\n");
    assertHtmlConformance("WWW.Example.com\n");
    // uppercase scheme that only the find-and-replace path accepts
    assertHtmlConformance("HTTP://foo_bar.com.\n");
  });

  test("`&...;` entity is trimmed as a whole, not just the `;`", () => {
    assertHtmlConformance("www.example.com&amp;\n");
    assertHtmlConformance("www.example.com&amp;)\n");
    assertHtmlConformance("see https://example.com&copy; ok\n");
    assertHtmlConformance("www.example.com&notreal\n");
  });

  test("`previousWww` set: only specific chars start a www construct", () => {
    // digit before `www.`: neither construct nor find-and-replace fires
    assertHtmlConformance("5www.example.com/p\n");
    // `.` before `www.`: find-and-replace path, splitUrl trims trailing `>`
    assertHtmlConformance(".www.example.com/p>\n");
    assertHtmlConformance("(www.example.com)\n");
  });

  test("trailing-punctuation forward scan, incl. balanced-paren trail", () => {
    assertHtmlConformance("www.example.com/a(b)\n");
    assertHtmlConformance("www.example.com/a(b.)\n");
    assertHtmlConformance("www.example.com/a(b&amp;)\n");
    assertHtmlConformance("https://example.com/foo).\n");
    assertHtmlConformance("www.example.com/p>\n");
  });

  test("autolink trigger at inline content start (after a `>` marker)", () => {
    assertHtmlConformance(">www.example.com/p*_~\n");
    assertHtmlConformance(">https://example.com).\n");
    assertHtmlConformance("> www.example.com/p*_~\n");
  });

  test("email domain: literal leading dot, double-dot stop, trailing dot", () => {
    assertHtmlConformance("contact@example.com.\n");
    assertHtmlConformance("a@b.com...x\n");
    assertHtmlConformance("8z y@.bar.baz\n");
    assertHtmlConformance("foo@sub.example.co, x\n");
  });
});

// Regression cases discovered by the autolink fuzz harness
// (test/conformance/fuzz/autolink.test.ts). Each pins a specific bug class.
describe("HTML conformance: GFM autolink fuzz regressions", () => {
  test("email domain `.` before `-`/`_` is kept by the FNR pipeline", () => {
    // construct's `emailDomainDotTrail` stops at `.`+`_`, but FNR's
    // `(?:\.[-\w]+)+` keeps it, so the reference links via FNR.
    assertHtmlConformance("@0@1_-._9a}.\n");
    assertHtmlConformance("a@b._c\n");
  });

  test("no FNR autolink inside a link label's nested emphasis", () => {
    // `findAndReplace` ignores the whole `link` subtree, not just direct text.
    assertHtmlConformance("[~www.foo.bar~](/x)\n");
    assertHtmlConformance("[*www.x.com*](/x)\n");
  });

  test("code span with an unclosed `[` still suppresses the autolink", () => {
    // The `[` must not shift code-span backtick pairing (code binds tighter).
    assertHtmlConformance("`*www.a.com[b`\n");
    assertHtmlConformance("`x[y www.z.com`\n");
  });

  test("www construct: bare `www` when only trail follows the dot", () => {
    assertHtmlConformance('> *www.!"~_",!\n');
    assertHtmlConformance("< WWW._*]?!\n");
    assertHtmlConformance("- *WWW..%&\n");
  });

  test("www construct: bare `www` when nothing but the dot follows", () => {
    // micromark's `wwwPrefix` succeeds for any non-EOF char after the dot, so
    // `www.` + whitespace/text links bare `www` via the construct, merging the
    // trailing `.` with the following text (mdast-visible). `www.` at true EOF
    // falls to FNR, which also links bare `www`.
    assertMdastConformance("www. x");
    assertMdastConformance("www.");
    assertMdastConformance("a www. b");
    assertMdastConformance("www.     indented code\n\n   paragraph\n\n       more code\n");
    // A non-www-prefixed dot run still falls to FNR and splits the trail.
    assertMdastConformance(".www.x. rest");
  });

  test("email local part with an emphasis `_` mid-token still links", () => {
    // Construct fires forward over the `_`; the firstpass must not let the
    // emphasis pass claim it first.
    assertHtmlConformance("(1a+-_@.a)\n");
    assertHtmlConformance("a+-_@example.com\n");
  });

  test("email local part start honours the FNR lookbehind boundary", () => {
    // `é_.a@x` links `.a@x` (start after the `_`), not `_.a@x`.
    assertHtmlConformance("contact é_.a@9bb-010.b here\n");
  });

  test("control: emphasis still pairs normally without an email/url", () => {
    assertHtmlConformance("a _b_ c *d* e\n");
    assertHtmlConformance("intra_word_underscore stays text\n");
  });
});

// A `[`, backtick, `<` or `$` earlier in the block defers the autolink, so its
// bytes also tokenize as ordinary inline content and have to be reconciled.
describe("MDAST conformance: deferred GFM autolink splice", () => {
  test("a character reference the URL ends inside leaves its raw residue", () => {
    // The `;` is trailing punctuation, so the URL stops one byte short of the
    // reference's end and the residue must stay literal, not decode.
    assertMdastConformance("[a] http://x.y&#104;");
    assertMdastConformance("[a] http://x.y&#x68;");
    assertMdastConformance("[a] www.x.y&#0;");
    assertMdastConformance("[a] http://x.y%&#104;");
    assertMdastConformance("`x` http://x.y&#104;");
    assertMdastConformance("<b> www.x.y&#104;");
  });

  test("a URL ending on a backslash does not lend it to the next node", () => {
    assertMdastConformance("[a] www.x.y\\_~");
    assertMdastConformance("`x` www.x.y\\_~");
    assertMdastConformance("[a] http://x.y\\*b*");
  });

  test("inline HTML still opens after a URL ending on a backslash", () => {
    assertMdastConformance("[a] http://x.y\\<a>x");
    assertMdastConformance("[a] http://x.y\\<!-- c -->");
    assertMdastConformance("[a] http://x.y\\<em>i</em>");
    assertMdastConformance("`x` http://x.y\\<a>x");
    // Blocked instead of fired: the escape stands and the `<` stays literal.
    assertMdastConformance("[a http://x.y\\<a>x");
  });

  test("constructs that survive a URL-ending backslash today", () => {
    assertMdastConformance("`x` www.a.b\\*em* rest");
    assertMdastConformance("[a] www.a.b\\`code`");
    assertMdastConformance("[a] www.x.y\\,");
    // Blocked: the escape stands and the candidate stays ordinary text.
    assertMdastConformance("[a www.a.b\\*em* rest");
    assertMdastConformance("[a http://x.y\\&amp;");
  });

  test("emphasis closes after a URL-ending backslash", () => {
    // text(`<`) + emphasis(link) + text(`~\t>`): the `_` the escape hid closes
    // the run opened before the URL, so the link is the emphasis's only child.
    assertMdastConformance("<_HTTPS://a.b:-&;\\_~\t>");
    assertMdastConformance("[a] *www.a.b\\* x");
    assertMdastConformance("[a] _www.a.b\\_ x");
    assertMdastConformance("[a] **www.a.b\\** x");
    assertMdastConformance("[a] ~~www.a.b\\~~ x");
    assertMdastConformance("[a] ~www.a.b\\~ x");
    assertMdastConformance("`x` *www.a.b\\* x");
    assertMdastConformance("<b> *www.a.b\\* x");
    // Blocked instead of fired: the escape stands, so the delimiter it hid is
    // literal and the run after it is one shorter.
    assertMdastConformance("[a *www.a.b\\* x");
    assertMdastConformance("[a ~~www.a.b\\~~ x");
  });

  test("a character reference after a URL-ending backslash still decodes", () => {
    // The link owns the `\`, so the reference the escape swallowed is split
    // back out and decodes: link(`www.a.b\`) + text(`&`).
    assertMdastConformance("[a] www.a.b\\&amp;");
    assertMdastConformance("[a] www.a.b\\&nbsp;");
    assertMdastConformance("[a] http://x.y\\&amp;");
    assertMdastConformance("`x` www.a.b\\&amp;");
    // Only the reference the escape swallowed is at stake; a second one
    // after it was never in doubt.
    assertMdastConformance("[a] www.a.b\\&amp;&amp;");
    // Not a reference, so the escape stands and the `&` stays literal.
    assertMdastConformance("[a] www.a.b\\&notanentity;");
    // Blocked instead of fired: the escape stands and nothing is split out.
    assertMdastConformance("[a http://x.y\\&amp;");
  });

  test("an escaped `<` outside any autolink is unaffected", () => {
    assertMdastConformance("a \\<b> c");
    assertMdastConformance("\\<em>not html\\</em>");
  });

  test("an entity-triggered link and an entity-ended one in the same block", () => {
    // The first link exists only because the trigger is decoded; the second
    // fires on the construct path and stops inside a reference.
    assertMdastConformance("[&#104;ttp://a.b and http://c.d&#104;");
    assertMdastConformance("`x` &#104;ttp://a.b and http://c.d&#104;");
  });
});

describe("MDAST conformance: a closed `[…]` unblocks a later autolink trigger", () => {
  test("wherever the paragraph lives", () => {
    for (const md of [
      "> [www.a.com]www.b.com",
      "- [www.a.com]www.b.com",
      "# [www.a.com]www.b.com",
      "| a |\n| - |\n| [www.a.com]www.b.com |",
      "a[^1]\n\n[^1]: [www.a.com]www.b.com",
    ]) {
      assertMdastConformance(md);
    }
  });

  test("the trailing-punctuation trim applies to the second URL", () => {
    assertMdastConformance("[www.a.com]www.b.com/p.");
    assertMdastConformance("[www.a.com].u@b.com");
  });
});

describe("MDAST conformance: unicode whitespace ends a GFM autolink literal", () => {
  const SPACES = [
    0x00a0,
    0x1680,
    ...Array.from({ length: 11 }, (_, i) => 0x2000 + i),
    0x2028,
    0x2029,
    0x202f,
    0x205f,
    0x3000,
  ].map((cp) => String.fromCodePoint(cp));

  test("it ends the URL wherever it lands", () => {
    for (const ws of SPACES) {
      assertMdastConformance(`www.exa${ws}mple.com\n`);
      assertMdastConformance(`www.example.com${ws}b\n`);
      assertMdastConformance(`www.example.com/a${ws}b\n`);
      assertMdastConformance(`http://example.com/a${ws}b\n`);
      assertMdastConformance(`www.example.com${ws}\n`);
    }
  });

  // `Cf` but not whitespace, so the URL runs straight through them.
  test("a format character stays inside the URL", () => {
    for (const cf of [0x180e, 0x200b, 0x2060].map((cp) => String.fromCodePoint(cp))) {
      assertMdastConformance(`www.exa${cf}mple.com\n`);
      assertMdastConformance(`www.example.com/a${cf}b\n`);
    }
  });

  test("a U+FEFF ending the URL stays in the text after it", () => {
    assertMdastConformance("www.example.com/a\u{feff}b\n");
    assertMdastConformance("https://example.com\u{feff}\n");
    assertMdastConformance("user@example.com\u{feff}x\n");
  });
});
