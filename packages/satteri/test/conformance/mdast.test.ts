import { describe, test } from "vitest";
import { assertMdastConformance, assertExtMdastConformance } from "./helpers.js";

describe("MDAST conformance: block elements", () => {
  test("heading", () => {
    assertMdastConformance("# Hello");
  });

  test("h2", () => {
    assertMdastConformance("## World");
  });

  test("paragraph", () => {
    assertMdastConformance("hello world");
  });

  test("multiple paragraphs", () => {
    assertMdastConformance("first\n\nsecond\n\nthird");
  });

  test("blockquote", () => {
    assertMdastConformance("> quoted text");
  });

  test("nested blockquote", () => {
    assertMdastConformance("> > nested");
  });

  test("horizontal rule", () => {
    assertMdastConformance("---");
  });

  test("code block", () => {
    assertMdastConformance("```\ncode\n```");
  });

  test("code block with language", () => {
    assertMdastConformance("```js\nconst x = 1\n```");
  });

  test("indented code block", () => {
    assertMdastConformance("    indented code");
  });
});

describe("MDAST conformance: inline elements", () => {
  test("bold", () => {
    assertMdastConformance("**bold**");
  });

  test("italic", () => {
    assertMdastConformance("*italic*");
  });

  test("bold and italic", () => {
    assertMdastConformance("***bold italic***");
  });

  test("inline code", () => {
    assertMdastConformance("`code`");
  });

  test("link", () => {
    assertMdastConformance("[text](https://example.com)");
  });

  test("link with title", () => {
    assertMdastConformance('[text](https://example.com "title")');
  });

  test("image", () => {
    assertMdastConformance("![alt](https://example.com/img.png)");
  });

  test("line break", () => {
    assertMdastConformance("line one  \nline two");
  });

  test("mixed inline", () => {
    assertMdastConformance("**bold** and *italic* and `code`");
  });
});

describe("MDAST conformance: lists", () => {
  test("unordered list", () => {
    assertMdastConformance("- one\n- two\n- three");
  });

  test("ordered list", () => {
    assertMdastConformance("1. one\n2. two\n3. three");
  });

  test("nested list", () => {
    assertMdastConformance("- outer\n  - inner\n- back");
  });

  test("list with paragraphs (loose)", () => {
    assertMdastConformance("- first\n\n- second");
  });

  test("task list (GFM)", () => {
    assertMdastConformance("- [x] done\n- [ ] todo");
  });

  test("loose list with nested list in first item", () => {
    assertMdastConformance("* *\n\n* text*");
  });

  test("spread only within item, not between items", () => {
    assertMdastConformance("gpr\n\n- e4smu\n- 245t2hw\n\n  m27rz3ex9");
  });

  test("empty list item position", () => {
    assertMdastConformance("1.  \n2. text");
  });

  test("empty unordered list item with continuation whitespace", () => {
    assertMdastConformance("+\n ");
  });

  test("empty list item with trailing spaces", () => {
    assertMdastConformance("*  \n ");
  });

  test("spec 259: nested blockquote ordered list with blank continuation", () => {
    assertMdastConformance("   > > 1.  one\n>>\n>>     two\n");
  });

  test("spec 325: list item with sublist and trailing content becomes loose", () => {
    assertMdastConformance("* foo\n  * bar\n\n  baz\n");
  });
});

describe("MDAST conformance: tables (GFM)", () => {
  test("simple table", () => {
    assertMdastConformance("| a | b |\n| - | - |\n| 1 | 2 |");
  });

  test("table with alignment", () => {
    assertMdastConformance("| left | center | right |\n| :--- | :---: | ---: |\n| a | b | c |");
  });
});

describe("MDAST conformance: HTML in markdown", () => {
  test("inline HTML", () => {
    assertMdastConformance("hello <em>world</em>");
  });

  test("block HTML", () => {
    assertMdastConformance("<div>block</div>");
  });
});

describe("MDAST conformance: images", () => {
  test("nested image in image alt text", () => {
    assertMdastConformance("![foo ![bar](/url)](/url2)");
  });
});

// A hard break inside an image label carries no visible content, so it adds
// nothing to the flattened alt text; a soft break contributes the source's own
// line ending.
describe("MDAST conformance: line breaks in image alt text", () => {
  test("backslash hard break", () => {
    assertMdastConformance("![a\\\nb](c.png)");
  });

  test("backslash hard break with CRLF", () => {
    assertMdastConformance("![a\\\r\nb](c.png)");
  });

  test("backslash hard break with a lone CR", () => {
    assertMdastConformance("![a\\\rb](c.png)");
  });

  test("repeated backslash hard breaks", () => {
    assertMdastConformance("![a\\\nb\\\nc](c.png)");
  });

  test("trailing-space hard break", () => {
    assertMdastConformance("![a  \nb](c.png)");
  });

  test("trailing-space hard break with CRLF", () => {
    assertMdastConformance("![a  \r\nb](c.png)");
  });

  test("trailing-space hard break with a lone CR", () => {
    assertMdastConformance("![a  \rb](c.png)");
  });

  test("soft break", () => {
    assertMdastConformance("![a\nb](c.png)");
  });

  test("soft break with CRLF", () => {
    assertMdastConformance("![a\r\nb](c.png)");
  });

  test("soft break with a lone CR", () => {
    assertMdastConformance("![a\rb](c.png)");
  });

  test("inline code in the label", () => {
    assertMdastConformance("![a`x`b](c.png)");
  });

  test("emphasis in the label", () => {
    assertMdastConformance("![a*x*b](c.png)");
  });

  test("hard break in a reference image label", () => {
    assertMdastConformance("![a\\\nb][d]\n\n[d]: /u");
  });

  test("hard break in a collapsed reference image label", () => {
    assertMdastConformance("![a\\\nb][]\n\n[a\\\nb]: /u");
  });
});

describe("MDAST conformance: edge cases", () => {
  test("empty input", () => {
    assertMdastConformance("");
  });

  test("only whitespace", () => {
    assertMdastConformance("   ");
  });

  test("escaped characters", () => {
    assertMdastConformance("\\*not bold\\*");
  });

  test("autolink", () => {
    assertMdastConformance("<https://example.com>");
  });

  test("GFM strikethrough", () => {
    assertMdastConformance("~~deleted~~");
  });

  test("GFM single-tilde strikethrough", () => {
    assertMdastConformance("~deleted~");
  });

  test("single-tilde strikethrough intraword", () => {
    assertMdastConformance("]1~lr~ -x");
  });

  test("single-tilde strikethrough with brackets", () => {
    assertMdastConformance("{sl~v[ {@~");
  });

  test("heading with inline formatting", () => {
    assertMdastConformance("## The `config` object");
  });

  test("blockquote with formatting", () => {
    assertMdastConformance("> **bold** in quote");
  });

  test("blockquote with leading space", () => {
    assertMdastConformance(" >~#7f\ndl");
  });

  test("soft break merges text nodes", () => {
    assertMdastConformance("^)\n4");
  });

  test("multiple soft breaks merge text nodes", () => {
    assertMdastConformance("c\nsq1\nz<o");
  });

  test("thematic break position excludes trailing newline", () => {
    assertMdastConformance("***\n\n# l");
  });

  test("heading positions in multi-block document", () => {
    assertMdastConformance("# hello\n\nworld");
  });

  test("task list with double space after checkbox", () => {
    assertMdastConformance("- [ ]  text");
  });

  test("list with leading space", () => {
    assertMdastConformance(" -");
  });

  test("indented ordered list", () => {
    assertMdastConformance("  0)");
  });

  test("emphasis wrapping punctuation (*_*)", () => {
    assertMdastConformance("\n+u*_*@|q)");
  });

  test("list spread with trailing content in item", () => {
    assertMdastConformance("gpr\n\n- e4smu\n- 245t2hw\n\n  m27rz3ex9");
  });

  test("tabs before newline are not hard break", () => {
    assertMdastConformance("-v\t\t\nr {l ");
  });

  test("escaped backtick position", () => {
    assertMdastConformance("\\`d");
  });

  test("escaped backtick with leading space", () => {
    assertMdastConformance(" \\`z");
  });

  test("single tilde with underscore is not strikethrough", () => {
    assertMdastConformance("2jj~_|m~<");
  });

  test("underscore emphasis does not open near attention markers", () => {
    assertMdastConformance(" ==d_*\\`_");
  });

  test("html block includes leading space", () => {
    assertMdastConformance(" <!n=n0p");
  });

  test("heading trailing tab stripped", () => {
    assertMdastConformance("# h\t");
  });

  test("heading trailing tab with closing hashes", () => {
    assertMdastConformance("# -0 #\t");
  });

  test("tilde fence with content", () => {
    assertMdastConformance("~~~)_>u");
  });

  test("indented code block with trailing blank indented line", () => {
    assertMdastConformance("\tl\n\t");
  });

  test("indented code block with trailing blank indented line (html content)", () => {
    assertMdastConformance("\t<@9\\s\n\t");
  });

  test("html block with backslash newline", () => {
    assertMdastConformance("<!o \\\n");
  });

  test("code span newline kept in mdast", () => {
    assertMdastConformance(")x_`[>^w\n`");
  });

  test("escaped backtick followed by backtick", () => {
    assertMdastConformance("\\``a");
  });

  test("escaped backtick followed by multiple backticks", () => {
    assertMdastConformance("\\``)( kpd");
  });

  test("empty sub-list cannot interrupt paragraph", () => {
    assertMdastConformance("x\n+ -");
  });

  test("empty list in blockquote after paragraph", () => {
    assertMdastConformance("x\n>*");
  });

  test("blockquote with dash after paragraph", () => {
    assertMdastConformance("x\n>-");
  });

  test("empty heading attribute block preserved", () => {
    assertMdastConformance("#  _i~+{}");
  });

  test("non-empty list in blockquote after paragraph", () => {
    assertMdastConformance("x\n>* a");
  });

  test("empty list in blockquote after blank line", () => {
    assertMdastConformance("x\n\n>*");
  });

  test("empty list with + in blockquote after paragraph", () => {
    assertMdastConformance("x\n>+");
  });

  test("heading with empty attribute block {}", () => {
    assertMdastConformance("# Hello {}");
  });

  test("heading with double empty attribute block {} {}", () => {
    assertMdastConformance("## World {} {}");
  });

  test("heading with empty id {#}", () => {
    assertMdastConformance("# H {#}");
  });

  test("html block with leading indentation", () => {
    assertMdastConformance("  <div>\n  *hello*\n         <foo><a>");
  });

  test("html block indentation in list", () => {
    assertMdastConformance("-    <div>\n   <div>");
  });

  test("html comment with indentation", () => {
    assertMdastConformance("    <!-- foo -->\n\n    <!-- foo -->");
  });

  test("single tilde strikethrough intraword", () => {
    assertMdastConformance("This~is~nothing");
  });

  test.skip("task list followed by blank then content", () => {
    // Tree structure and text value match remark; only paragraph/text
    // start.line and start.column differ due to a remark quirk
    assertMdastConformance("- [x]\t\t\n\\\n-");
  });

  test("task list marker not consumed when blank", () => {
    assertMdastConformance("* [ ] \n---");
  });

  test("task list with nested content after blank marker", () => {
    assertMdastConformance(
      "- [x] * some text\n- [ ] > some text\n- [x]\n  * some text\n- [ ]\n  > some text",
    );
  });

  test("tab before newline is not hard break", () => {
    assertMdastConformance("hello\t  \nworld");
  });

  test("blockquote with empty continuation", () => {
    assertMdastConformance(">n4\n>");
  });

  test("reference link", () => {
    assertMdastConformance("[text][ref]\n\n[ref]: https://example.com");
  });
});

describe("MDAST conformance: GFM autolink literal", () => {
  // Bare URLs in text are promoted to `link` nodes (remark-gfm behavior).
  test("https:// URL in paragraph", () => {
    assertMdastConformance("Visit https://example.com today");
  });

  test("https:// URL at end of sentence (trailing punctuation trimmed)", () => {
    assertMdastConformance("Check out https://example.com.");
  });

  test("URL alone in paragraph", () => {
    assertMdastConformance("https://example.com");
  });

  test("URL in parentheses", () => {
    assertMdastConformance("See (https://example.com) for details");
  });

  test("www. URL gets http:// prepended", () => {
    assertMdastConformance("Visit www.example.com");
  });

  test("URL with path and query", () => {
    assertMdastConformance("See https://example.com/path?q=1#frag");
  });

  test("URL after bold text", () => {
    assertMdastConformance("**bold** https://example.com");
  });

  test("bare URL is NOT matched when preceded by letter", () => {
    // GFM: URL must be preceded by whitespace, (, *, _, ~, or start of line.
    assertMdastConformance("abchttps://example.com");
  });

  test("trailing closing paren balanced", () => {
    assertMdastConformance("(see https://example.com/foo)");
  });

  test("URL with port preserved when directive is enabled", () => {
    // When both GFM autolink and remark-directive are enabled, `:4321` in
    // `http://host:4321` must stay inside the URL. In remark this happens
    // because autolink tokenization beats directive detection; we achieve
    // the same effect by merging the `text + textDirective + text` split
    // back together before the autolink scan runs.
    assertExtMdastConformance("Navigate to http://localhost:4321/ now", ["directive"]);
  });

  test("URL with port inside bracketed link keeps directive (not merged)", () => {
    // Inverse of the above: inside a `[label](...)` bracketed link, remark
    // keeps the directive split. The merge must skip link children.
    assertExtMdastConformance("See [http://localhost:4321/](http://localhost:4321/)", [
      "directive",
    ]);
  });
});

describe("MDAST conformance: entity decoding merges text", () => {
  // Regression: decoded entities used to emit a standalone Text node
  // that broke adjacent text runs into multiple siblings. remark merges
  // them into a single Text; satteri now matches via emit_text_merging.

  test("&lt; and &gt; around plain text", () => {
    assertMdastConformance("Promise&lt;string&gt;");
  });

  test("&amp; between runs", () => {
    assertMdastConformance("x&amp;y");
  });

  test("&colon; between runs", () => {
    assertMdastConformance("x&colon;y");
  });

  test("entity at start of strong", () => {
    assertMdastConformance("**&amp;foo**");
  });

  test("entity at end of strong", () => {
    assertMdastConformance("**foo&amp;**");
  });

  test("multiple entities in one text run", () => {
    assertMdastConformance("a&lt;b&gt;c&amp;d");
  });

  test("entity inside link text", () => {
    assertMdastConformance("[set&colon;html](/foo)");
  });
});

describe("MDAST conformance: softbreak preserves CRLF", () => {
  // Regression: inline SoftBreak was hard-coded to emit "\n" when merging into
  // an adjacent Text node, collapsing `\r\n` line endings to `\n`. The line
  // ending must be taken from the source span to keep CRLF source round-
  // tripping through text nodes.

  test("plain text across CRLF softbreak", () => {
    assertMdastConformance("a\r\nb");
  });

  test("CRLF softbreak after strong", () => {
    assertMdastConformance("**a** b\r\n**c** d");
  });

  test("CRLF softbreak after inline code", () => {
    assertMdastConformance("`a`\r\nb");
  });

  test("plain LF softbreak still works", () => {
    assertMdastConformance("a\nb");
  });
});

// CommonMark counts `\n`, `\r` and `\r\n` alike as line endings. The line
// table was built by scanning for `\n` only, so a lone `\r` left every later
// node on the previous line (offsets were unaffected).
describe("MDAST conformance: standalone CR positions", () => {
  test("paragraph across a lone CR", () => {
    assertMdastConformance("a\rb");
  });

  test("lone CR inside a link destination", () => {
    assertMdastConformance("[Mercury](\rmercury)");
  });

  test("mixed CR, CRLF and LF in one document", () => {
    assertMdastConformance("a\r\nb\rc\nd");
  });

  test("CR at document start", () => {
    assertMdastConformance("\ra");
  });

  test("CR at document end", () => {
    assertMdastConformance("a\r");
  });

  test("consecutive CRs separate blocks", () => {
    assertMdastConformance("a\r\rb");
  });

  test("lone CR across block structures", () => {
    assertMdastConformance("# h\rp\r\n- x\r  y");
  });

  test("lone CR in a blockquote", () => {
    assertMdastConformance("> q\rq2");
  });

  test("lone CR with multibyte characters", () => {
    assertMdastConformance("❤️a\r😀b\rc");
  });
});

// Block structure was decided by scanning for `\n` only, so a document whose
// line endings are all lone `\r` was read as a single line.
describe("MDAST conformance: standalone CR block structure", () => {
  test("blank line between list items makes the list loose", () => {
    assertMdastConformance("- a\r\r- b");
  });

  test("fenced code block", () => {
    assertMdastConformance("```js\rcode\r```\r");
  });

  test("fenced code block left open", () => {
    assertMdastConformance("```js\rcode\r");
  });

  test("indented code block strips continuation indentation", () => {
    assertMdastConformance("    a\r    b\r");
  });

  test("HTML block ends at a blank line", () => {
    assertMdastConformance("<div>\ra\r\rb\r");
  });

  test("setext heading underline", () => {
    assertMdastConformance("title\r=====\r");
  });

  test("thematic break between paragraphs", () => {
    assertMdastConformance("a\r***\rb");
  });

  test("nested list indentation", () => {
    assertMdastConformance("- a\r  - b\r    - c\r");
  });

  test("block quote with a lazy continuation line", () => {
    assertMdastConformance("> a\rb\r\rc");
  });

  test("link reference definition followed by a use", () => {
    assertMdastConformance("[foo]: /url\r\r[foo]\r");
  });

  test("setext heading directly after a definition inherits its start", () => {
    assertMdastConformance("[foo]: /url\rtitle\r=====\r");
  });

  test("setext heading after a run of definitions", () => {
    assertMdastConformance("[a]: /a\r[b]: /b\r  title\r=====\r");
  });

  test("blank line between a definition and a setext heading breaks the chain", () => {
    assertMdastConformance("[foo]: /url\r\rtitle\r=====\r");
  });

  test("hard line break before a lone CR", () => {
    assertMdastConformance("a  \rb");
  });

  test("table", () => {
    assertMdastConformance("| a | b |\r| - | - |\r| 1 | 2 |\r");
  });
});

// Values carry the document's own line endings byte for byte; only the
// matching `identifier` of a definition is normalized.
describe("MDAST conformance: line endings inside values", () => {
  const FLAVORS: [string, string][] = [
    ["LF", "\n"],
    ["CRLF", "\r\n"],
    ["CR", "\r"],
  ];

  for (const [name, eol] of FLAVORS) {
    describe(name, () => {
      const md = (tpl: string) => tpl.split("EOL").join(eol);

      test("inline code span", () => {
        assertMdastConformance(md("`fooEOLbar`EOL"));
      });

      test("inline code span stripped of one leading and trailing line ending", () => {
        assertMdastConformance(md("``EOLfooEOLbar  EOLbazEOL``EOL"));
      });

      test("inline code span holding only a line ending", () => {
        assertMdastConformance(md("`EOL`EOL"));
      });

      test("definition title", () => {
        assertMdastConformance(md("[foo]: /url 'tEOLt2'EOLEOL[foo]EOL"));
      });

      test("definition title in parentheses", () => {
        assertMdastConformance(md("[foo]: /url (tEOLt2)EOLEOL[foo]EOL"));
      });

      test("definition label", () => {
        assertMdastConformance(md("[EOLfooEOL]: /urlEOLbarEOL"));
      });

      test("definition label with indented continuation", () => {
        assertMdastConformance(md("[FooEOL  bar]: /urlEOLEOL[Baz][Foo bar]EOL"));
      });

      test("inline link title", () => {
        assertMdastConformance(md("[a](/u 'tEOLt2')EOL"));
      });

      test("code block value", () => {
        assertMdastConformance(md("    aEOL    bEOL"));
      });
    });
  }
});

describe("MDAST conformance: closing code fence whitespace", () => {
  // Regression: CommonMark/remark allow tabs as well as spaces after the
  // closing fence. Satteri previously only consumed spaces, leaving the
  // `\`\`\`\t` line as literal content of the code block.

  test("closing fence followed by a tab", () => {
    assertMdastConformance("```js\nfoo\n```\t\n");
  });

  test("closing fence followed by space then tab", () => {
    assertMdastConformance("```js\nfoo\n``` \t\n");
  });

  test("closing fence followed by spaces (baseline)", () => {
    assertMdastConformance("```js\nfoo\n```  \n");
  });
});

// Astral characters are a single code point but two UTF-16 units, and
// `position` counts UTF-16 units — so both columns and offsets advance by
// two. BMP multibyte text can't catch a regression back to code points;
// only these can.
describe("MDAST conformance: astral characters in positions", () => {
  test("astral before a link", () => {
    assertMdastConformance("😀 [a](/x)");
  });

  test("astral in a heading", () => {
    assertMdastConformance("# 😀 head");
  });

  test("astral across blocks and lines", () => {
    assertMdastConformance("😀 one\n\n😀😀 **two**\n\n- 😀 item");
  });

  test("astral mixed with BMP multibyte", () => {
    assertMdastConformance("❤️你好😀αβγ [mixed](/x)");
  });

  test("astral at end of document", () => {
    assertMdastConformance("a [l](/x) 😀");
  });
});

describe("MDAST conformance: fuzz regressions", () => {
  // GFM strikethrough requires the opening `~~` to be left-flanking per
  // CommonMark emphasis rules: a `~~` preceded by an alphanumeric and
  // followed by punctuation isn't left-flanking and shouldn't open.
  test("strikethrough flanking: alnum before, punct after rejects", () => {
    assertMdastConformance("=l0u~~!~~");
  });

  // Strikethrough/emphasis interleaving: `_/~z)*~*nf` should parse as
  // `_/~z)` text + `*~*` emphasis. Our single-pass inline resolver
  // mimics micromark's phase ordering (emphasis first, then strikethrough)
  // by refusing to match `~`/`^` across an unmatched `*`/`_` opener on
  // the stack — that way the `*…*` pair claims its span before the
  // strikethrough pairer sees the inner `~`.
  test("strikethrough/emphasis nesting crossing", () => {
    assertMdastConformance("_/~z)*~*nf");
  });

  // Underscore emphasis nesting: in `\\ \`_@_b__=` the reference parses
  // `_@_b_` as an outer emphasis containing inner `_b_`. Used to be a known
  // bug; fixed when the emphasis pairer learned to re-check rule 9 with
  // remaining run lengths (one `<strong>`/`<em>` per inner-loop pass).
  test("nested underscore emphasis around intraword", () => {
    assertMdastConformance("\\ `_@_b__=");
  });
});

// Only spaces and tabs make up block structure and line-end padding, so VT and
// FF are content wherever they appear.
describe("MDAST conformance: control and format characters at a text-node edge", () => {
  test("trailing VT ends a paragraph", () => {
    assertMdastConformance("abc\u{b}\n");
    assertMdastConformance("abc\u{b}\u{b}\n");
    assertMdastConformance("abc\u{b}\r\n");
  });

  test("trailing FF ends a paragraph", () => {
    assertMdastConformance("abc\u{c}\n");
  });

  test("trailing VT after an inline construct is its own text node", () => {
    assertMdastConformance("*a*\u{b}\n");
    assertMdastConformance("`c`\u{b}\n");
    assertMdastConformance("[a](/x)\u{b}\n");
    assertMdastConformance("<https://a.com>\u{b}\n");
  });

  test("trailing VT at end of input", () => {
    assertMdastConformance("abc\u{b}");
    assertMdastConformance("abc\u{c}");
  });

  test("VT and FF mid-line are content", () => {
    assertMdastConformance("a\u{b}b\n");
    assertMdastConformance("a\u{c}b\n");
    assertMdastConformance("*a\u{b}*\n");
  });

  test("only the spaces and tabs around a VT are stripped", () => {
    assertMdastConformance("abc \u{b} \n");
    assertMdastConformance("abc\t\u{b}\t\n");
    // Two trailing spaces after the VT still make a hard break.
    assertMdastConformance("abc\u{b}  \ndef\n");
  });

  test("trailing VT ends a soft-broken line", () => {
    assertMdastConformance("abc\u{b}\ndef\n");
    assertMdastConformance("a\\\u{b}\nb\n");
  });

  test("trailing VT inside a container", () => {
    assertMdastConformance("> q\u{b}\n");
    assertMdastConformance("- i\u{b}\n");
    assertMdastConformance("| a |\n| - |\n| x\u{b} |\n");
    assertMdastConformance("h\u{b}\n===\n");
  });

  test("VT in a code block is content", () => {
    assertMdastConformance("```\nx\u{b}\n```\n");
    assertMdastConformance("    code\u{b}\n");
  });

  test.fails("a BOM opening a text node is kept", () => {
    assertMdastConformance("*a*\u{feff}x\n");
    assertMdastConformance("user@example.com\u{feff}\n");
  });

  test("a line of only VT or FF is a paragraph, not a blank line", () => {
    assertMdastConformance("\u{b}\n");
    assertMdastConformance("\u{c}\n");
    assertMdastConformance("  \u{b}  \n");
    assertMdastConformance("a\n\u{b}\nb\n");
    assertMdastConformance("- a\n\u{b}\n- b\n");
    assertMdastConformance("```\na\n\u{b}\nb\n```\n");
  });

  test("a VT after a definition destination makes it a paragraph", () => {
    assertMdastConformance("[a]: /x\u{b}\n\n[a]\n");
    assertMdastConformance('[a]: /x "t"\u{b}\n\n[a]\n');
  });

  test("a VT does not stand in for the space a block marker needs", () => {
    assertMdastConformance("-\u{b}a\n");
    assertMdastConformance(">\u{b}a\n");
    assertMdastConformance("- [\u{b}] a\n");
    assertMdastConformance("```js\u{b}x\n```\n");
  });

  test("a VT does not open an ATX heading", () => {
    assertMdastConformance("#\u{b}h\n");
    assertMdastConformance("#\u{b}\n");
    assertMdastConformance("###\u{c}h\n");
  });

  test("a VT does not close a task list marker", () => {
    assertMdastConformance("- [ ]\u{b} a\n");
    assertMdastConformance("- [x]\u{c} a\n");
  });

  test("a VT in a label is part of the identifier", () => {
    assertMdastConformance("[\u{b}a]: /x\n\n[a]\n");
    assertMdastConformance("[a\u{b}]: /x\n\n[a]\n");
    assertMdastConformance("a[^1\u{b}]\n\n[^1]: n\n");
    assertMdastConformance("a[^1]\n\n[^1\u{b}]: n\n");
  });

  test("a VT does not separate HTML attributes", () => {
    assertMdastConformance('<a href\u{b}="/x">l</a>\n');
    assertMdastConformance('<a href=\u{b}"/x">l</a>\n');
  });
});

describe("MDAST conformance: table cell with an escaped pipe", () => {
  test("leading `\\|` in a cell", () => {
    assertMdastConformance("| a |\n| - |\n| \\|abc |\n");
    assertMdastConformance("| a |\n| - |\n| \\| |\n");
    assertMdastConformance("| a |\n| - |\n|\\|abc|\n");
    assertMdastConformance("| \\|h |\n| - |\n| a |\n");
    assertMdastConformance("| a | b |\n| - | - |\n| x | \\|y |\n");
  });

  test("consecutive and repeated escaped pipes in a cell", () => {
    assertMdastConformance("| a |\n| - |\n| \\|\\|a |\n");
    assertMdastConformance("| a |\n| - |\n| \\|abc\\|d |\n");
    assertMdastConformance("| a |\n| - |\n| \\|\\*x |\n");
  });

  test("a leading `\\|` followed by an inline construct", () => {
    assertMdastConformance("| a |\n| - |\n| \\|*e* |\n");
    assertMdastConformance("| a |\n| - |\n| \\|`c` |\n");
  });

  test("a text node that starts on `\\|` after an inline construct", () => {
    assertMdastConformance("| a |\n| - |\n| *x*\\|z |\n");
    assertMdastConformance("| a |\n| - |\n| `c`\\|z |\n");
    assertMdastConformance("| a |\n| - |\n| user@example.com\\|z |\n");
    assertMdastConformance("| a |\n| - |\n| **b**\\|\\|z |\n");
    assertMdastConformance("| a |\n| - |\n| [t](u)\\|z |\n");
  });

  test("an escape that is neither leading nor a pipe is unaffected", () => {
    assertMdastConformance("| a |\n| - |\n| abc\\|z |\n");
    assertMdastConformance("| a |\n| - |\n| \\*abc |\n");
  });
});
