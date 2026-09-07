import { describe, test, expect } from "vitest";
import {
  assertMdastConformance,
  assertMdastConformanceNoPosition,
  assertHastConformance,
  assertHtmlConformance,
  assertExtMdastConformance,
  assertExtHastConformance,
  satteriMdast,
  referenceMdast,
  assertSliceInvariantEverywhere,
} from "./helpers.js";

const MATH: ["math"] = ["math"];

describe("HTML block in list item", () => {
  test("`<textarea>` in list item keeps trailing newline on close", () => {
    assertMdastConformance("+\t<textarea>\n\nfoo");
  });

  test("complex orphan HTML table fragments + list + paragraph", () => {
    assertMdastConformance(
      "*\n@9jifle>\n\n  <tr>\n\n    <td>\n      Hi\n    </td>\n\n  </tr>\n\n</table>\n",
    );
  });
});

describe("indented code merging after list sibling", () => {
  test("indented code merges across blanks after list sibling", () => {
    assertMdastConformance("- a\n- Foo\n\n      bar\n\n      baz");
  });

  test("three blank lines between indented code lines stay one block", () => {
    assertMdastConformance("- a\n- Foo\n\n      bar\n\n\n      baz");
  });

  test("deeply nested list, then sibling, then indented code with blanks", () => {
    assertMdastConformance("[r#kh- b\n  - c\n   - d\n    - e\n- Foo\n\n      bar\n\n\n      baz");
  });
});

describe("email autolinks preceded by `[`", () => {
  test("`[skeLTO:FOO@BAR.BAZ>` keeps `>\\n$` as one trailing text", () => {
    assertMdastConformance("[skeLTO:FOO@BAR.BAZ>\n$");
  });
});

describe("type-7 HTML on lazy line in list item", () => {
  test("eof without trailing newline: html at root", () => {
    assertMdastConformance('- a\n<a href="x">');
  });

  test("bare tag name + EOF no newline: html at root", () => {
    assertMdastConformance("- a\n<unknown>");
  });

  test("trailing newline: html stays INSIDE list item", () => {
    assertMdastConformance("- a\n<a>\n");
  });

  test("blank line after html: html stays INSIDE list item", () => {
    assertMdastConformance("- a\n<a>\n\n");
  });

  test("text after html: html inside, text as sibling paragraph", () => {
    assertMdastConformance("- a\n<a>\nx");
  });

  test("bracketed list item content followed by HTML and paragraphs", () => {
    assertMdastConformance("+  $o[Foo bar]:\n<my url>\n'title'\n\n[Foo bar]\n");
  });
});

describe("autolink suppressed by unbalanced `[`", () => {
  test("`[https://foo` rejected (no `.` in domain)", () => {
    assertMdastConformance("[https://foo");
  });

  test("`[https://foo.bar` accepted via find-and-replace (has `.`)", () => {
    assertMdastConformance("[https://foo.bar");
  });

  test("`[foo<https://foo` rejected (failed angle-autolink leaves bracket open)", () => {
    assertMdastConformance("[foo<https://foo");
  });
});

describe("bracket+URL trail split", () => {
  test("`]` after URL preceded by `[` becomes its own text node", () => {
    assertMdastConformance("[https://foo.bar] x");
  });

  test("`]` followed by ` >` keeps `]` and ` >` separate", () => {
    assertMdastConformance("f[< https://foo.barq\\eh] >");
  });

  test("URL preceded by `[` at start of paragraph splits trail", () => {
    assertMdastConformance("[< https://foo.barq] x");
  });

  test("autolink not preceded by `[` keeps `] y` merged", () => {
    assertMdastConformance("x https://foo.barq] y");
  });
});

describe("footnote definition label escapes", () => {
  test("`[^foot\\\\]: ...` label has unescaped backslash", () => {
    assertMdastConformance("[^foot\\\\]: footnote");
  });

  test("`[^l{r\\\\]: /uri` plus reference link mixes label escape rules", () => {
    assertMdastConformance("[^l{r\\\\]: /uri\n\n[bar\\\\]\n");
  });

  test("footnote identifier is case-folded", () => {
    assertMdastConformance("[^lFOO]: /url\n\n[Foo]\n");
  });

  test("footnote reference identifier is case-folded too", () => {
    assertMdastConformance("[^Doh]: I know.\n\n[^Doh] reference");
  });
});

describe("HTML block trailing newline", () => {
  test("`><style\\n\\nfoo`: single-line blockquote html trims trailing \\n (blank)", () => {
    assertMdastConformance("><style\n\nfoo");
  });

  test("`><style\\n>more`: multi-line content joined with \\n", () => {
    assertMdastConformance("><style\n>more");
  });

  test("blockquote html with multi-line + lazy continuation", () => {
    assertMdastConformance('><style\n  type="text/css">\n\nfoo\n');
  });

  test("list-item html comment closed by lazy line trims trailing \\n", () => {
    assertMdastConformance("* <!-- this is a --\ncomment - with hyphens -->\n");
  });

  test("list-item html closed by blank line keeps trailing \\n", () => {
    assertMdastConformance("+\t<style\n\nbar");
  });

  test("list-item html closed by lazy line (no blank) trims \\n", () => {
    assertMdastConformance("+\t<style\nbar");
  });
});

describe("trim-lines on text→hast", () => {
  test("`&#9;` decoded to tab on continuation line is stripped in hast", () => {
    assertHastConformance("n\n&#9;foo\n");
  });

  test("trailing spaces before a soft-break are stripped in hast", () => {
    assertHastConformance("a   \nb");
  });
});

describe("GFM tables", () => {
  test("minimal `header\\n:-` table is recognized", () => {
    assertHastConformance("r5\n:-");
  });

  test("delimiter cell with internal whitespace is rejected (`- -`)", () => {
    assertMdastConformance("h\n| - - |");
  });

  test("delimiter cell with two trailing colons is rejected (`-::`)", () => {
    assertMdastConformance("h\n-::-");
  });

  test("overflow cells preserved in MDAST, dropped in HAST", () => {
    assertMdastConformance("h | h2 |\n| - | - |\n| a | b | c");
    assertHastConformance("h | h2 |\n| - | - |\n| a | b | c");
  });

  test("multiple overflow cells preserved in MDAST", () => {
    assertMdastConformance("h | h2 |\n| - | - |\n| a | b |c|d|e");
  });

  test("trailing text after last `|` is its own cell", () => {
    assertMdastConformance("h | h2 |\n| - | - |\n| a | b |trailing");
  });

  test("≥4 leading spaces after table → indented code, not row", () => {
    assertMdastConformance("a | b |\n| - | - |\n| 1 | 2 |\n     bar");
  });

  test("≥4 leading spaces also breaks tables with escaped pipes", () => {
    assertMdastConformance("a | b |\n| - | - |\n| 1 | 2\\|\n     bar");
  });

  test("3 leading spaces still allowed as table row", () => {
    assertMdastConformance("a | b |\n| - | - |\n| 1 | 2 |\n   bar");
  });
});

describe("link definitions", () => {
  test("definition label preserves trailing whitespace", () => {
    assertMdastConformance("[m(  ]:8");
  });

  test("duplicate refdef labels each get their own definition node", () => {
    assertMdastConformance('[x]: https://a.com\n\n[x]: https://b.com "t"');
  });
});

describe("math at EOF", () => {
  test("math fence at EOF with empty body keeps trailing newline in position", () => {
    assertExtMdastConformance("$$\n", MATH);
  });

  test("math fence at EOF with trailing whitespace-only line keeps it as content", () => {
    assertExtMdastConformance("$$\n ", MATH);
  });
});

describe("backslash escapes", () => {
  test("inline math after `\\\\` is still parsed", () => {
    assertExtHastConformance("\\+$+$j", MATH);
  });
});

describe("paragraph continuation", () => {
  test("`::` on continuation line stays in the paragraph", () => {
    assertMdastConformance("s\n::cw !u");
  });
});

describe("code blocks", () => {
  test("trailing indented blank line is part of the code block", () => {
    assertHtmlConformance("\t* :u4i\n\t\t");
  });
});

describe("math meta", () => {
  test("math meta preserves trailing space", () => {
    assertExtMdastConformance("$$|/0= ", MATH);
  });

  test("math meta preserves trailing tab", () => {
    assertExtMdastConformance("$$!\t\nvs*", MATH);
  });
});

describe("post-break whitespace", () => {
  test("inline math after hard break has leading whitespace trimmed", () => {
    assertExtHastConformance("a\\\n$\t$", MATH);
  });

  test("inline code after hard break has leading whitespace trimmed", () => {
    assertHastConformance("a\\\n` x`");
  });
});

describe("blockquote continuation", () => {
  test("tab followed by `>` is lazy continuation, not a marker", () => {
    assertMdastConformance(">:\n\t>");
  });

  test("space+tab followed by `>` is lazy continuation", () => {
    assertMdastConformance(">a\n \t>b");
  });
});

describe("indented code blocks", () => {
  test("trailing indented blank line preserves a separating newline", () => {
    assertMdastConformance("\tfoo\n\n\t");
  });

  test("multiple blank lines before trailing indented blank are preserved", () => {
    assertMdastConformance("\tfoo\n\n\n\t");
  });
});

describe("GFM table delimiter precedence", () => {
  test("delimiter line that's also a list marker (`{!\\n -\\t|`) is a list", () => {
    assertMdastConformance("{!\n -\t|");
  });

  test("delimiter line with leading space + space content prefers list", () => {
    assertMdastConformance("h\n - |");
  });
});

describe("inline HTML wrapping", () => {
  test("continuation line drops leading whitespace from inline HTML", () => {
    assertMdastConformance("<a\n jr_r>");
  });

  test("tab on continuation line is replaced by overflow spaces", () => {
    assertMdastConformance("<a\n\tattr>");
  });
});

describe("footnote vs definition", () => {
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

describe("refdef nesting", () => {
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

describe("light table interrupts paragraphs", () => {
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

describe("tilde delimiter flanking", () => {
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

describe("link definition position", () => {
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

describe("fenced code block position at EOF", () => {
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

describe("definition/reference label backslash unescape", () => {
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

describe("HTML block on blockquote lazy-continuation", () => {
  test("type-7 open tag with attributes on lazy line opens HTML inside blockquote", () => {
    assertMdastConformance('>oo\n<a href="bar">\nbaz');
  });

  test("type-7 close tag on lazy line opens HTML inside blockquote", () => {
    assertMdastConformance(">oo\n</a>\nbaz");
  });

  test("plain text after lazy-line HTML becomes a sibling paragraph at root", () => {
    assertMdastConformance(">ning>\n*bar*\n</Warning>\n");
  });

  test("type-7 in plain paragraph stays inline", () => {
    assertMdastConformance('oo\n<a href="bar">');
  });

  test("type-6 on lazy line pops the container", () => {
    assertMdastConformance(">x\n<div>");
  });
});

describe("GFM literal autolink with escapes", () => {
  test("URL spanning backslash escapes keeps raw source bytes", () => {
    assertMdastConformance("https://example.com/\\[\\>");
  });

  test("URL with leading text + escapes keeps raw source bytes", () => {
    assertMdastConformance("-https://example.com/\\[\\>");
  });

  test("digit-prefixed URL inside unclosed `[` produces no link", () => {
    assertMdastConformance("[0https://example.com/\\[\\>");
  });

  test("email with backslash in local-part uses find-and-replace path", () => {
    assertMdastConformance("do\\+@bar.example.com>");
  });

  test("email after `@` prev char still matches", () => {
    assertMdastConformance("o6(t@+@bar.example.com>");
  });

  test("URL with escape after open `[` uses text bytes (not raw source)", () => {
    assertMdastConformance("6[1}]pd\t[=https://example.com?find=\\*>");
  });

  test("URL after Cyrillic letter is autolinked via construct path", () => {
    assertMdastConformance("_oпhttps://example.com");
  });

  test("email after `/` retries shorter walkback until prev is acceptable", () => {
    assertMdastConformance("/5w3+special@Bar.baz-bar0.com>");
  });

  test("invalid `\\h` doesn't hide URL from backtick suppression", () => {
    assertMdastConformance(":wh\\https://foo.bar.`baz>`");
  });
});

describe("MDX inline expression after backslash-escaped `<`", () => {
  test("escaped `<` doesn't suppress later inline expression", () => {
    // This assertion covers plain Markdown; the MDX fuzz suite checks expression parsing separately.
    assertMdastConformance('[r\\<Foo bar={1} baz="two"/>h');
  });

  test("escaped `<` on prior line doesn't suppress blockquote interrupt", () => {
    assertMdastConformance("7\\<o\n> bar");
  });
});

describe("indented code split after blockquote close", () => {
  test("empty blockquote then per-line indented code splits", () => {
    assertMdastConformance(">\n    bar\n    baz");
  });

  test("blockquote with leaf then per-line indented code splits", () => {
    assertMdastConformance("> # Foo\n    > bar\n    > baz");
  });

  test("blank line between bq and indented code allows merge", () => {
    assertMdastConformance("> Foo\n\n    bar\n    baz");
  });

  test("lazy zone clears after first one-line code block", () => {
    assertMdastConformance(">\n    bar\n    baz\n\n    qux");
  });

  test("indented code after wide-marker list item splits per line", () => {
    assertMdastConformance("-    foo\n\n    bar\n    baz");
  });

  test("indented code after wide-marker list item: extends after first split", () => {
    assertMdastConformance("-    foo\n\n    bar\n    baz\n    qux");
  });

  test("list with fence then per-line indented code splits", () => {
    assertMdastConformance("-    ```\n    aaa\n    ```");
  });
});

describe("CDATA inline HTML close requires `]]>`", () => {
  test("CDATA with one `]` is not a complete close", () => {
    assertMdastConformance("foo <![CDATA[>&<]>");
  });
});

describe("protocol autolink first-char rejection", () => {
  test("`http://-` rejected (first body char is punctuation)", () => {
    assertMdastConformance("Foo\n-<http://--\nbar");
  });

  test("`https://-foo` rejected by both paths", () => {
    assertMdastConformance("https://-foo");
  });

  test("`https://_foo` rejected by both paths", () => {
    assertMdastConformance("https://_foo");
  });

  test("`https://.foo` accepted via find-and-replace path", () => {
    assertMdastConformance("https://.foo");
  });

  test("`https://..` rejected (splitUrl trims to empty)", () => {
    assertMdastConformance("https://..");
  });

  test("`https://../` accepted via find-and-replace path", () => {
    assertMdastConformance("https://../");
  });

  test("`https://-foo.bar` accepted via find-and-replace path", () => {
    assertMdastConformance("https://-foo.bar");
  });

  test("`https://foo_bar` rejected (no `.`, parts.length<2)", () => {
    assertMdastConformance("https://foo_bar");
  });

  test("`https://foo_bar.com` rejected (`_` in penult segment)", () => {
    assertMdastConformance("https://foo_bar.com");
  });

  test("`https://foo.bar_baz` rejected (`_` in last segment)", () => {
    assertMdastConformance("https://foo.bar_baz");
  });
});

describe("email walkback past `_`", () => {
  test("email starts after `_` via find-and-replace path", () => {
    assertMdastConformance("$/_ipecial@Bar.baz-bar0.com>");
  });
});

describe("code span across lines suppresses autolink", () => {
  test("paragraph-scoped earlier-backtick beats autolink-inside suppression", () => {
    assertMdastConformance("pz  _xlo`\n<https://foo.bar.`baz>`");
  });
});

describe("MDX JSX whitespace around `=`", () => {
  test("space before `=` parses as attribute", () => {
    assertMdastConformance("<Foo bar = 'baz'/>");
  });

  test("space after `=` parses as attribute", () => {
    assertMdastConformance("zj<Foo bar= {1}/>");
  });
});

describe("indented code after empty list item", () => {
  test("`*\\n\\n      bar\\n      baz` → two code blocks at root", () => {
    assertMdastConformance("*\n\n      bar\n      baz");
  });

  test("`*\\n\\n      bar\\n\\n      baz` (blank in between): two blocks", () => {
    assertMdastConformance("*\n\n      bar\n\n      baz");
  });

  test("non-empty list-item keeps merged code block", () => {
    assertMdastConformance("- a\n\n      bar\n      baz");
  });
});

describe("fenced code block trim on container outdent", () => {
  test("`- ```\\n  b\\n\\noo` → code value `b`", () => {
    assertMdastConformance("- ```\n  b\n\noo");
  });

  test("two blank lines before outdent: code value `b\\n`", () => {
    assertMdastConformance("- ```\n  b\n\n\noo");
  });

  test("multi-line content + blank + outdent: trailing \\n trimmed", () => {
    assertMdastConformance("- ```\n  b\n  c\n\noo");
  });
});

describe("list extension first-content-line only absorbs `>`", () => {
  test("`>+ # Foo\\n> bar\\n> baz\\n` keeps list end at line 1", () => {
    assertMdastConformance(">+ # Foo\n> bar\n> baz\n");
  });

  test("`>- one\\n>>` still extends list to col 2 of line 2", () => {
    assertMdastConformance(">- one\n>>");
  });
});

describe("list/bq extension stops at indented-code threshold", () => {
  test("`>* > # Foo` then 4-space indented `> bar` keeps positions tight", () => {
    assertMdastConformance("~-{tg\t\n>* > # Foo\n    > bar\n    > baz\n");
  });
});

describe("HAST footnote elements carry position", () => {
  test("footnote ref/def positions appear in HAST", () => {
    assertHastConformance("j4nu0[^y]\n\nrvt[^bxmw]\n\n[^y]: 4quj08jtc\n");
  });
});

describe("GFM email rejects when domain ends in `-`, digit, or `_`", () => {
  test("trailing `-` after domain: no email", () => {
    assertHtmlConformance("foo@bar.com-");
  });

  test("trailing digit in domain: no email", () => {
    assertHtmlConformance("foo@bar.com12");
  });

  test("trailing `_` after domain: no email", () => {
    assertHtmlConformance("foo@bar.com_");
  });

  test("trailing `.` after domain: email kept, `.` stays as text", () => {
    assertHtmlConformance("foo@bar.com.");
  });

  test("compound: leading and trailing dashes around email", () => {
    assertHtmlConformance("-----foo@bar.example.c----");
  });

  test("setext heading followed by a paragraph containing an email", () => {
    assertHtmlConformance(
      "\\$l->yhwn#\n\tFoo *bar*\n=========\n\nFoo *bar*\n-----foo@bar.example.c----\n",
    );
  });
});

describe("inline HTML clears backslash-escape on trail", () => {
  test("inline HTML with escaped char in attribute: trail position correct", () => {
    assertMdastConformance('foo <a href="\\*">>\t\tfoo\n');
  });
});

describe("CommonMark autolink clears backslash-escape on trail", () => {
  test("autolink with escaped close delimiter: trail starts after `>`", () => {
    assertMdastConformance("<https://example.com/\\>foo");
  });

  test("autolink with multiple escapes inside URL: trail position correct", () => {
    assertMdastConformance("<https://example.com/\\[\\>foo");
  });

  test("compound paragraph with escape autolink + trail", () => {
    assertMdastConformance("f*bar*baz**\n<https://example.com/\\[\\>c =:d");
  });
});

describe("URL encoding for invalid percent-encoding", () => {
  test("invalid `%2^` becomes `%252%5E`", () => {
    assertHtmlConformance("ba/[link](foo%2^b&auml;)\n");
  });

  test("valid `%20` stays as `%20`", () => {
    assertHtmlConformance("[link](foo%20bar)");
  });

  test("trailing `%` (no hex) becomes `%25`", () => {
    assertHtmlConformance("[link](foo%)");
  });
});

describe("list-item end extends through next marker when last child ends after \\n", () => {
  test("`- ```\\n- d\\n` → listItem 1 end at content col of listItem 2", () => {
    assertMdastConformance("- ```\n- d\n");
  });

  test("compound: HTML+empty fenced + blanks + close + next listItem", () => {
    assertMdastConformance("- </a>```\n  b\n\n\n  ```\n- c\n");
  });
});

describe("autolink continuation line drops leading space", () => {
  test("email autolink on continuation line skips leading indent space", () => {
    assertMdastConformance("r<@\n special@Bar.baz-bar0.com>\n");
  });
});

describe("list extension absorbs trailing tab on blank marker line", () => {
  test("blank `>\\t` marker line: list end past tab", () => {
    assertMdastConformance(">-\n>\t\n:7^");
  });
});

describe("link refdef resolves in source order, not node-id order", () => {
  test("top-level def with title wins over bq-nested def without title (HTML)", () => {
    assertHtmlConformance('[foo]: /url "title"\n\n[foo]\n\n> [foo]: /url');
  });

  test("image reference also picks the top-level def with title (HTML)", () => {
    assertHtmlConformance('![foo][]\n\n[foo]: /url "title"\n[foo]\n\n> [foo]: /url');
  });
});

describe("nested list inside list-item-blockquote does not extend through blank `>>`", () => {
  test("nested bq>bq>list inside list-item leaves positions at line 1", () => {
    assertMdastConformance("-   > > 1.  one\n>>\n>>     two\n");
  });
});

describe("footnote definition trims trailing whitespace", () => {
  test("trailing space after content", () => {
    assertMdastConformance("[^a]: foo\n ");
  });

  test("blank line + trailing space", () => {
    assertMdastConformance("[^a]: foo\n\n ");
  });

  test("multiple trailing spaces", () => {
    assertMdastConformance("[^a]: foo\n  ");
  });

  test("compound: fn def followed by blank + space at EOF", () => {
    assertMdastConformance("[^xawyfy]: mt8owfjngw\n\n ");
  });
});

describe("empty unclosed fenced code in list-item before new container", () => {
  test("empty fenced followed by new list (different marker)", () => {
    assertMdastConformance("*\t  ```\n  c\n  ```\n- d\n");
  });

  test("empty fenced followed by blockquote keeps \\n", () => {
    assertMdastConformance("- ```\n> foo\n");
  });

  test("empty fenced followed by ordered list keeps \\n", () => {
    assertMdastConformance("- ```\n1. foo\n");
  });

  test("empty fenced followed by ATX heading drops \\n", () => {
    assertMdastConformance("- ```\n# h\n");
  });

  test("empty fenced followed by indented code drops \\n", () => {
    assertMdastConformance("-    ```\n    aaa\n    ```");
  });
});

describe("`~~` flanking when followed by punctuation", () => {
  test("`a~~/foo~~` stays text (alnum before, punct after)", () => {
    assertMdastConformance(":f~~/42e~~\n[]\n~~~\n");
  });

  test("`x~~.foo~~y` similar pattern", () => {
    assertMdastConformance("x~~.foo~~y");
  });
});

describe("autolink + backtick code-span ordering", () => {
  test("`[\\nhttps://foo.bar.\\`baz>\\`` splits URL and code span", () => {
    assertMdastConformance("[\nhttps://foo.bar.`baz>`");
    assertHastConformance("[\nhttps://foo.bar.`baz>`");
    assertHtmlConformance("[\nhttps://foo.bar.`baz>`");
  });

  test("backslash-prefix + multi-line variant", () => {
    assertMdastConformance("\\@: \t*=8[\nhttps://foo.bar.`baz>`\n");
    assertHastConformance("\\@: \t*=8[\nhttps://foo.bar.`baz>`\n");
    assertHtmlConformance("\\@: \t*=8[\nhttps://foo.bar.`baz>`\n");
  });

  test("URL with interior backticks (no bracket) stays as URL", () => {
    assertMdastConformance("https://foo.bar.`baz>`");
  });
});

describe("HTML attribute leniency for type-7 blocks", () => {
  test('`<img src=title="*"/>` parses as HTML block', () => {
    assertMdastConformance('<img src=title="*"/>\n');
    assertHastConformance('<img src=title="*"/>\n');
    assertHtmlConformance('<img src=title="*"/>\n');
  });

  test("unquoted chain `foo=a=b` accepted", () => {
    assertMdastConformance("<xyzzy foo=a=b/>\n");
  });

  test("quoted value followed by junk still rejected", () => {
    assertMdastConformance('<xyzzy foo="a"="b"/>\n');
    assertMdastConformance('<xyzzy a="b"c="d"/>\n');
  });
});

describe("strikethrough/emphasis two-pass resolve", () => {
  test("`*~bar~*` → emphasis wraps delete", () => {
    assertMdastConformance("*~bar~*");
    assertHtmlConformance("*~bar~*");
  });

  test("`**~bar~**` → strong wraps delete", () => {
    assertMdastConformance("**~bar~**");
  });

  test("`*foo~bar~*` → emphasis(text + delete)", () => {
    assertMdastConformance("*foo~bar~*");
  });

  test("`*foo~bar~baz*` → emphasis(text + delete + text)", () => {
    assertMdastConformance("*foo~bar~baz*");
  });

  test("`[*~bar~*](url)` → link with emphasis(delete) inside", () => {
    assertMdastConformance("[*~bar~*](url)");
  });

  test("`_*~bar~*_` → emphasis(emphasis(delete))", () => {
    assertMdastConformance("_*~bar~*_");
  });

  test("`*foo*~bar~*baz*` → emphasis + delete + emphasis (sibling)", () => {
    assertMdastConformance("*foo*~bar~*baz*");
  });

  test("`_/~z)*~*nf` → emphasis(~), no delete (would cross)", () => {
    assertMdastConformance("_/~z)*~*nf");
    assertHastConformance("_/~z)*~*nf");
    assertHtmlConformance("_/~z)*~*nf");
  });

  test("`*>+~(-[_~_` emphasis claims its span before single-~ pair", () => {
    assertMdastConformance("*>+~(-[_~_");
    assertHastConformance("*>+~(-[_~_");
    assertHtmlConformance("*>+~(-[_~_");
  });

  test("`~_a_~` → delete(emphasis(a))", () => {
    assertMdastConformance("~_a_~");
  });

  test("`~_a_~_b_` → delete(emphasis(a)) + emphasis(b)", () => {
    assertMdastConformance("~_a_~_b_");
  });

  test("`~_~:_<` → delete(_), no emphasis (capturing wins)", () => {
    assertMdastConformance("~_~:_<");
  });

  test("`~*~:*<` → delete(*), no emphasis (capturing wins)", () => {
    assertMdastConformance("~*~:*<");
  });

  test("`~#\\=_~:_<` → delete with escape, no emphasis", () => {
    assertMdastConformance("~#\\=_~:_<");
  });

  test("`#~_n~>=` → strikethrough survives across `_`", () => {
    assertMdastConformance("#~_n~>=");
  });

  test("`~foo*bar*~` → delete with emphasis inside", () => {
    assertMdastConformance("~foo*bar*~");
  });

  test("`~~text~~` → delete", () => {
    assertMdastConformance("~~text~~");
  });

  test("`*~~text~~*` → emphasis(delete)", () => {
    assertMdastConformance("*~~text~~*");
  });

  test("`~~!~~` → delete with single char", () => {
    assertMdastConformance("~~!~~");
  });

  test("`*~bar~*` keeps single-tilde semantics", () => {
    assertHastConformance("*~bar~*");
  });

  test("inert leading `_` makes emphasis resolve first", () => {
    assertHtmlConformance("_ ~a*b~*\n");
    assertHtmlConformance("_  ~a*ab~*\n");
  });

  test("literal `^` doesn't skew emphasis/strikethrough order", () => {
    assertHtmlConformance("^*~*a~ ~_\n");
    assertHtmlConformance("^ ~a*b~*\n");
  });

  test("nested content resolves strikethrough-first regardless of top-level", () => {
    assertHtmlConformance("_~a*b~*\n");
    assertHtmlConformance("[_~a*b~*](/x)\n");
    assertHtmlConformance("![_~a*b~*](/i)\n");
    assertHtmlConformance("[!www..a;_~%*_~](/x)\n");
  });
});

describe("URL percent-encoding", () => {
  test("`%` + two alphanumerics is kept, not re-encoded to `%25`", () => {
    assertHtmlConformance("www.x.com/a%ax\n");
    assertHtmlConformance("www.example.com/100%off\n");
    assertHtmlConformance("[x](http://h/a%2gb)\n");
  });
  test("`%` not followed by two alphanumerics is encoded", () => {
    assertHtmlConformance("[x](http://h/a%2)\n");
    assertHtmlConformance("[x](http://h/a% b)\n");
  });
});

describe("email autolink underscore in domain", () => {
  test("`xg@_xample.com` → email (leading `_` in penult is fine)", () => {
    assertMdastConformance("7!xg@_xample.com\n");
  });

  test("`foo@bar_baz.com` → email (`_` in penult is fine)", () => {
    assertMdastConformance("foo@bar_baz.com");
  });

  test("`foo@bar.b_z` → email (`_` in TLD interior is fine)", () => {
    assertMdastConformance("foo@bar.b_z");
  });

  test("`foo@bar_.com` → email (`_` at penult end is fine)", () => {
    assertMdastConformance("foo@bar_.com");
  });

  test("`foo@a_b.c` → email (`_` mid-penult is fine)", () => {
    assertMdastConformance("foo@a_b.c");
  });

  test("`foo@a.b_` → NOT an email (label ends in `_`)", () => {
    assertMdastConformance("foo@a.b_");
  });
});

describe("GFM autolink trail split on newline", () => {
  test("`https://../>\\nfoo` → link + text `>` + text `\\nfoo`", () => {
    assertMdastConformance("6r1# #https://../>\nfoo******bar*********baz");
  });

  test("trail at EOF kept as separate text node", () => {
    assertMdastConformance("https://../>");
  });

  test("trail with same-line content stays merged", () => {
    assertMdastConformance("See (https://example.com) for details");
  });
});

describe("GFM email domain can't start with `.`", () => {
  test("`2@\\.baz>` (text `2@.baz`) is NOT an email", () => {
    assertMdastConformance("t\t2@\\.baz>\n");
  });

  test("`@.foo.bar` not an email (domain starts with dot)", () => {
    assertMdastConformance("@.foo.bar");
  });
});

describe("code span container scan beyond span", () => {
  test("tab-marker list item: code span keeps trailing indent", () => {
    assertMdastConformance("+\t w4-w```\naaa\n    ```\n");
  });

  test("tab-marker list item: code span EOF-no-newline keeps trailing indent", () => {
    assertMdastConformance("+\t w4-w```\naaa\n    ```");
  });
});

describe("shortcut link suppressed by following `[`", () => {
  test("`[foo][ref[]` stays text (inner `[` invalidates label)", () => {
    assertMdastConformance("[foo][ref[]\n\n[foo]: /url");
  });

  test("`[foo][ref[]` with no definition: also plain text", () => {
    assertMdastConformance("[foo][ref[]");
  });
});

describe("table delimiter row last-cell hyphen check", () => {
  test("`-|:` (no `-` in last cell) is NOT a delimiter row", () => {
    assertMdastConformance("[is:|{\n-|:");
  });

  test("`|::|` (no `-` anywhere) is NOT a delimiter row", () => {
    assertMdastConformance("a|b\n|::|");
  });
});

describe("footnote def with leading whitespace in label", () => {
  test("`[^ *o]: url` becomes a refdef, not a footnote", () => {
    assertMdastConformance('[^ *o]: /url\n"title" ok\n');
  });

  test("`[^foo ]:` (trailing space) also rejected as footnote", () => {
    assertMdastConformance("[^foo ]: /url");
  });
});

describe("fenced code block trailing whitespace at EOF", () => {
  test("single space line at EOF kept as code content", () => {
    assertMdastConformance("~~~s\n ");
  });

  test("trailing tab line at EOF kept", () => {
    assertMdastConformance("~~~s\n\t");
  });
});

describe("ordered list start≠1 after indented code", () => {
  test("`    code\\n\\n2. b` → [code, paragraph]", () => {
    assertMdastConformance("    code\n\n2. b");
  });

  test("multiple blank lines don't reset (still paragraph)", () => {
    assertMdastConformance("    code\n\n\n\n2. b");
  });

  test("intervening paragraph clears the suppression", () => {
    assertMdastConformance("    code\n\nx\n\n2. b");
  });

  test("start=1 always opens a new list", () => {
    assertMdastConformance("    code\n\n1. b");
  });

  test("`)` delimiter form also suppressed", () => {
    assertMdastConformance("    code\n\n2) b");
  });
});

describe("MDX list-item content column", () => {
  // This assertion covers plain Markdown; the MDX fuzz suite checks its different indentation rules.
  test("plain mdast: 5+ space marker keeps clamped content col", () => {
    assertMdastConformance("1.     abc\n\n   def");
  });
});

describe("autolink text chunk covers backslash-escape source", () => {
  test("leading `\\[` before email autolink keeps escape in text span", () => {
    assertMdastConformance("\\[foo@bar.example.com");
  });

  test("`!7j4{h\\:3)v{` text before email autolink covers `\\:` source", () => {
    assertMdastConformance("!7j4{h\\:3)v{j@bar.example.com>");
  });
});

describe("bracket depth propagates across inline parents", () => {
  test("`![` inside emphasis suppresses position on following autolink", () => {
    assertMdastConformance("*![fw*https://example.com");
  });
});

describe("link with escape in URL leaves trail position unshifted", () => {
  test("`[link](foo\\()trail` puts trail at offset 13 not 12", () => {
    assertMdastConformance("[link](foo\\()trail");
  });

  test("`&[link](foo\\(an)r> d\\(bar\\))` trail spans 16..28 (one escape)", () => {
    assertMdastConformance("&[link](foo\\(an)r> d\\(bar\\))");
  });
});

describe("text_to_source skips trimmed line-ending whitespace", () => {
  test("trailing space before soft-break keeps source-byte alignment", () => {
    assertMdastConformance(":rap#| \n6!< https://foo.bar >");
  });

  test("leading whitespace on continuation line keeps source-byte alignment", () => {
    assertMdastConformance("z\n i}@f<https://foo.bar/baz bim>");
  });
});

describe("list inside blockquote extends position to absorb trailing markers", () => {
  test("`>>- one\\n>>\\n  >  > two` list extends through blank `>>`", () => {
    assertMdastConformance(">>- one\n>>\n  >  > two");
  });

  test("`>- one\\n>>` list extends past first `>` on continuation line", () => {
    assertMdastConformance(">- one\n>>\n  >  > two");
  });

  test("`>- one\\n>` (no next sibling) list extends past `>` on line 2", () => {
    assertMdastConformance(">- one\n>");
  });
});

describe("blockquote-parented fenced code at EOF trims trailing newline", () => {
  test("`>\\`\\`\\`\\n` blockquote/code both end at offset 4 (before `\\n`)", () => {
    assertMdastConformance(">```\n");
  });

  test("`\\`\\`\\`\\n` (top-level) keeps trailing newline in code span", () => {
    assertMdastConformance("```\n");
  });
});

describe("setext heading extends position to preceding adjacent definition", () => {
  test("`[foo]: /url\\nbar\\n===` heading start extends to def start (offset 0)", () => {
    assertMdastConformance("[foo]: /url\nbar\n===");
  });

  test("` [foo]: /url\\nbar\\n===` heading start extends to def start (offset 1)", () => {
    assertMdastConformance(" [foo]: /url\nbar\n===");
  });

  test("`[foo]: /url\\n\\nbar\\n===` (blank between) heading starts at line 3", () => {
    assertMdastConformance("[foo]: /url\n\nbar\n===");
  });

  test("`[foo]: /url\\n[foo]: /url\\nbar\\n===` chains through both defs", () => {
    assertMdastConformance(" 5o] bar\n\n[foo]: /url\n[foo]: /url\nbar\n===\n[foo]");
  });

  test("three adjacent defs chain back to the first", () => {
    assertMdastConformance("[a]: /1\n[b]: /2\n[c]: /3\nbar\n===");
  });

  test("`[foo]: /url\\n-\\nbaz\\n===` first content `-` breaks chain", () => {
    assertMdastConformance("[foo]: /url\n-\nbaz\n===");
  });

  test("`[foo]: /url\\n=\\nbaz\\n===` first content `=` breaks chain", () => {
    assertMdastConformance("[foo]: /url\n=\nbaz\n===");
  });

  test("`[foo]: /url\\n+\\nbaz\\n===` first content `+` (NOT underline char) keeps chain", () => {
    assertMdastConformance("[foo]: /url\n+\nbaz\n===");
  });

  test("`[foo]: /url\\n-x\\nbaz\\n===` `-x` is text, not underline shape — chain", () => {
    assertMdastConformance("[foo]: /url\n-x\nbaz\n===");
  });

  test("`[foo]: /url\\n   -\\nbaz\\n===` 3-space indent + `-` IS underline shape — break", () => {
    assertMdastConformance("[foo]: /url\n   -\nbaz\n===");
  });

  test("definition followed by a `-` line and list items with code blocks", () => {
    assertMdastConformance(
      "/_-/zr]\n\n[foo]: /url1\n-\n  foo\n-\n  ```\n  bar\n  ```\n-\n      baz",
    );
  });
});

describe("indented code position extension respects lazy-one-line flag", () => {
  test("`*\\n\\n    foo\\n    \\n\\n` (lazy split) code ends after `foo`", () => {
    assertMdastConformance("*\n\n    foo\n    \n\n");
  });

  test("` \\n    foo\\n    \\n\\n` (non-lazy) code still extends through `    `", () => {
    assertMdastConformance(" \n    foo\n    \n\n");
  });
});

describe("empty list-item split only when blank-separated from indented code", () => {
  test("empty marker directly followed by indented code stays in listItem", () => {
    assertMdastConformance("-\n             bbb\n                                       ccc");
  });

  test("empty marker + blank line + indented code → list + code outside", () => {
    assertMdastConformance("*\n\n    foo");
  });
});

describe("accept literal `.` as first domain char but reject source-escaped `\\.`", () => {
  test("`8z y@.bar.baz` emits email link with literal first-dot domain", () => {
    assertMdastConformance("8z y@.bar.baz");
  });

  test("`2@\\.baz` drops email entirely (source-escaped `.` after `@`)", () => {
    assertMdastConformance("2@\\.baz");
  });
});

describe("distinguish literal `\\` from escape before email", () => {
  test("`3\\e-gdafoo@bar.example.com` keeps position on email link", () => {
    assertMdastConformance("3\\e-gdafoo@bar.example.com");
  });
});

describe("email autolink suppresses position when escape directly precedes local-part", () => {
  test("`\\+@bar.example.com>` produces position-less email + trailing text", () => {
    assertMdastConformance("\\+@bar.example.com>");
  });

  test("`<\\+@bar.example.com>` keeps trailing `>` position-less too", () => {
    assertMdastConformance("<\\+@bar.example.com>");
  });
});

describe("thematic break clears list_interrupted_paragraph", () => {
  test("`    foo\\n----\\n+ ` empty marker after thematic break opens list", () => {
    assertMdastConformance("    foo\n----\n+ ");
  });

  test("`    foo\\n----\\n+` (no trailing space) same behavior", () => {
    assertMdastConformance("    foo\n----\n+");
  });
});

describe("indented code position respects container indent threshold", () => {
  test("`- Foo\\n\\n      bar\\n\\n\\n      baz\\n\\t` trailing tab stays out of list-item code", () => {
    assertMdastConformance("- Foo\n\n      bar\n\n\n      baz\n\t");
  });

  test("`    foo\\n\\t\\n` (top-level) trailing tab IS absorbed into code", () => {
    assertMdastConformance("    foo\n\t\n");
  });

  test("`    foo\\n\\t` (top-level, no trailing newline) trailing tab IS absorbed", () => {
    assertMdastConformance("    foo\n\t");
  });
});

describe("autolink construct domain extraction stops at non-domain chars", () => {
  test("`#h6:0< https://foo.barf]_q#4` autolink keeps position", () => {
    assertMdastConformance("#h6:0< https://foo.barf]_q#4");
  });
});

describe("indented code extension skipped when inside a blockquote", () => {
  test("`>     chunk1\\n      \\n      chunk2\\n` blockquote ends at line 1", () => {
    assertMdastConformance(">     chunk1\n      \n      chunk2\n");
  });
});

describe("scan_reference honors backslash-escaped opening bracket", () => {
  test("`[]\\[foo]\\n\\n[foo]: /url` produces literal `[][foo]`, not empty link", () => {
    assertHtmlConformance("[]\\[foo]\n\n[foo]: /url\n");
  });

  test("escaped reference opener with preceding punctuation and a titled definition", () => {
    assertHtmlConformance('_>~$ []\\[foo]\n\n[foo]: /url "title"\n');
  });

  test("`[]\\\\[foo]\\n\\n[foo]: /url` (double-escape) IS a full-reference link", () => {
    assertHtmlConformance("[]\\\\[foo]\n\n[foo]: /url\n");
  });
});

describe("setext heading chain-back tolerates 1-3 leading spaces on first content line", () => {
  test("`[bar]: /url\\n   Foo\\n---` (3-space indent before Foo) chains heading start to def", () => {
    assertMdastConformance("[bar]: /url\n   Foo\n---");
  });

  test("4-space indent on first content line STILL chains back (paragraph continuation)", () => {
    assertMdastConformance("[bar]: /url\n    Foo\n---");
  });

  test("reference definition followed by multiple indented setext headings", () => {
    assertMdastConformance(
      '2\tb n[foo][bar]\n\n[bar]: /url "title"\n   Foo\n---\n\n  Foo\n-----\n\n  Foo\n  ===',
    );
  });
});

describe("autolink trim uses find-and-replace set when preceded by unbalanced `[`", () => {
  test("`[ **<URL_**` keeps trailing `_` in the URL", () => {
    assertHtmlConformance("[ **<https://foo.bar.baz/tes_**\n");
  });

  test("`**<URL_**` (no unbalanced `[`) uses construct trim — `_` stripped", () => {
    assertHtmlConformance("**<https://foo.bar.baz/tes_**\n");
  });

  test("`[http://foo.bar.baz/q_` keeps trailing `_` (find-and-replace path)", () => {
    assertHtmlConformance("[http://foo.bar.baz/q_\n");
  });
});

describe("text_to_source map skips blockquote `>` prefixes", () => {
  test("`># Foo\\n>bar\\n> baz\\nfoo@bar.example.com` email link gets correct position", () => {
    assertMdastConformance("># Foo\n>bar\n> baz\nfoo@bar.example.com");
  });
});

describe("emphasis lowerbound keyed by current_count not run_length", () => {
  test("`cz*x\\`*foo***bar***baz` outer em opens between `cz` and `baz`", () => {
    assertHtmlConformance("cz*x`*foo***bar***baz\n");
  });

  test("`*x\\`*foo***bar***baz` (no `cz` prefix) same shape", () => {
    assertHtmlConformance("*x`*foo***bar***baz\n");
  });

  test("`*x\\\\`*foo***bar***baz` (escaped backtick) chains to outer", () => {
    assertHtmlConformance("*x\\`*foo***bar***baz\n");
  });
});

describe("inner blockquote extends through outer-marker-only continuation lines", () => {
  test("`>>g\\n>` inner bq extends to after outer marker on line 2", () => {
    assertMdastConformance(">>g\n>");
  });

  test("inner blockquote with punctuation extends through the outer marker", () => {
    assertMdastConformance(">>g^[( \n>");
  });

  test("`>>g\\n>>` extends through both continuation markers", () => {
    assertMdastConformance(">>g\n>>");
  });

  test("`>>g\\n` (no marker line 2) ends at content", () => {
    assertMdastConformance(">>g\n");
  });

  test("`>> foo\\n>\\n> bar\\n` inner bq does NOT extend past line 1 (next sibling exists)", () => {
    assertMdastConformance(">> foo\n>\n> bar\n");
  });

  test("`>   > > 1.  one\\n>>\\n>>     two\\n` deeply nested case with siblings", () => {
    assertMdastConformance(">   > > 1.  one\n>>\n>>     two\n");
  });
});

describe("list-marker after link reference definition", () => {
  test("`[ref]: /uri\\n1. 2. foo` — `2.` becomes text inside the `1.` item", () => {
    assertMdastConformance("[ref]: /uri\n1. 2. foo\n");
  });
  test("`[ref]: /uri\\n- 2. foo` — `2.` becomes text inside the `-` item", () => {
    assertMdastConformance("[ref]: /uri\n- 2. foo\n");
  });
  test("`[ref]: /uri\\n1. - 2. foo` — `2.` stays as text under the nested `-`", () => {
    assertMdastConformance("[ref]: /uri\n1. - 2. foo\n");
  });
  test("autolink followed by a reference definition and nested list markers", () => {
    assertMdastConformance("oo<https://example.com/?search=][ref]>\n\n[ref]: /uri\n1. - 2. foo\n");
  });
});

describe("autolink URL stops at matched-backtick code span (find-and-replace path)", () => {
  test("`[\\nURL.\\`baz>\\`` URL ends at first backtick", () => {
    // Check URL positions independently of the known trailing code-span parsing divergence.
    const ref = referenceMdast("[\nhttps://foo.bar.`baz>`\n") as any;
    const act = satteriMdast("[\nhttps://foo.bar.`baz>`\n") as any;
    const refUrl = JSON.stringify(ref).match(/"url":"([^"]*)"/)?.[1];
    const actUrl = JSON.stringify(act).match(/"url":"([^"]*)"/)?.[1];
    expect(actUrl).toBe(refUrl);
  });
});

describe("table header pipe count with consecutive backslashes", () => {
  test("`lf\\\\| a | b |` produces 3 header cells, not 2 (not a table vs 2-col delim)", () => {
    assertMdastConformance("lf\\\\| col | val |\n| --- | --- |\n| a   | $x$ |");
  });

  test("`\\\\|` inside cells: row keeps both backslashes and splits on the pipe", () => {
    assertMdastConformance("| a\\\\| b |\n| --- | --- |\n");
  });

  test("`\\|` still escapes the pipe (single backslash)", () => {
    assertMdastConformance("| a\\| b |\n| --- |\n");
  });

  test("`\\\\\\|` (three backslashes) escapes the pipe", () => {
    assertMdastConformance("a\\\\\\| b | c\n--- | ---\n");
  });
});

describe("ordered list interrupt requires textual `1.`", () => {
  test("`foo\\n01. bar` → single paragraph (leading zero forbids interrupt)", () => {
    assertMdastConformance("foo\n01. bar");
  });

  test("`foo\\n001. bar` → single paragraph", () => {
    assertMdastConformance("foo\n001. bar");
  });

  test("`foo\\n10. bar` → single paragraph (multi-digit forbids interrupt)", () => {
    assertMdastConformance("foo\n10. bar");
  });

  test("`foo\\n01) bar` → single paragraph (`)` delim too)", () => {
    assertMdastConformance("foo\n01) bar");
  });

  test("`foo\\n1. bar` → paragraph + list (textual `1.` allowed)", () => {
    assertMdastConformance("foo\n1. bar");
  });

  test("`foo\\n1) bar` → paragraph + list (textual `1)` allowed)", () => {
    assertMdastConformance("foo\n1) bar");
  });

  test("`01. bar` (no preceding paragraph) still opens a list", () => {
    assertMdastConformance("01. bar");
  });

  test("`10. bar` (no preceding paragraph) still opens a list", () => {
    assertMdastConformance("10. bar");
  });

  test("zero-prefixed list marker does not interrupt a paragraph containing math", () => {
    assertMdastConformance("vdpj\n01. ordered $a$\n2. items $b$");
  });

  test("zero-prefixed list marker with indented content does not interrupt a paragraph", () => {
    assertMdastConformance("tx\n01.      indented code\n\n   paragraph\n\n       more code\n");
  });
});

describe("lazy indented code after empty blockquote doesn't suppress next list", () => {
  test("`>\\n\\t9\\n+` → blockquote(), code:'9', list(emptyItem)", () => {
    assertMdastConformance(">\n\t9\n+");
  });

  test("`>\\n\\t9^\\n+\\np` → list opens, then paragraph 'p'", () => {
    assertMdastConformance(">\n\t9^\n+\np");
  });

  test("`\\t9\\n+` (no preceding bq) keeps suppression: paragraph '+'", () => {
    assertMdastConformance("\t9\n+");
  });

  test("`>\\n\\n\\t9\\n+` (blank line between bq and code) keeps suppression", () => {
    assertMdastConformance(">\n\n\t9\n+");
  });

  test("`    code\\n\\n2. b` still becomes [code, paragraph]", () => {
    assertMdastConformance("    code\n\n2. b");
  });
});

describe("empty list marker can't interrupt refdef-paragraph", () => {
  // CommonMark example 305 forbids empty list markers from interrupting a paragraph.
  test("`[a]: u\\n>*` keeps `*` as paragraph text inside the blockquote", () => {
    assertMdastConformance("[a]: u\n>*");
  });

  test("`[a]: u\\n>-` keeps `-` as paragraph text inside the blockquote", () => {
    assertMdastConformance("[a]: u\n>-");
  });

  test("`[a]: u\\n>+` keeps `+` as paragraph text inside the blockquote", () => {
    assertMdastConformance("[a]: u\n>+");
  });

  test("`[a]: u\\n>* foo` is still a real list (marker has content)", () => {
    assertMdastConformance("[a]: u\n>* foo");
  });

  test("`[a]: u\\n\\n>*` (blank line) opens a fresh blockquote+list", () => {
    assertMdastConformance("[a]: u\n\n>*");
  });

  test("`[a]: u\\n\\n  >*` (blank line + indent) opens a fresh blockquote+list", () => {
    assertMdastConformance("[a]: u\n\n  >*");
  });
});

describe("HTML block preserves tab-expansion leftover in blockquote", () => {
  test("`>\\t<div>` keeps 2 leading spaces in the html value", () => {
    assertMdastConformance(">\t<div>\nbar\n</div>\n*foo*\n");
  });

  test("`>\\t<style>` (type 1) keeps tab-leftover spaces", () => {
    assertMdastConformance(">\t<style>x</style>\n");
  });

  test("leading space ` <!--…-->` is NOT doubled (raw byte, not phantom)", () => {
    assertMdastConformance(" <!n=n0p");
  });
});

describe("code/html block trailing newline depends on terminator", () => {
  test("code fence + 2 blanks + new list item keeps both newlines", () => {
    assertMdastConformanceNoPosition("- ```\n  b\n\n\n2. x");
  });

  test("code fence + 2 blanks + sibling unordered marker keeps both newlines", () => {
    assertMdastConformanceNoPosition("- ```\n  b\n\n\n- x");
  });

  test("code fence + 2 blanks + blockquote keeps both newlines", () => {
    assertMdastConformanceNoPosition("- ```\n  b\n\n\n> x");
  });

  test("code fence + 2 blanks + heading still trims separator", () => {
    assertMdastConformance("- ```\n  b\n\n\n# x");
  });

  test("HTML block in blockquote ended by sibling list keeps trailing newline", () => {
    assertMdastConformance(">\t<!r foo\n- bar");
  });

  test("HTML block in blockquote ended by paragraph still trims trailing newline", () => {
    assertMdastConformance(">\t<!r foo\nbar");
  });
});

describe("refdef label decodes HTML entities and backslash escapes", () => {
  test("`[A]\\n\\n[&AElig;]: /url` decodes entity in label", () => {
    assertMdastConformance("[A]\n\n[&AElig;]: /url\n");
  });

  test("multi-line label with mixed entities + literal `&S` (invalid)", () => {
    assertMdastConformance("[ẞ]\n\n[S&nbsp; &amp; &copy; &AElig; &Dcaron;\n&frac34; &S]: /url\n");
  });
});

describe("image alt preserves inline HTML verbatim", () => {
  test('`![foo<div>\\n bar](/u "title")` keeps `<div>` in alt', () => {
    assertMdastConformance('m(q\n~k}y ![foo<div>\n bar](/path/to/train.jpg  "title"   )\n');
  });

  test("simpler: `![a<i>b](u)` keeps `<i>`", () => {
    assertMdastConformance("![a<i>b](u)");
  });
});

describe("numeric character references map control chars to U+FFFD", () => {
  test("`&#17;` (C0 control U+0011) → U+FFFD", () => {
    assertMdastConformance("8|o&#17;&#10;bar\n");
  });

  test("`&#0;` (NULL) → U+FFFD", () => {
    assertMdastConformance("&#0;");
  });

  test("`&#22;` (control) mixed with valid hex entities", () => {
    assertMdastConformance("[&#22; &#XD06; &#xcab;\n");
  });

  test("`&#xD800;` (surrogate) → U+FFFD", () => {
    assertMdastConformance("&#xD800;");
  });

  test("valid `&#9;` (TAB) and `&#10;` (LF) pass through", () => {
    assertMdastConformance("&#9;");
  });
});

describe("GFM autolink trail split based on trim-set kind", () => {
  test("trail `>` + newline + html → split into trail + `\\n` + html", () => {
    assertMdastConformance("https://../>\n</script>");
  });

  test('trail `!";` + newline + html → merge into one text node', () => {
    assertMdastConformance('a<https://foo.bar/?a!";\n</script>');
  });

  test('trail `!";` + newline + paragraph → merge', () => {
    assertMdastConformance('a<https://x/?a!";\nfoo');
  });

  test("autolink trail followed by a script closing tag and paragraph", () => {
    assertMdastConformance(
      '3[:[a@^o~r(<script type="text/javascript">\n// JavaScript example\n\ndocument.getElementById("demo").innerHTML = "Hello Jav__a<https://foo.bar/?aScript!";\n</script>\nokay\n',
    );
  });
});

describe("MDX JSX namespace allows whitespace around `:`", () => {
  test("`<a :b/>` opens flow JSX with name `a:b`", async () => {
    const { satteriMdxMdast, referenceMdxMdast } = await import("./fuzz/shared.js");
    expect(satteriMdxMdast("<a :b/>")).toEqual(referenceMdxMdast("<a :b/>"));
  });

  test("`b<acz :circle/>` inline JSX keeps the namespace", async () => {
    const { satteriMdxMdast, referenceMdxMdast } = await import("./fuzz/shared.js");
    expect(satteriMdxMdast("b<acz :circle/>")).toEqual(referenceMdxMdast("b<acz :circle/>"));
  });
});

describe("GFM autolink fires during inline tokenization, not as post-pass", () => {
  test("[a](https://x[![alt](url) → trailing literal autolink", () => {
    assertMdastConformance("[a](https://x[![alt](url)");
  });

  test("multi-link soup with paragraphs", () => {
    assertMdastConformance(
      "*r{}>4bnk](#fragment)\n\n[link](https://exa[![moon](mmple.com#fragment)\n\n[link](https://example.com?foo=3#frag)\n",
    );
  });

  test("`\\<http://foo.bar.baz>` fires literal autolink (escaped `<`)", () => {
    assertMdastConformance("\\<http://foo.bar.baz>\n");
  });

  test("URL ending in `\\` before `\\n` is not a hardbreak marker", () => {
    assertMdastConformance("_<https://foo.bar/bazfoo\\\nb bim>\n");
  });

  test("email preceded by Cyrillic letter is rejected by FNR", () => {
    assertMdastConformance("пo\\+@bar.example.com>\n");
  });
});

describe("email autolink after a backslash escape", () => {
  test.fails("escaped backslash before a non-alphanumeric local part", () => {
    assertMdastConformance("\\\\+@.a\n");
  });

  test.fails("escaped `+` still starts an email in satteri", () => {
    assertMdastConformance("\\+_@.a\n");
  });

  test.fails("escaped backslash after a soft break before an email", () => {
    assertMdastConformance("text\n\\\\+@.9a);\n");
  });
});

describe("Markdown structural divergences", () => {
  test("list/bq/code-fence/autolink cascade", () => {
    assertMdastConformance(
      "*>ss)1.  foo\n\n    ```\n <https://ex   bar\n    ```\n\n    baz\n\n    > bam\n",
    );
  });

  test.fails("link label with backslash-newline hard break", () => {
    assertMdastConformance("[\\\n](3*foo)\n");
  });

  test("code span pairing with autolink-like body", () => {
    assertMdastConformance("`[)4p$[g`https://foo.bar.`baz>`\n");
  });

  test.fails("4-tick fence in listitem, then root setext", () => {
    assertMdastConformance("* !!f~````\naaa\n```\n``````\n    Foo\n    ---\n\n    Foo\n---");
  });

  test.fails("multi-line refdef label inside listitem resolves outer reference", () => {
    assertMdastConformance("* [Foo\n  bar]: /url\n\n[Baz][Foo bar]\n");
  });

  test.fails("link label with hard break (variant)", () => {
    assertMdastConformance("\nd[37r\\\n]()\n");
  });
});

describe("MDX structural divergences", () => {
  test.fails("code span body that contains autolink-like `<…>` errors in mdx", async () => {
    const { satteriMdxMdast, referenceMdxMdast } = await import("./fuzz/shared.js");
    expect(satteriMdxMdast("l`{yk[[=\t\n<https://foo.bar.`baz>`\n")).toEqual(
      referenceMdxMdast("l`{yk[[=\t\n<https://foo.bar.`baz>`\n"),
    );
  });

  test.fails("empty bullet then indent-stepped ordered items stay siblings", async () => {
    const { satteriMdxMdast, referenceMdxMdast } = await import("./fuzz/shared.js");
    expect(satteriMdxMdast("-\n\n  2. b\n\n    3. c\n")).toEqual(
      referenceMdxMdast("-\n\n  2. b\n\n    3. c\n"),
    );
  });
});

describe("inline code after a link destination with CJK trail", () => {
  test("link + CJK + inline code", () => {
    assertMdastConformance("使用 [link](http://x)，即运行`cmd`。");
    assertHastConformance("使用 [link](http://x)，即运行`cmd`。");
  });

  test("link + CJK comma + inline code (no trailing text)", () => {
    assertMdastConformance("使用 [link](http://x)，`cmd`");
  });

  test("link + backtick directly adjacent (leading CJK only)", () => {
    assertMdastConformance("使用 [link](http://x)`cmd`");
  });

  test("image (not link) followed by CJK + inline code", () => {
    assertMdastConformance("使用 ![alt](http://x)，即运行`cmd`。");
  });

  test("nested parens in link destination still skipped", () => {
    assertMdastConformance("[a](http://x(y)z)，即运行`cmd`。");
  });

  test("bare autolink before CJK still consumes CJK as URL (unchanged)", () => {
    assertMdastConformance("use http://x，foo");
  });
});

describe("the slice invariant across every line ending", () => {
  const cases = [
    "{\r a",
    "a\r\t$",
    "#-\r\n\\$",
    "q\r [Foo]\n[foo]: /url\n> bar\n",
    "p\r\tfoo\tbaz\t\tbim\n",
  ];
  for (const input of cases) {
    test(`positions decode back for ${JSON.stringify(input)}`, () => {
      assertSliceInvariantEverywhere(satteriMdast(input), input);
    });
  }
});
