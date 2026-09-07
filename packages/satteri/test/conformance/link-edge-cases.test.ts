import { describe, test, expect } from "vitest";
import {
  assertHtmlConformance,
  assertCommonMarkMdastConformance,
  assertExtMdastConformance,
  assertMdastConformance,
  satteriMdast,
} from "./helpers.js";
import type { Link, Paragraph, Root } from "mdast";

describe("HTML conformance: malformed reference definitions fall back to paragraphs", () => {
  test("blank line inside refdef label — bare URL autolinks in trailing paragraph", () => {
    assertHtmlConformance("[x\\\n\n]: https://rust-lang.org\n");
  });

  test("setext H2 underline breaks refdef label", () => {
    assertHtmlConformance("[First try\n----------\nSecond try]: https://rust-lang.org\n");
  });

  test("setext H2 underline breaks refdef label then reference below", () => {
    assertHtmlConformance("[first\n-\nsecond]: https://example.com\n\n[first\n-\nsecond]\n");
  });
});

describe("MDAST conformance: autolink-literal vs directive inside broken link labels", () => {
  test("unmatched `[` + `:port` after URL host: directive wins, URL stays text", () => {
    assertExtMdastConformance('[``x``:"http://localhost:4321"`](https://a.com/) z `foo`.', [
      "directive",
    ]);
  });

  test("unmatched `[` in directive: no autolink, no port merge", () => {
    assertExtMdastConformance('[`a`:"http://localhost:4321" end', ["directive"]);
  });
});

describe("MDAST conformance: GFM autolink-literal trim-back split", () => {
  test("unclosed `[` + URL: remark splits `),` from the trailing text", () => {
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
    assertHtmlConformance("<http://\x07>\n");
  });
});

describe("HTML conformance: deflist-shaped input without the deflist extension", () => {
  test("`* item\\n\\n  : body` renders as a loose list with a `:`-prefixed paragraph", () => {
    assertHtmlConformance("* def this\n\n  : def text def text\n");
  });
});

describe("HTML conformance: malformed reference definition labels with footnote", () => {
  test("`[label[^fn]]: URL` — invalid refdef label, URL autolinks in trailing paragraph", () => {
    assertHtmlConformance(
      "My [otherlink[^c]].\n\n[^c]: foo.\n\n[otherlink[^c]]: https://example.com/path\n",
    );
  });
});

describe("HTML conformance: malformed inline links fall back to paragraphs", () => {
  test("nested sublist marker breaks `[text](url)` across lines", () => {
    assertHtmlConformance("- [foo\n  - -\n  baz](https://example.com)\n");
  });

  test("parens nesting beyond pulldown-cmark's balance limit rejects the link", () => {
    assertHtmlConformance(
      "[30](https://rust.org/something%3A((((((((((((((((((((((((((((((())))))))))))))))))))))))))))))))\n[40](https://rust.org/something%3A((((((((((((((((((((((((((((((((((((((((())))))))))))))))))))))))))))))))))))))))))\n",
    );
  });

  test("fenced code block inside a list item splits a `[text](url)` link", () => {
    assertHtmlConformance(
      "- Item definition [it\n  ```rust\n  ```\n  stuff](https://example.com)\n",
    );
  });
});

describe("MDAST conformance: unescaped `(` inside a parenthesized title (#211)", () => {
  test("the reported input is a link with url `*` and title `(`", () => {
    const md = "[a](* (())";
    assertExtMdastConformance(md, []);
    const paragraph = (satteriMdast(md) as Root).children[0] as Paragraph;
    const link = paragraph.children[0] as Link;
    expect(link.url).toBe("*");
    expect(link.title).toBe("(");
  });

  test("links and images keep the `(` in the title", () => {
    assertMdastConformance("[a](/url (ti(tle))");
    assertMdastConformance("![a](/url (ti(tle))");
    assertMdastConformance("[a](/url (tit\\(le))");
  });

  test("the title still ends at the first unescaped `)`", () => {
    assertMdastConformance("[a](/url ((()))");
    assertMdastConformance("[a](/url (a(b)c))");
  });

  test("a paragraph full of unclosed `(` titles parses in linear time", () => {
    const md = "[a](x (".repeat(20000);
    const paragraph = (satteriMdast(md) as Root).children[0] as Paragraph;
    expect(paragraph.children).toEqual([{ type: "text", value: md, position: expect.anything() }]);
  });

  test("reference definitions take the same titles", () => {
    assertMdastConformance("[a]: /url (ti(tle)\n\n[a]\n");
    assertHtmlConformance("[link]: test (()\n\n[link]\n");
    assertHtmlConformance("[link]: test (())\n\n[link]\n");
  });
});

describe("MDAST conformance: a link title needs whitespace after the destination", () => {
  test("a title jammed against a pointy destination is not a link", () => {
    assertMdastConformance('[a](<x>"t")');
    assertMdastConformance("[a](<x>(t))");
    assertMdastConformance("[a](<>'t')");
    assertMdastConformance('![a](<x>"t")');
  });

  test("spaces or a line ending still separate a title", () => {
    assertMdastConformance('[a](<x> "t")');
    assertMdastConformance('[a](<x>\n"t")');
    assertMdastConformance("[a](<x>)");
    assertMdastConformance('[a](/u"t")');
  });

  test("reference definitions keep the same rule", () => {
    assertMdastConformance('[a]: <x>"t"\n\n[a]\n');
    assertMdastConformance('[a]: <x> "t"\n\n[a]\n');
  });
});

describe("HTML conformance: YAML metadata block edge cases", () => {
  test("YAML frontmatter with leading blank line consumes the whole block", () => {
    assertHtmlConformance("---\n\ntitle: example\nanother_field: 0\n---\n");
  });

  test("`---` after a paragraph isn't a frontmatter start", () => {
    assertHtmlConformance("My paragraph here.\n\n---\ntitle: example\nanother_field: 0\n---\n");
  });
});

describe("MDAST conformance: edge-case reference parsing", () => {
  test("blank line inside refdef label — no definition node emitted", () => {
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

  // Nested-list positions differ from remark, so this input is compared only as HTML.

  test("YAML frontmatter with leading blank line — one yaml node at root", () => {
    assertExtMdastConformance("---\n\ntitle: example\nanother_field: 0\n---\n", ["frontmatter"]);
  });
});

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
    assertHtmlConformance("HTTP://foo_bar.com.\n");
  });

  test("`&...;` entity is trimmed as a whole, not just the `;`", () => {
    assertHtmlConformance("www.example.com&amp;\n");
    assertHtmlConformance("www.example.com&amp;)\n");
    assertHtmlConformance("see https://example.com&copy; ok\n");
    assertHtmlConformance("www.example.com&notreal\n");
  });

  test("`previousWww` set: only specific chars start a www construct", () => {
    assertHtmlConformance("5www.example.com/p\n");
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

describe("HTML conformance: GFM autolink fuzz regressions", () => {
  test("email domain `.` before `-`/`_` is kept by the FNR pipeline", () => {
    assertHtmlConformance("@0@1_-._9a}.\n");
    assertHtmlConformance("a@b._c\n");
  });

  test("no FNR autolink inside a link label's nested emphasis", () => {
    assertHtmlConformance("[~www.foo.bar~](/x)\n");
    assertHtmlConformance("[*www.x.com*](/x)\n");
  });

  test("code span with an unclosed `[` still suppresses the autolink", () => {
    assertHtmlConformance("`*www.a.com[b`\n");
    assertHtmlConformance("`x[y www.z.com`\n");
  });

  test("www construct: bare `www` when only trail follows the dot", () => {
    assertHtmlConformance('> *www.!"~_",!\n');
    assertHtmlConformance("< WWW._*]?!\n");
    assertHtmlConformance("- *WWW..%&\n");
  });

  test("www construct: bare `www` when nothing but the dot follows", () => {
    assertMdastConformance("www. x");
    assertMdastConformance("www.");
    assertMdastConformance("a www. b");
    assertMdastConformance("www.     indented code\n\n   paragraph\n\n       more code\n");
    assertMdastConformance(".www.x. rest");
  });

  test("email local part with an emphasis `_` mid-token still links", () => {
    assertHtmlConformance("(1a+-_@.a)\n");
    assertHtmlConformance("a+-_@example.com\n");
  });

  test("email local part start honours the FNR lookbehind boundary", () => {
    assertHtmlConformance("contact é_.a@9bb-010.b here\n");
  });

  test("control: emphasis still pairs normally without an email/url", () => {
    assertHtmlConformance("a _b_ c *d* e\n");
    assertHtmlConformance("intra_word_underscore stays text\n");
  });
});

describe("MDAST conformance: deferred GFM autolink splice", () => {
  test("a character reference the URL ends inside leaves its raw residue", () => {
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
    assertMdastConformance("[a http://x.y\\<a>x");
  });

  test("constructs that survive a URL-ending backslash today", () => {
    assertMdastConformance("`x` www.a.b\\*em* rest");
    assertMdastConformance("[a] www.a.b\\`code`");
    assertMdastConformance("[a] www.x.y\\,");
    assertMdastConformance("[a www.a.b\\*em* rest");
    assertMdastConformance("[a http://x.y\\&amp;");
  });

  test("emphasis closes after a URL-ending backslash", () => {
    assertMdastConformance("<_HTTPS://a.b:-&;\\_~\t>");
    assertMdastConformance("[a] *www.a.b\\* x");
    assertMdastConformance("[a] _www.a.b\\_ x");
    assertMdastConformance("[a] **www.a.b\\** x");
    assertMdastConformance("[a] ~~www.a.b\\~~ x");
    assertMdastConformance("[a] ~www.a.b\\~ x");
    assertMdastConformance("`x` *www.a.b\\* x");
    assertMdastConformance("<b> *www.a.b\\* x");
    assertMdastConformance("[a *www.a.b\\* x");
    assertMdastConformance("[a ~~www.a.b\\~~ x");
  });

  test("a character reference after a URL-ending backslash still decodes", () => {
    assertMdastConformance("[a] www.a.b\\&amp;");
    assertMdastConformance("[a] www.a.b\\&nbsp;");
    assertMdastConformance("[a] http://x.y\\&amp;");
    assertMdastConformance("`x` www.a.b\\&amp;");
    assertMdastConformance("[a] www.a.b\\&amp;&amp;");
    assertMdastConformance("[a] www.a.b\\&notanentity;");
    assertMdastConformance("[a http://x.y\\&amp;");
  });

  test("an escaped `<` outside any autolink is unaffected", () => {
    assertMdastConformance("a \\<b> c");
    assertMdastConformance("\\<em>not html\\</em>");
  });

  test("an entity-triggered link and an entity-ended one in the same block", () => {
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

describe("MDAST conformance: a defined footnote reference beats an inline link tail", () => {
  const DEF = "[^x]: d\n\n";

  test("`[^x](y)` is a footnote reference plus literal `(y)`", () => {
    assertMdastConformance(`${DEF}[^x](y)`);
    assertMdastConformance(`${DEF}[^x](y "t")`);
    assertMdastConformance(`${DEF}[^x]()`);
  });

  test("an undefined `[^x](y)` is still a link", () => {
    assertMdastConformance("[^x](y)");
    assertMdastConformance(`${DEF}[^y](y)`);
  });

  test("the definition is matched case-insensitively", () => {
    assertMdastConformance(`${DEF}[^X](y)`);
  });

  test("a label with whitespace is not a footnote call, so the link stands", () => {
    assertMdastConformance(`${DEF}[^x ](y)`);
    assertMdastConformance(`${DEF}[^ x](y)`);
  });

  test("`![^x](y)` stays an image even when `x` is defined", () => {
    assertMdastConformance(`${DEF}![^x](y)`);
    assertMdastConformance("![^x](y)");
  });

  test("a plain link next to a footnote definition is untouched", () => {
    assertMdastConformance(`${DEF}[x](y)`);
    assertMdastConformance(`${DEF}[^x] [y](z)`);
  });

  test("with GFM off the label is a link again", () => {
    assertCommonMarkMdastConformance(`${DEF}[^x](y)`);
    assertCommonMarkMdastConformance(`${DEF}![^x](y)`);
  });

  test("the footnote wins inside emphasis too", () => {
    assertMdastConformance(`${DEF}*[^x](y)*`);
  });
});
