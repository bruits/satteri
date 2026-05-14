import { describe, test } from "vitest";
import {
  assertMdastConformance,
  assertHastConformance,
  assertHtmlConformance,
  assertExtMdastConformance,
  assertExtHastConformance,
} from "./helpers.js";

// Each case below was discovered by fuzz runs in test/conformance/fuzz/ and
// reduced to a minimal repro.

const MATH: ["math"] = ["math"];

describe("fuzz regressions: GFM tables", () => {
  test("minimal `header\\n:-` table is recognized", () => {
    assertHastConformance("r5\n:-");
  });

  test("delimiter cell with internal whitespace is rejected (`- -`)", () => {
    assertMdastConformance("h\n| - - |");
  });

  test("delimiter cell with two trailing colons is rejected (`-::`)", () => {
    assertMdastConformance("h\n-::-");
  });
});

describe("fuzz regressions: link definitions", () => {
  test("definition label preserves trailing whitespace", () => {
    assertMdastConformance("[m(  ]:8");
  });

  test("duplicate refdef labels each get their own definition node", () => {
    assertMdastConformance('[x]: https://a.com\n\n[x]: https://b.com "t"');
  });
});

describe("fuzz regressions: math at EOF", () => {
  test("math fence at EOF with empty body keeps trailing newline in position", () => {
    assertExtMdastConformance("$$\n", MATH);
  });

  test("math fence at EOF with trailing whitespace-only line keeps it as content", () => {
    assertExtMdastConformance("$$\n ", MATH);
  });
});

describe("fuzz regressions: backslash escapes", () => {
  test("inline math after `\\\\` is still parsed", () => {
    assertExtHastConformance("\\+$+$j", MATH);
  });
});

describe("fuzz regressions: paragraph continuation", () => {
  test("`::` on continuation line stays in the paragraph", () => {
    assertMdastConformance("s\n::cw !u");
  });
});

describe("fuzz regressions: code blocks", () => {
  test("trailing indented blank line is part of the code block", () => {
    assertHtmlConformance("\t* :u4i\n\t\t");
  });
});

describe("fuzz regressions: math meta", () => {
  test("math meta preserves trailing space", () => {
    assertExtMdastConformance("$$|/0= ", MATH);
  });

  test("math meta preserves trailing tab", () => {
    assertExtMdastConformance("$$!\t\nvs*", MATH);
  });
});

describe("fuzz regressions: post-break whitespace", () => {
  test("inline math after hard break has leading whitespace trimmed", () => {
    assertExtHastConformance("a\\\n$\t$", MATH);
  });

  test("inline code after hard break has leading whitespace trimmed", () => {
    assertHastConformance("a\\\n` x`");
  });
});

describe("fuzz regressions: blockquote continuation", () => {
  test("tab followed by `>` is lazy continuation, not a marker", () => {
    assertMdastConformance(">:\n\t>");
  });

  test("space+tab followed by `>` is lazy continuation", () => {
    assertMdastConformance(">a\n \t>b");
  });
});

describe("fuzz regressions: indented code blocks", () => {
  test("trailing indented blank line preserves a separating newline", () => {
    assertMdastConformance("\tfoo\n\n\t");
  });

  test("multiple blank lines before trailing indented blank are preserved", () => {
    assertMdastConformance("\tfoo\n\n\n\t");
  });
});

describe("fuzz regressions: GFM table delimiter precedence", () => {
  test("delimiter line that's also a list marker (`{!\\n -\\t|`) is a list", () => {
    assertMdastConformance("{!\n -\t|");
  });

  test("delimiter line with leading space + space content prefers list", () => {
    assertMdastConformance("h\n - |");
  });
});

describe("fuzz regressions: inline HTML wrapping", () => {
  test("continuation line drops leading whitespace from inline HTML", () => {
    assertMdastConformance("<a\n jr_r>");
  });

  test("tab on continuation line is replaced by overflow spaces", () => {
    assertMdastConformance("<a\n\tattr>");
  });
});

describe("fuzz regressions: footnote vs definition", () => {
  test("`[^a b]:` falls back to a regular definition (label has whitespace)", () => {
    assertMdastConformance("[^a b]:!");
  });

  test("`[^]:` falls back to a regular definition (empty label)", () => {
    assertMdastConformance("[^]:x");
  });

  test("`[^a\\tb]:` falls back to a regular definition (tab in label)", () => {
    assertMdastConformance("[^a\tb]:x");
  });
});

describe("fuzz regressions: refdef nesting", () => {
  test("definition inside a list item stays inside the item", () => {
    assertMdastConformance("- [a]:b");
  });

  test("definition inside a list item with following paragraph", () => {
    assertMdastConformance("- [a]:b\n  text");
  });

  test("definition inside a blockquote stays inside", () => {
    assertMdastConformance("> [a]:b");
  });
});

describe("fuzz regressions: light table interrupts paragraphs", () => {
  test("light delim row interrupts a multi-line paragraph", () => {
    assertMdastConformance("foo\nbar\n:--");
  });

  test("light delim row with `+` header (no pipes)", () => {
    assertMdastConformance("7\n+\n:--");
  });

  test("inline content (`*em*`) before light table is preserved", () => {
    assertMdastConformance("*em*\nh\n:--");
  });

  test("light table inside blockquote with full continuation markers", () => {
    assertMdastConformance("> foo\n> bar\n> :--");
  });

  test("light table is suppressed on lazy-continuation line", () => {
    assertMdastConformance("> blockquote\nx\n:--");
  });
});

describe("fuzz regressions: tilde delimiter flanking", () => {
  test("single-tilde opener can't pair across an escaped `~`", () => {
    assertMdastConformance("~#zs(\\~~qc");
  });

  test("single-tilde delimiter still works on bare text", () => {
    assertMdastConformance("~a~");
  });

  test("single-tilde opener can't close on a double-tilde run", () => {
    assertMdastConformance("~a~~");
  });
});

describe("fuzz regressions: link definition position", () => {
  test("trailing space after URL is part of the definition span", () => {
    assertMdastConformance("[yu]:k ");
  });

  test("trailing tab after URL is part of the definition span", () => {
    assertMdastConformance("[yu]:k\t");
  });

  test("trailing whitespace then EOL stays in the span", () => {
    assertMdastConformance("[yu]:k \n");
  });
});

describe("fuzz regressions: fenced code block position at EOF", () => {
  test("trailing newline at EOF is preserved in the position span", () => {
    assertMdastConformance("~~~|>(*]\n");
  });

  test("multiple trailing newlines at EOF are all preserved", () => {
    assertMdastConformance("~~~\nfoo\n\n");
  });

  test("empty fenced block with just info+newline keeps the newline", () => {
    assertMdastConformance("```js\n");
  });
});

describe("fuzz regressions: definition/reference label backslash unescape", () => {
  test("definition label resolves `\\\\` escape to `\\`", () => {
    assertMdastConformance("[a\\\\b]:url");
  });

  test("definition label leaves `\\n` alone (n is not punctuation)", () => {
    assertMdastConformance("[a\\nb]:url");
  });

  test("link reference (full) label resolves backslash escapes", () => {
    assertMdastConformance("[t][a\\\\b]\n\n[a\\\\b]:u");
  });

  test("link reference (collapsed) label resolves backslash escapes", () => {
    assertMdastConformance("[a\\\\b][]\n\n[a\\\\b]:u");
  });

  test("link reference (shortcut) label resolves backslash escapes", () => {
    assertMdastConformance("[a\\\\b]\n\n[a\\\\b]:u");
  });

  test("image reference label resolves backslash escapes", () => {
    assertMdastConformance("![a\\\\b]\n\n[a\\\\b]:u");
  });
});
