import { describe, test } from "vitest";
import { assertHastConformance } from "./helpers.js";

describe("HAST conformance: block elements", () => {
  test("heading", () => {
    assertHastConformance("# Hello");
  });

  test("h2", () => {
    assertHastConformance("## World");
  });

  test("paragraph", () => {
    assertHastConformance("hello world");
  });

  test("multiple paragraphs", () => {
    assertHastConformance("first\n\nsecond\n\nthird");
  });

  test("blockquote", () => {
    assertHastConformance("> quoted text");
  });

  test("nested blockquote", () => {
    assertHastConformance("> > nested");
  });

  test("horizontal rule", () => {
    assertHastConformance("---");
  });

  test("code block", () => {
    assertHastConformance("```\ncode\n```");
  });

  test("code block with language", () => {
    assertHastConformance("```js\nconst x = 1\n```");
  });

  test("indented code block", () => {
    assertHastConformance("    indented code");
  });
});

describe("HAST conformance: inline elements", () => {
  test("bold", () => {
    assertHastConformance("**bold**");
  });

  test("italic", () => {
    assertHastConformance("*italic*");
  });

  test("bold and italic", () => {
    assertHastConformance("***bold italic***");
  });

  test("inline code", () => {
    assertHastConformance("`code`");
  });

  test("link", () => {
    assertHastConformance("[text](https://example.com)");
  });

  test("link with title", () => {
    assertHastConformance('[text](https://example.com "title")');
  });

  test("image", () => {
    assertHastConformance("![alt](https://example.com/img.png)");
  });

  test("line break", () => {
    assertHastConformance("line one  \nline two");
  });

  test("mixed inline", () => {
    assertHastConformance("**bold** and *italic* and `code`");
  });
});

describe("HAST conformance: lists", () => {
  test("unordered list", () => {
    assertHastConformance("- one\n- two\n- three");
  });

  test("ordered list", () => {
    assertHastConformance("1. one\n2. two\n3. three");
  });

  test("nested list", () => {
    assertHastConformance("- outer\n  - inner\n- back");
  });

  test("list with paragraphs", () => {
    assertHastConformance("- first\n\n- second");
  });

  test("task list (GFM)", () => {
    assertHastConformance("- [x] done\n- [ ] todo");
  });

  test("list item with blockquote and code (no extra newlines)", () => {
    assertHastConformance("- a\n  > b\n  ```\n  c\n  ```\n- d\n");
  });

  test("loose list with nested list in first item", () => {
    assertHastConformance("* *\n\n* text*");
  });
});

describe("HAST conformance: tables (GFM)", () => {
  test("simple table", () => {
    assertHastConformance("| a | b |\n| - | - |\n| 1 | 2 |");
  });

  test("table with alignment", () => {
    assertHastConformance("| left | center | right |\n| :--- | :---: | ---: |\n| a | b | c |");
  });
});

describe("HAST conformance: HTML in markdown", () => {
  test("inline HTML", () => {
    assertHastConformance("hello <em>world</em>");
  });

  test("block HTML", () => {
    assertHastConformance("<div>block</div>");
  });
});

describe("HAST conformance: edge cases", () => {
  test("empty input", () => {
    assertHastConformance("");
  });

  test("only whitespace", () => {
    assertHastConformance("   ");
  });

  test("escaped characters", () => {
    assertHastConformance("\\*not bold\\*");
  });

  test("autolink", () => {
    assertHastConformance("<https://example.com>");
  });

  test("GFM strikethrough", () => {
    assertHastConformance("~~deleted~~");
  });

  test("single-tilde strikethrough intraword", () => {
    assertHastConformance("]1~lr~ -x");
  });

  test("heading with inline formatting", () => {
    assertHastConformance("## The `config` object");
  });

  test("blockquote with formatting", () => {
    assertHastConformance("> **bold** in quote");
  });

  test("reference link", () => {
    assertHastConformance("[text][ref]\n\n[ref]: https://example.com");
  });

  test("soft break merges text nodes", () => {
    assertHastConformance("]\n< ");
  });

  test("thematic break position excludes trailing newline", () => {
    assertHastConformance("---\n\nparagraph");
  });

  test("task list with double space after checkbox", () => {
    assertHastConformance("- [x]  text");
  });

  test("code block with blank line before closing fence", () => {
    assertHastConformance("```\ng\n\n```");
  });

  test("code block with blank line and language", () => {
    assertHastConformance("```rust\n\nmrdtvt\n\n```");
  });

  test("empty blockquote", () => {
    assertHastConformance(">");
  });

  test("emphasis wrapping punctuation (*_*)", () => {
    assertHastConformance("{0v*_*2");
  });

  test("list with leading space and content", () => {
    assertHastConformance(" * 3g");
  });

  test("ordered list start as number", () => {
    assertHastConformance("0)");
  });

  test("tabs before newline are not hard break", () => {
    assertHastConformance("-v\t\t\nr {l ");
  });

  test("escaped backtick position", () => {
    assertHastConformance("\\`d");
  });

  test("escaped backtick with leading space", () => {
    assertHastConformance(" \\`z");
  });

  test("autolink email adds mailto", () => {
    assertHastConformance("<x_@6>{|1");
  });

  test("autolink email in code context", () => {
    assertHastConformance("`g<xj@ht>");
  });

  test("code span newline to space in HAST", () => {
    assertHastConformance("7m`xy2co\n`");
  });

  test("code span newline stripped in HAST", () => {
    assertHastConformance("`\n]~ w`+|)");
  });

  test("spread task list items HAST structure", () => {
    assertHastConformance("- [ ] p\n\n- 8rj2\n- 3uabr2\n- xmr");
  });

  test("spread task list double spread", () => {
    assertHastConformance("- [x] k\n\n- [x] b16ibm247hrh\n- [ ]  a2cmlb\n\n*88i22p0bt8wy*");
  });

  test("task list mixed items 1", () => {
    assertHastConformance("- [x] 0 ud\n- [x] 81h\n\n- b0fxcmh1q\n\n# svk");
  });

  test("task list mixed items 2", () => {
    assertHastConformance("- [ ] 7j3xbf\n- [ ] o4m\n\n- [x] 97p2 zwfnr\n- [x] 61fg");
  });

  test("empty checkbox not task item", () => {
    assertHastConformance("- [x] o\n- [x] hl9i\n- [x]  ");
  });

  test("spread empty checkbox not task item", () => {
    assertHastConformance("###### i69j\n\nw9\n\n- [x] ft\n- [x]  \n- [ ] 9h\n\n**yljwm9**");
  });

  test("ordered then empty task", () => {
    assertHastConformance(
      "1. 8tj\n2. 721ruj3\n\n- [ ]  \n- [ ] f mr0\n- [ ] unyrdla7n\n\n- [x] dqof",
    );
  });

  test("empty task spread paragraph", () => {
    assertHastConformance(
      "- [ ]  \n\n*9 xjct6yd1*\n\n*k1x4l0*\n\nym\n\n> 440xlhbng\n\n##### 84722",
    );
  });

  test("ordered then mixed task empty", () => {
    assertHastConformance(
      "1. 6ewzgkavoqr\n2. tz2ds7kofn\n3. ebhcu3hxls\n\n[ xqi08kw20](https://example.com/qnvjfq)\n\n- nhkl0p54th7h\n\n- [ ]  \n\n**3clittp**",
    );
  });

  test("spread task list empty item no extra newline", () => {
    assertHastConformance("- [x] a\n- [x]  \n\n- b");
  });

  test("item-level spread propagates to all siblings", () => {
    assertHastConformance("- a\n- b\n\n  c");
  });

  test("empty list in blockquote after paragraph", () => {
    assertHastConformance("x\n>*");
  });

  test("empty list with space in blockquote after paragraph", () => {
    assertHastConformance("x\n> *");
  });

  test("unclosed code fence has empty content", () => {
    assertHastConformance("```~q");
  });

  test("autolink email encodes special chars in href", () => {
    assertHastConformance("<{{@8-w>");
  });

  test("html block trailing newline preserved in hast", () => {
    assertHastConformance("<!c\n");
  });

  test("blockquote end position includes empty continuation", () => {
    assertHastConformance(">n4\n>");
  });

  test("tab before spaces is not hard break", () => {
    assertHastConformance("uau>(\t  \nr");
  });
});
