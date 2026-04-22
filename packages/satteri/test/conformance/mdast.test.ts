import { describe, test } from "vitest";
import { assertMdastConformance } from "./helpers.js";

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

  test.skip("html block indentation in list", () => {
    // TODO: HTML block inside list item includes trailing newline in value
    assertMdastConformance("-    <div>\n   <div>");
  });

  test("html comment with indentation", () => {
    assertMdastConformance("    <!-- foo -->\n\n    <!-- foo -->");
  });

  test("single tilde strikethrough intraword", () => {
    assertMdastConformance("This~is~nothing");
  });

  test.skip("task list followed by blank then content", () => {
    // TODO: text value after task marker includes leading newline
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

  test.skip("reference link", () => {
    // Satteri resolves references eagerly (produces `link` node),
    // remark keeps them as `linkReference` + `definition`. To be changed.
    assertMdastConformance("[text][ref]\n\n[ref]: https://example.com");
  });
});
