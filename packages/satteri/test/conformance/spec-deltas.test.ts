import { describe, test } from "vitest";
import {
  assertExtHastConformance,
  assertHastConformance,
  assertHtmlConformance,
  assertMdastConformance,
} from "./helpers.js";

describe("CommonMark spec deltas: HTML blocks with following content", () => {
  test("spec 148: HTML block in table cell with following paragraph", () => {
    assertHastConformance(
      "<table><tr><td>\n<pre>\n**Hello**,\n\n_world_.\n</pre>\n</td></tr></table>\n",
    );
  });

  test("spec 155: HTML block div with following paragraph", () => {
    assertHastConformance("<div>\n*foo*\n\n*bar*\n");
  });

  test("spec 174: HTML block in blockquote with following paragraph", () => {
    assertHastConformance("> <div>\n> foo\n\nbar\n");
  });

  test("spec 177: HTML comment with following content", () => {
    assertHastConformance("<!-- foo -->*bar*\n*baz*\n");
  });

  test("spec 191: table with indented content", () => {
    assertHastConformance(
      "<table>\n\n  <tr>\n\n    <td>\n      Hi\n    </td>\n\n  </tr>\n\n</table>\n",
    );
  });
});

describe("CommonMark spec deltas: list paragraph wrapping", () => {
  test("spec 300: heading and setext heading in list", () => {
    assertHastConformance("- # Foo\n- Bar\n  ---\n  baz\n");
  });

  test("spec 321: list item with blockquote and code", () => {
    assertHastConformance("- a\n  > b\n  ```\n  c\n  ```\n- d\n");
  });
});

describe("CommonMark spec deltas: URL encoding", () => {
  test("spec 526: autolink with ] in URL", () => {
    assertHastConformance("[foo<https://example.com/?search=](uri)>\n");
  });

  test("spec 538: autolink with ][ in URL", () => {
    assertHastConformance("[foo<https://example.com/?search=][ref]>\n\n[ref]: /uri\n");
  });

  test("spec 603: autolink with escaped brackets", () => {
    assertHastConformance("<https://example.com/\\[\\>\n");
  });
});

describe("CommonMark spec deltas: list spread detection", () => {
  test("spec 259: nested blockquote ordered list with blank continuation", () => {
    assertHastConformance("   > > 1.  one\n>>\n>>     two\n");
  });

  test("spec 325: list item with sublist and trailing content becomes loose", () => {
    assertHastConformance("* foo\n  * bar\n\n  baz\n");
  });
});

describe("CommonMark spec deltas: HTML block in list item", () => {
  test("regression 175: code block followed by HTML block in list item", () => {
    assertHastConformance("*\n      <div>\n     <div>\n");
  });
});

describe("CommonMark spec deltas: image alt text", () => {
  test("spec 574: nested image in image alt", () => {
    assertHastConformance("![foo ![bar](/url)](/url2)\n");
  });
});

describe("CommonMark spec deltas: fuzz-discovered regressions", () => {
  test("fuzz: GFM table with single-char header row", () => {
    assertMdastConformance("r\n|-");
  });

  test("fuzz: invalid HTML comment syntax stays as text", () => {
    assertMdastConformance("<!~@7reg>)");
  });

  test("fuzz: GFM table with punctuation-only header", () => {
    assertHastConformance("06*!@)(\n-|");
  });

  test("fuzz: invalid HTML tag syntax stays as paragraph text", () => {
    assertHastConformance("<c-!@9>#>}");
  });

  test("fuzz: single-tilde strikethrough around content with inner tildes", () => {
    assertHastConformance("~o5o~~#(~");
  });

  test("fuzz: GFM table with escaped backslash before delimiter", () => {
    assertHtmlConformance("]g\\\n|-");
  });

  // A line starting with `- ` (bullet-list marker) takes precedence over being
  // a GFM table delimiter row, even when the column counts match.
  test("bullet-list marker beats table delimiter (no backslash)", () => {
    assertHtmlConformance("a | b\n- | -\n1 | 2\n");
  });

  test("bullet-list marker beats table delimiter (with hard-break backslash)", () => {
    assertHtmlConformance("a | b\\\n- | -\n1 | 2\n");
  });

  // Remark keeps any `{...}` suffix in a heading as plain text rather than
  // stripping it as an attribute block (heading attributes aren't part of
  // CommonMark / GFM).
  test("fuzz: heading with brace suffix stays as text", () => {
    assertHastConformance("# { ()g}");
  });

  // Backtick in HTML attribute values is serialized as `&#x60;` to match
  // rehype-stringify (which escapes `` ` `` for legacy-browser safety).
  test("fuzz: backtick in code-fence language is entity-encoded", () => {
    assertHtmlConformance("~~~r`|");
  });
});

describe("GFM autolink-literal: over-parenthesised URL", () => {
  // remark re-tokenises an over-balanced `[40](…)` URL via the GFM autolink-literal
  // post-pass (above the 32-paren inline-link cap); cmark-gfm leaves it raw.
  test("over-balanced parens URL re-autolinked as literal", () => {
    assertHtmlConformance(
      "[30](https://rust.org/something%3A((((((((((((((((((((((((((((((())))))))))))))))))))))))))))))))\n" +
        "[40](https://rust.org/something%3A((((((((((((((((((((((((((((((((((((((((())))))))))))))))))))))))))))))))))))))))))\n",
    );
  });
});

describe("GFM tables: stray pipe-only lines", () => {
  test("bare-pipe line continues the table as empty-cell row", () => {
    assertHtmlConformance(
      "| Table | Header |\n|-------|--------|\n| Table | Body   |\n|\n| Not   | Enough |\n",
    );
  });

  test("bare-pipe line right after delimiter row is a body row, not a paragraph", () => {
    assertHtmlConformance("| Table | Header |\n|-------|--------|\n|\n");
  });
});

describe("Directive: nested same-fence closer", () => {
  test("single `:::` closes nested same-length fences, trailing `:::` becomes paragraph", () => {
    assertExtHastConformance(":::a\n:::b\n\n:::\n:::\n", ["directive"]);
  });
});

describe("GFM list-item edge cases", () => {
  // remark emits a single newline between the indented code block and the trailing HTML block, not a blank line.
  test("indented code block then HTML block inside list item", () => {
    assertHtmlConformance("*\n      <div>\n     <div>\n");
  });

  test("task-list marker ends line, next line carries lazy paragraph content", () => {
    assertHtmlConformance("- [x]\n\\\n-\n");
  });

  // When the next line is a paragraph interrupt (nested list / blockquote), the `[x]` is NOT a task marker; the item becomes plain text + a nested block.
  test("task-list marker ends line, next line is a paragraph interrupt", () => {
    assertHtmlConformance(
      "- [x] * some text\n- [ ] > some text\n- [x]\n  * some text\n- [ ]\n  > some text\n",
    );
  });
});

// pulldown-cmark and cmark-gfm produce different emphasis/strikethrough
// trees than remark/micromark for the cases below. satteri tracks remark.
describe("Emphasis: remark vs cmark divergences", () => {
  test("regression 729: triple asterisk run keeps outer `*` literal", () => {
    assertHtmlConformance("j***5*=*\n");
  });

  test("regression 741: `**a.*.**a*.**.` resolves as single strong + nested em", () => {
    assertHtmlConformance("**a.*.**a*.**.\n");
  });
});

describe("GFM strikethrough: remark flanking rule", () => {
  test("`~~!~~` against intraword punctuation stays literal", () => {
    assertHtmlConformance("Here I strike out an exclamation point~~!~~.\n");
  });
});
