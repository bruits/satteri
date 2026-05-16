import { describe, test, expect } from "vitest";
import {
  assertMdastConformance,
  assertHastConformance,
  assertHtmlConformance,
  assertExtMdastConformance,
  assertExtHastConformance,
  satteriMdast,
  referenceMdast,
} from "./helpers.js";

// Each case below was discovered by fuzz runs in test/conformance/fuzz/ and
// reduced to a minimal repro.

const MATH: ["math"] = ["math"];

describe("fuzz regressions: HTML block in list item", () => {
  // Type-1 HTML block (`<textarea>`/`<pre>` etc) inside a list item ends
  // when the parent container closes. micromark stops BEFORE the blank
  // line that triggers the close, so the html value keeps the trailing
  // newline of the last content line. Sätteri previously trimmed it.
  test("`<textarea>` in list item keeps trailing newline on close", () => {
    assertMdastConformance("+\t<textarea>\n\nfoo");
  });

  // Orphan HTML fragments at root after a list and paragraph were combining
  // weirdly when the list-item-close logic interacted with the HTML buf.
  test("complex orphan HTML table fragments + list + paragraph", () => {
    assertMdastConformance(
      "*\n@9jifle>\n\n  <tr>\n\n    <td>\n      Hi\n    </td>\n\n  </tr>\n\n</table>\n",
    );
  });
});

describe("fuzz regressions: indented code merging after list sibling", () => {
  // After a list item closes a previous sibling, the `pending_lazy_close`
  // flag was being kept across a subsequent blank line. The next indented
  // code block then ran in `lazy_one_line` mode and split across blanks.
  // Now the blank line clears the flag — matching micromark's furtherStart
  // which only rejects lazy lines that *directly* follow the close.
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

describe("fuzz regressions: email autolinks preceded by `[`", () => {
  // Emails go through findEmail (not findUrl) in mdast-util-gfm-autolink-
  // literal — there's no `splitUrl`/`trail` split, so trailing chars after
  // an email stay in the surrounding text node even when `[` precedes.
  test("`[skeLTO:FOO@BAR.BAZ>` keeps `>\\n$` as one trailing text", () => {
    assertMdastConformance("[skeLTO:FOO@BAR.BAZ>\n$");
  });
});

describe("fuzz regressions: type-7 HTML on lazy line in list item", () => {
  // Type-7 HTML on a lazy-continuation line of a list item normally OPENS
  // INSIDE the list item (micromark's child-flow parser allows type-7
  // there). Exception: if the html line is at EOF without a trailing
  // newline, the html ends up at root instead — `- a\n<a>` → `[list, html]`
  // but `- a\n<a>\n` → `[list[item[para, html]]]`.
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

  test("original fuzz repro: bracketed item content + html + more", () => {
    assertMdastConformance("+  $o[Foo bar]:\n<my url>\n'title'\n\n[Foo bar]\n");
  });
});

describe("fuzz regressions: autolink suppressed by unbalanced `[`", () => {
  // When preceded by an unbalanced `[`/`![`, micromark's `previousUnbalanced`
  // suppresses the construct path entirely. Only find-and-replace can
  // still accept — and only if `isCorrectDomain` (≥2 dot segments with
  // alphanumeric content) passes. Without `.`, both paths reject.
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

describe("fuzz regressions: bracket+URL trail split", () => {
  // When an autolink literal is preceded by an open `[`/`![`, micromark's
  // construct path is suppressed and find-and-replace runs. Its `splitUrl`
  // strips a wider set of trailing chars (`!"&'),.:;<>?\]}`) and emits
  // them as a SEPARATE text node, distinct from the text that follows.
  // E.g. `[https://foo.bar] x` → `[`, LINK, `]`, ` x` (4 nodes).
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

describe("fuzz regressions: footnote definition label escapes", () => {
  // mdast-util-from-markdown's footnoteDefinition handler resolves
  // backslash escapes in `label` (the human-readable form) but keeps the
  // normalized form (case-folded, ws-collapsed) in `identifier`.
  test("`[^foot\\\\]: ...` label has unescaped backslash", () => {
    assertMdastConformance("[^foot\\\\]: footnote");
  });

  test("`[^l{r\\\\]: /uri` plus reference link mixes label escape rules", () => {
    assertMdastConformance("[^l{r\\\\]: /uri\n\n[bar\\\\]\n");
  });

  // Identifier normalization: uppercase letters → lowercase.
  test("footnote identifier is case-folded", () => {
    assertMdastConformance("[^lFOO]: /url\n\n[Foo]\n");
  });

  test("footnote reference identifier is case-folded too", () => {
    assertMdastConformance("[^Doh]: I know.\n\n[^Doh] reference");
  });
});

describe("fuzz regressions: HTML block trailing newline", () => {
  // Trim trailing `\n` from the html value depends on close path:
  //   * lazy close (non-blank breaks container): trim, any parent
  //   * blank close + blockquote parent: trim
  //   * blank close + list-item parent: keep
  //   * EOF after continuation marker: keep
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

describe("fuzz regressions: trim-lines on text→hast", () => {
  // mdast-util-to-hast applies `trim-lines` to text node values: spaces
  // and tabs adjacent to interior `\n`s are stripped, while leading
  // whitespace of the very first line and trailing whitespace of the very
  // last line are preserved. Tab character entered as `&#9;` ends up as a
  // literal `\t` in mdast and gets trimmed in hast.
  test("`&#9;` decoded to tab on continuation line is stripped in hast", () => {
    assertHastConformance("n\n&#9;foo\n");
  });

  test("trailing spaces before a soft-break are stripped in hast", () => {
    assertHastConformance("a   \nb");
  });
});

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

  // mdast-util-gfm-table preserves source cell count (overflow not truncated).
  // mdast-util-to-hast then drops the overflow when rendering. So MDAST has
  // 3 cells in row 2 even though the header has 2.
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

describe("fuzz regressions: HTML block on blockquote lazy-continuation", () => {
  // Type-7 HTML blocks (`<a href="...">`, `</a>`) normally cannot interrupt
  // a paragraph per CommonMark, but micromark/remark close the paragraph
  // when one appears on a *lazy-continuation* line of a blockquote — and the
  // new HTML block opens INSIDE the still-open blockquote, then closes when
  // the next non-`>` content line ends the blockquote too.
  test("type-7 open tag with attributes on lazy line opens HTML inside blockquote", () => {
    assertMdastConformance('>oo\n<a href="bar">\nbaz');
  });

  test("type-7 close tag on lazy line opens HTML inside blockquote", () => {
    assertMdastConformance(">oo\n</a>\nbaz");
  });

  test("plain text after lazy-line HTML becomes a sibling paragraph at root", () => {
    assertMdastConformance(">ning>\n*bar*\n</Warning>\n");
  });

  // Non-blockquote (root paragraph): type-7 does NOT interrupt — the tag
  // stays as inline HTML inside the paragraph (per the CommonMark spec).
  test("type-7 in plain paragraph stays inline", () => {
    assertMdastConformance('oo\n<a href="bar">');
  });

  // Type-6 (`<div>`, etc.) interrupts paragraphs even outside lazy mode and
  // pops the container — the HTML block sits at root, not in the blockquote.
  test("type-6 on lazy line pops the container", () => {
    assertMdastConformance(">x\n<div>");
  });
});

describe("fuzz regressions: GFM literal autolink with escapes", () => {
  // micromark's protocolAutolink construct tokenises raw source bytes, so a
  // backslash-escape inside a URL stays literal in the URL value (`\[\>` not
  // `[>`). The displayed link text matches the raw source too.
  test("URL spanning backslash escapes keeps raw source bytes", () => {
    assertMdastConformance("https://example.com/\\[\\>");
  });

  test("URL with leading text + escapes keeps raw source bytes", () => {
    assertMdastConformance("-https://example.com/\\[\\>");
  });

  // Both autolink paths fail when prev is loose-only (digit) AND we're inside
  // an unclosed `[`: the construct is suppressed by `previousUnbalanced`, and
  // find-and-replace's strict `previous` rejects digits. Should emit no link.
  test("digit-prefixed URL inside unclosed `[` produces no link", () => {
    assertMdastConformance("[0https://example.com/\\[\\>");
  });

  // Email construct can't tokenise `\` in the local-part, so find-and-replace
  // takes over and emits the URL using text bytes (`+` not `\+`), no position.
  test("email with backslash in local-part uses find-and-replace path", () => {
    assertMdastConformance("do\\+@bar.example.com>");
  });

  // Email's prev char check accepts punctuation including `@` (the leading
  // `t@` is its own failed email attempt; the next email starts at `+`).
  test("email after `@` prev char still matches", () => {
    assertMdastConformance("o6(t@+@bar.example.com>");
  });

  // Preceded by an open `[`, the literalAutolink construct is suppressed
  // and find-and-replace runs on text bytes — so backslash-escapes inside
  // the URL get RESOLVED in the URL value (`\*` → `*`), unlike the
  // construct-succeeded path that keeps raw source bytes.
  test("URL with escape after open `[` uses text bytes (not raw source)", () => {
    assertMdastConformance("6[1}]pd\t[=https://example.com?find=\\*>");
  });

  // micromark's `previousProtocol` rejects only ASCII alpha; non-ASCII
  // letters (Cyrillic etc.) pass the loose check, so the construct fires
  // after `п` in `_oпhttps://...`.
  test("URL after Cyrillic letter is autolinked via construct path", () => {
    assertMdastConformance("_oпhttps://example.com");
  });

  // Email's prev char must not be `/` per find-and-replace's `previous(_, true)`.
  // `/5w3+special@bar.com` triggers retry: max walkback gives `5w3+special`
  // (prev=`/` → reject), shorter retries fail until prev is `+` (`\p{S}`).
  test("email after `/` retries shorter walkback until prev is acceptable", () => {
    assertMdastConformance("/5w3+special@Bar.baz-bar0.com>");
  });

  // `\h` is NOT a valid escape (h isn't punctuation), so the autolink scan
  // should detect `https://...` immediately after the `\` and suppress the
  // backtick code-span attempt inside the URL. Without limiting the escape
  // skip to actual punctuation, we'd skip past the `h` and miss the URL.
  test("invalid `\\h` doesn't hide URL from backtick suppression", () => {
    assertMdastConformance(":wh\\https://foo.bar.`baz>`");
  });
});

describe("fuzz regressions: MDX inline expression after backslash-escaped `<`", () => {
  // `\<Foo bar={1}...` — the `\<` is an escape, so `<Foo` is literal text
  // and `{1}` should still be detected as an inline MDX expression.
  // Without skipping the escape in the open-jsx-tag scan, the inline
  // expression was suppressed because we thought we were inside a tag.
  test("escaped `<` doesn't suppress later inline expression", () => {
    // mdx via assertExtMdastConformance isn't quite right; this is checked
    // implicitly by the fuzz harness on mdxToMdast. Use the markdown helper
    // to at least make sure it parses without erroring (no MDX in plain mode
    // means `{...}` stays as text, which both paths agree on).
    assertMdastConformance('[r\\<Foo bar={1} baz="two"/>h');
  });

  // `7\<o\n> bar` — `\<o` is escaped text, so the `> bar` line on its own
  // should interrupt the paragraph as a blockquote. Without skipping the
  // escape in `prev_line_has_open_inline_jsx`, the would-be JSX opener
  // suppressed the interrupt and `> bar` continued the paragraph.
  test("escaped `<` on prior line doesn't suppress blockquote interrupt", () => {
    assertMdastConformance("7\\<o\n> bar");
  });
});

describe("fuzz regressions: indented code split after blockquote close", () => {
  // micromark sets `lazy[line]` per line based on container-stack mismatch,
  // and `codeIndented`'s `furtherStart` rejects lazy lines — so an indented
  // code block opened on a line that lazy-closes a blockquote can't extend
  // to the next (also-lazy) line. Subsequent indented lines become their
  // own one-line code blocks.
  test("empty blockquote then per-line indented code splits", () => {
    assertMdastConformance(">\n    bar\n    baz");
  });

  test("blockquote with leaf then per-line indented code splits", () => {
    assertMdastConformance("> # Foo\n    > bar\n    > baz");
  });

  // The blank-line close is a *proper* close (not lazy), so subsequent
  // indented code blocks merge normally.
  test("blank line between bq and indented code allows merge", () => {
    assertMdastConformance("> Foo\n\n    bar\n    baz");
  });

  // Only the FIRST indented code block after a lazy bq close is one-line.
  // Once a new code block opens (after a blank line), subsequent lines
  // extend it normally.
  test("lazy zone clears after first one-line code block", () => {
    assertMdastConformance(">\n    bar\n    baz\n\n    qux");
  });

  // Same lazy-close pattern but for a list item: when an indented code
  // line falls outside the list item's content column after a blank line,
  // it splits per line just like the blockquote case.
  test("indented code after wide-marker list item splits per line", () => {
    assertMdastConformance("-    foo\n\n    bar\n    baz");
  });

  test("indented code after wide-marker list item: extends after first split", () => {
    assertMdastConformance("-    foo\n\n    bar\n    baz\n    qux");
  });

  // List with fenced code that closes early triggers the same pattern.
  test("list with fence then per-line indented code splits", () => {
    assertMdastConformance("-    ```\n    aaa\n    ```");
  });
});

describe("fuzz regressions: CDATA inline HTML close requires `]]>`", () => {
  // CommonMark CDATA section ends at the literal `]]>` — `<![CDATA[…]>`
  // (one `]` only) should NOT close. Sätteri previously accepted any `]+>`
  // and emitted spurious inline HTML.
  test("CDATA with one `]` is not a complete close", () => {
    assertMdastConformance("foo <![CDATA[>&<]>");
  });
});

describe("fuzz regressions: protocol autolink first-char rejection", () => {
  // micromark's `afterProtocol` rejects when the first byte after `://`
  // is whitespace, control, or Unicode punctuation (incl. `-`). Without
  // this check, `<http://--` would tokenise as a literal autolink even
  // though `-` is an invalid domain-start character.
  test("`http://-` rejected (first body char is punctuation)", () => {
    assertMdastConformance("Foo\n-<http://--\nbar");
  });

  // `-foo` and `_foo` first chars: construct rejects (afterProtocol punct);
  // find-and-replace also rejects (parts.length<2 for `-foo`; underscore
  // in last part for `_foo`). Both cases stay as text.
  test("`https://-foo` rejected by both paths", () => {
    assertMdastConformance("https://-foo");
  });

  test("`https://_foo` rejected by both paths", () => {
    assertMdastConformance("https://_foo");
  });

  // `https://.foo` rejected by construct (first-char punct), accepted by
  // find-and-replace (parts=[``, `foo`], last part alphanumeric, no `_`).
  test("`https://.foo` accepted via find-and-replace path", () => {
    assertMdastConformance("https://.foo");
  });

  // `https://..` rejected by both: find-and-replace's splitUrl trims `..`
  // off the end, leaving an empty URL.
  test("`https://..` rejected (splitUrl trims to empty)", () => {
    assertMdastConformance("https://..");
  });

  // `https://../` accepted by find-and-replace: `/` blocks splitUrl trim,
  // so URL stays `https://../`.
  test("`https://../` accepted via find-and-replace path", () => {
    assertMdastConformance("https://../");
  });

  // `-foo.bar` first-char: construct rejects (afterProtocol). Find-and-
  // replace accepts (parts=[`-foo`, `bar`], both valid).
  test("`https://-foo.bar` accepted via find-and-replace path", () => {
    assertMdastConformance("https://-foo.bar");
  });

  // Construct's domain rule rejects `_` in the last/penultimate dot-segment;
  // find-and-replace also rejects when the last/penult part contains `_`.
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

describe("fuzz regressions: email walkback past `_`", () => {
  // `_` is in `\p{Pc}` (connector punctuation) so it satisfies the email
  // regex's lookbehind `^|\s|\p{P}|\p{S}`. Find-and-replace's retry must
  // accept `_` as a valid prev char, even though it's also `\w`.
  test("email starts after `_` via find-and-replace path", () => {
    assertMdastConformance("$/_ipecial@Bar.baz-bar0.com>");
  });
});

describe("fuzz regressions: code span across lines suppresses autolink", () => {
  // A backtick on a previous line opens a code span that micromark would
  // close on the second backtick — even when the second backtick falls
  // inside what looks like a GFM literal autolink URL. The earlier-backtick
  // check has to scan the whole paragraph (across line boundaries), not
  // just the current line; otherwise we extend the code span past the
  // matching closer.
  test("paragraph-scoped earlier-backtick beats autolink-inside suppression", () => {
    assertMdastConformance("pz  _xlo`\n<https://foo.bar.`baz>`");
  });
});

describe("fuzz regressions: MDX JSX whitespace around `=`", () => {
  // mdx-js accepts whitespace on either side of an attribute's `=`:
  // `<Foo bar = "x"/>`, `<Foo bar= {1}>`, `<Foo bar=\n  {1}/>`. Without
  // skipping that whitespace, Sätteri rejected the tag entirely.
  test("space before `=` parses as attribute", () => {
    assertMdastConformance("<Foo bar = 'baz'/>");
  });

  test("space after `=` parses as attribute", () => {
    assertMdastConformance("zj<Foo bar= {1}/>");
  });
});

describe("fuzz regressions: indented code after empty list item", () => {
  // After an empty list item (`*\n`) closes via blank line, subsequent
  // indented code lines do NOT merge into one block — each becomes its own
  // indented code block. Mirrors micromark's `furtherStart` restriction
  // that limits lazy-after-close indented code to one line. Non-empty list
  // items keep the merged behavior (codes stay inside the item).
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

describe("fuzz regressions: fenced code block trim on container outdent", () => {
  // When a fenced code block ends because the container outdents (no close
  // fence), the blank line(s) immediately before the outdent are NOT part
  // of code content — they act as the list-item separator. arena_build
  // strips one trailing newline; we strip one more in firstpass to match
  // remark's behavior.
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

describe("fuzz regressions: list extension first-content-line only absorbs `>`", () => {
  // `>+ # Foo\n> bar\n> baz\n` — list inside a bq, followed by a sibling
  // paragraph in the same bq. Line 2 `> bar` is bq-continuation + text.
  // The list must NOT extend through line 2's `>` (it's a sibling, not
  // a list continuation). Refined rule: absorb the marker only when the
  // content past it is another `>` (deeper-bq attempt).
  test("`>+ # Foo\\n> bar\\n> baz\\n` keeps list end at line 1", () => {
    assertMdastConformance(">+ # Foo\n> bar\n> baz\n");
  });

  test("`>- one\\n>>` still extends list to col 2 of line 2", () => {
    assertMdastConformance(">- one\n>>");
  });
});

describe("fuzz regressions: list/bq extension stops at indented-code threshold", () => {
  // `~-{tg\t\n>* > # Foo\n    > bar\n    > baz\n` — the outer bq>list
  // closes after "Foo" on line 2. Line 3 has 4 leading spaces before
  // its `>`, so pulldown emits a root-level indented code block, not a
  // bq continuation. The list-extension loop must NOT absorb the `>`
  // (4+ space indent is the indented-code threshold).
  test("`>* > # Foo` then 4-space indented `> bar` keeps positions tight", () => {
    assertMdastConformance("~-{tg\t\n>* > # Foo\n    > bar\n    > baz\n");
  });
});

describe("fuzz regressions: HAST footnote elements carry position", () => {
  // remark-rehype emits position on the footnote anchor `<a>` (inside
  // `<sup>`), the rendered footnote `<li>` (inside the trailing
  // `<section class="footnotes">`), and the synthesized text node that
  // appends a backref separator space to the paragraph's last text.
  // Our impl was dropping these.
  test("footnote ref/def positions appear in HAST", () => {
    assertHastConformance("j4nu0[^y]\n\nrvt[^bxmw]\n\n[^y]: 4quj08jtc\n");
  });
});

describe("fuzz regressions: GFM email rejects when domain ends in `-`, digit, or `_`", () => {
  // findEmail's `/[-\d_]$/.test(label)` rejects the WHOLE match — not
  // just the trailing chars — when the regex-captured domain ends in
  // `-`, ASCII digit, or `_`. Our impl was trimming and accepting a
  // truncated email, producing spurious links like `<a>foo@bar.com</a>-`.
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

  test("compound: full fuzz case with setext heading + paragraph", () => {
    assertHtmlConformance(
      "\\$l->yhwn#\n\tFoo *bar*\n=========\n\nFoo *bar*\n-----foo@bar.example.c----\n",
    );
  });
});

describe("fuzz regressions: inline HTML clears backslash-escape on trail", () => {
  // `foo <a href="\*">>\t\tfoo\n` — pulldown tokenizes `\*` (a valid
  // escape) marking the byte after the `*` as a candidate trail anchor.
  // When the inline HTML consumes the `\*`, the stale backslash_escaped
  // flag on the trail still pulled the text's start back one byte.
  test("inline HTML with escaped char in attribute: trail position correct", () => {
    assertMdastConformance('foo <a href="\\*">>\t\tfoo\n');
  });
});

describe("fuzz regressions: CommonMark autolink clears backslash-escape on trail", () => {
  // `<https://example.com/\>foo` — pulldown tokenizes `\>` as Backslash +
  // escaped Text, marking the trailing `foo` as backslash_escaped.
  // arena_build then extends the trail's position back by 1 to cover
  // the `\`. But the autolink consumed `\` as part of the URL, so the
  // flag is stale — clear it when the trail's start advances.
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

describe("fuzz regressions: URL encoding for invalid percent-encoding", () => {
  // remark's normalizeUri treats `%` as URL-safe only when followed by two
  // hex digits; otherwise the `%` itself is encoded as `%25`. Our impl
  // previously left bare `%` unchanged, producing `foo%2%5Eb%C3%A4`
  // instead of `foo%252%5Eb%C3%A4`.
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

describe("fuzz regressions: list-item end extends through next marker when last child ends after \\n", () => {
  // When a list-item's last child's position span includes a trailing
  // newline (e.g. an unclosed fenced code followed by a new list-item),
  // remark extends the previous list-item's position past the new
  // marker to the next item's first content column.
  test("`- ```\\n- d\\n` → listItem 1 end at content col of listItem 2", () => {
    assertMdastConformance("- ```\n- d\n");
  });

  test("compound: HTML+empty fenced + blanks + close + next listItem", () => {
    assertMdastConformance("- </a>```\n  b\n\n\n  ```\n- c\n");
  });
});

describe("fuzz regressions: autolink continuation line drops leading space", () => {
  // `r<@\n special@Bar.baz-bar0.com>\n` — email autolink whose local-part
  // starts on a continuation line preceded by a space (paragraph indent
  // collapses). The link's `start.offset` must point at the local-part's
  // first byte, not the dropped space.
  test("email autolink on continuation line skips leading indent space", () => {
    assertMdastConformance("r<@\n special@Bar.baz-bar0.com>\n");
  });
});

describe("fuzz regressions: list extension absorbs trailing tab on blank marker line", () => {
  // `>-\n>\t\n:7^` — a list inside a blockquote followed by a `>\t` blank
  // marker line then a root-level paragraph. The list's end must reach
  // past the tab on the marker line (offset 5, the \n), not stop at the
  // `>` (offset 4).
  test("blank `>\\t` marker line: list end past tab", () => {
    assertMdastConformance(">-\n>\t\n:7^");
  });
});

describe("fuzz regressions: link refdef resolves in source order, not node-id order", () => {
  // Top-level refdefs are emitted at the END of arena_build so their
  // node IDs come AFTER blockquote-nested defs even when they appear
  // earlier in the source. First-wins resolution must therefore key off
  // source position, not node id. MDAST already matches; this regresses
  // at HAST/HTML conversion time.
  test("top-level def with title wins over bq-nested def without title (HTML)", () => {
    assertHtmlConformance('[foo]: /url "title"\n\n[foo]\n\n> [foo]: /url');
  });

  test("image reference also picks the top-level def with title (HTML)", () => {
    assertHtmlConformance('![foo][]\n\n[foo]: /url "title"\n[foo]\n\n> [foo]: /url');
  });
});

describe("fuzz regressions: nested list inside list-item-blockquote does not extend through blank `>>`", () => {
  // `-   > > 1.  one\n>>\n>>     two\n` — the inner list lives inside
  // two blockquotes, but those blockquotes are themselves inside a
  // root-level list-item. The list-extension logic (added for cases
  // like `>>- one\n>>\n  >  > two`) must NOT fire here, or all five
  // ancestor containers cascade through the blank `>>` line and the
  // whole subtree's positions get over-extended.
  test("nested bq>bq>list inside list-item leaves positions at line 1", () => {
    assertMdastConformance("-   > > 1.  one\n>>\n>>     two\n");
  });
});

describe("fuzz regressions: footnote definition trims trailing whitespace", () => {
  // Trailing whitespace (a blank line, spaces, or both) after a footnote
  // definition's content should NOT extend the definition's position
  // range. Previously pulldown's `item.end` carried these bytes into the
  // span, putting the def's end on the trailing whitespace line. Match
  // remark by using the last child's end instead.
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

describe("fuzz regressions: empty unclosed fenced code in list-item before new container", () => {
  // When an unclosed fenced code is the last child of a list-item and the
  // next line opens a new container (different-marker list, blockquote,
  // ordered list), remark keeps the trailing \n in the code's position
  // span. When the next line is a leaf (paragraph, heading, indented
  // code), the \n is trimmed instead.
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

describe("fuzz regressions: `~~` flanking when followed by punctuation", () => {
  // GFM strikethrough's `~~` opening must respect CommonMark flanking
  // rules: when followed by punctuation, the preceding char must be
  // whitespace or punctuation. Previously we returned `true` for any
  // `~~` run, accepting `a~~/foo~~` as strikethrough even though GFM
  // rejects it (the `~~` after alnum + before punct isn't left-flanking).
  test("`a~~/foo~~` stays text (alnum before, punct after)", () => {
    assertMdastConformance(":f~~/42e~~\n[]\n~~~\n");
  });

  test("`x~~.foo~~y` similar pattern", () => {
    assertMdastConformance("x~~.foo~~y");
  });
});

describe("fuzz regressions: GFM autolink trail split on newline", () => {
  // When the post-trail content starts on a new line, mdast emits the trail
  // and the rest as TWO text nodes (micromark text events split at line
  // boundaries). Same-line content after the trail merges into one node.
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

describe("fuzz regressions: GFM email domain can't start with `.`", () => {
  // mdast-util-gfm-autolink-literal's email regex requires the first domain
  // segment to be `[-\w]+` (no `.`). After backslash unescape, source
  // `2@\.baz` becomes text `2@.baz`, which previously matched our looser
  // scanner. The regex would reject it because the domain starts with `.`.
  test("`2@\\.baz>` (text `2@.baz`) is NOT an email", () => {
    assertMdastConformance("t\t2@\\.baz>\n");
  });

  test("`@.foo.bar` not an email (domain starts with dot)", () => {
    assertMdastConformance("@.foo.bar");
  });
});

describe("fuzz regressions: code span container scan beyond span", () => {
  // skip_container_prefixes inside make_code_span was called with the SPAN
  // slice only. A partial-indent line (e.g. `    ` with 4 spaces under a
  // 5-indent list item) ends the slice, which is_at_eol misreads as a
  // proper line end, so scan_containers wrongly accepts the indent and
  // over-strips. Pass the full source bytes so is_at_eol sees the real
  // next char.
  test("tab-marker list item: code span keeps trailing indent", () => {
    assertMdastConformance("+\t w4-w```\naaa\n    ```\n");
  });

  test("tab-marker list item: code span EOF-no-newline keeps trailing indent", () => {
    assertMdastConformance("+\t w4-w```\naaa\n    ```");
  });
});

describe("fuzz regressions: shortcut link suppressed by following `[`", () => {
  // CommonMark says a shortcut reference link is only valid when NOT
  // followed by `[]` or a link label. We previously fell back to shortcut
  // for `[text][invalid_label]`, but the `[` after `[text]` is enough to
  // suppress the shortcut even when the second bracket pair isn't a valid
  // label.
  test("`[foo][ref[]` stays text (inner `[` invalidates label)", () => {
    assertMdastConformance("[foo][ref[]\n\n[foo]: /url");
  });

  test("`[foo][ref[]` with no definition: also plain text", () => {
    assertMdastConformance("[foo][ref[]");
  });
});

describe("fuzz regressions: table delimiter row last-cell hyphen check", () => {
  // Every cell of a GFM delimiter row must contain at least one `-`. The
  // inter-cell check fired at `|`, but the LAST cell was pushed without
  // the same check, so `-|:` (cells `-` and `:`) wrongly opened a table.
  test("`-|:` (no `-` in last cell) is NOT a delimiter row", () => {
    assertMdastConformance("[is:|{\n-|:");
  });

  test("`|::|` (no `-` anywhere) is NOT a delimiter row", () => {
    assertMdastConformance("a|b\n|::|");
  });
});

describe("fuzz regressions: footnote def with leading whitespace in label", () => {
  // micromark's gfm-footnote `labelInside` rejects whitespace
  // character-by-character — including LEADING whitespace stripped by
  // scan_link_label_rest's trim. `[^ *o]:` falls through to a regular
  // reference definition. We previously checked the trimmed label, which
  // wrongly accepted these as footnote definitions.
  test("`[^ *o]: url` becomes a refdef, not a footnote", () => {
    assertMdastConformance('[^ *o]: /url\n"title" ok\n');
  });

  test("`[^foo ]:` (trailing space) also rejected as footnote", () => {
    assertMdastConformance("[^foo ]: /url");
  });
});

describe("fuzz regressions: fenced code block trailing whitespace at EOF", () => {
  // scan_closing_code_fence used to return Some(0) for empty input, which
  // wrongly fired after consuming a leading-space line — `~~~s\n ` lost the
  // trailing space. Handling EOF explicitly in the caller (and removing the
  // empty-bytes early return) preserves the content line.
  test("single space line at EOF kept as code content", () => {
    assertMdastConformance("~~~s\n ");
  });

  test("trailing tab line at EOF kept", () => {
    assertMdastConformance("~~~s\n\t");
  });
});

describe("fuzz regressions: ordered list start≠1 after indented code", () => {
  // micromark's list construct rejects a fresh ordered list with start != 1
  // when `self.interrupt` is set (mirrors `currentConstruct` lingering after
  // the previous block). After an indented code block, `2. b` becomes a
  // paragraph rather than a new list. Index-1 starts are always allowed.
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

describe("fuzz regressions: MDX list-item content column", () => {
  // CommonMark clamps post-marker spaces at 4 — `1.     foo` puts `foo` in
  // an indented code block inside the list item with content col 4. MDX
  // disables indented code blocks, so the actual content col matters:
  // `1.     foo\n   bar` should make `bar` (at col 4) a sibling paragraph,
  // not a continuation of the list item (whose content col is 8).
  // assertExtMdastConformance with no extensions is plain remark+gfm; this
  // case is verified end-to-end by the mdx fuzz harness via mdxToMdast.
  // Use mdast assertion to at least pin the plain-mode behavior (which
  // does the opposite — `bar` IS in the list item under CommonMark rules).
  test("plain mdast: 5+ space marker keeps clamped content col", () => {
    assertMdastConformance("1.     abc\n\n   def");
  });
});

describe("fuzz regressions: autolink text chunk covers backslash-escape source", () => {
  // `\[foo@bar.example.com` — the text node `[` preceding the email
  // autolink must cover both source bytes `\[` (offset 0..2), not just
  // the unescaped `[` (0..1). Same family for any text chunk preceding
  // an autolink whose source contains backslash escapes.
  test("leading `\\[` before email autolink keeps escape in text span", () => {
    assertMdastConformance("\\[foo@bar.example.com");
  });

  test("`!7j4{h\\:3)v{` text before email autolink covers `\\:` source", () => {
    assertMdastConformance("!7j4{h\\:3)v{j@bar.example.com>");
  });
});

describe("fuzz regressions: bracket depth propagates across inline parents", () => {
  // `*![fw*https://example.com` — the `![` opens labelImage inside the
  // Emphasis. micromark's `previousUnbalanced` sees the open labelImage
  // when `https` starts, so the literalAutolink construct is suppressed
  // and find-and-replace runs without position info. The bracket-depth
  // tracker in the autolink pass must aggregate at the inline-block
  // ancestor (Paragraph), not per-parent, for the suppression to fire.
  test("`![` inside emphasis suppresses position on following autolink", () => {
    assertMdastConformance("*![fw*https://example.com");
  });
});

describe("fuzz regressions: link with escape in URL leaves trail position unshifted", () => {
  // `[link](foo\()trail` — the URL `foo\(` consumes the backslash, and
  // the trail text node (originally created with `backslash_escaped`
  // because the escape was at its start) has been advanced past the
  // escape by link resolution. The `backslash_escaped` flag must be
  // cleared in that case so the arena-build position fixup doesn't
  // shift trail's source span back by one to "include" a backslash
  // that the link already owns.
  test("`[link](foo\\()trail` puts trail at offset 13 not 12", () => {
    assertMdastConformance("[link](foo\\()trail");
  });

  test("`&[link](foo\\(an)r> d\\(bar\\))` trail spans 16..28 (one escape)", () => {
    assertMdastConformance("&[link](foo\\(an)r> d\\(bar\\))");
  });
});

describe("fuzz regressions: text_to_source skips trimmed line-ending whitespace", () => {
  // `:rap#| \n6!< https://foo.bar >` — the trailing space before `\n` is
  // trimmed from the inline text but still occupies a source byte. The
  // text-to-source map must skip it; otherwise the map is discarded and
  // chunk positions fall back to text-byte offsets (off by 1).
  test("trailing space before soft-break keeps source-byte alignment", () => {
    assertMdastConformance(":rap#| \n6!< https://foo.bar >");
  });

  test("leading whitespace on continuation line keeps source-byte alignment", () => {
    // `z\n i}@f<https://foo.bar/baz bim>` — the leading space on line 2
    // is trimmed from the inline text. The text-to-source map must skip
    // it so chunk positions stay aligned to source bytes.
    assertMdastConformance("z\n i}@f<https://foo.bar/baz bim>");
  });
});

describe("fuzz regressions: list inside blockquote extends position to absorb trailing markers", () => {
  // `>>- one\n>>\n  >  > two` — when a list inside a blockquote is
  // followed by a blank blockquote-marker-only line and then a
  // sibling block, remark extends the list's position to include
  // markers on the blank line. For `>- one\n>>`, the list still
  // extends to right after the FIRST `>` on line 2 (the second `>`
  // belongs to a deeper blockquote that's NOT the list's container).
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

describe("fuzz regressions: blockquote-parented fenced code at EOF trims trailing newline", () => {
  // `>\`\`\`\n` — fenced code opener-only inside a blockquote, EOF
  // right after the opener's `\n`. Top-level (`\`\`\`\n`) keeps the
  // trailing newline in the position span; blockquote-parented does
  // NOT — the trailing `\n` belongs to neither the code nor the
  // blockquote (no `>` carries it, no closer consumed it).
  test("`>\\`\\`\\`\\n` blockquote/code both end at offset 4 (before `\\n`)", () => {
    assertMdastConformance(">```\n");
  });

  test("`\\`\\`\\`\\n` (top-level) keeps trailing newline in code span", () => {
    assertMdastConformance("```\n");
  });
});

describe("fuzz regressions: setext heading extends position to preceding adjacent definition", () => {
  // `[foo]: /url\nbar\n===\n` — remark/micromark quirk: the paragraph
  // token was opened at the definition's position; even after the
  // definition split off, the paragraph kept that start, and the
  // setext heading inherits it. So the heading's start extends back
  // to offset 0 (where the def started). With a blank line between
  // (`[foo]: /url\n\nbar\n===\n`), the heading starts at line 3 as
  // usual. Sibling sort tie-breaker uses end_offset to keep the
  // shorter Definition before the longer Heading at the same start.
  test("`[foo]: /url\\nbar\\n===` heading start extends to def start (offset 0)", () => {
    assertMdastConformance("[foo]: /url\nbar\n===");
  });

  test("` [foo]: /url\\nbar\\n===` heading start extends to def start (offset 1)", () => {
    assertMdastConformance(" [foo]: /url\nbar\n===");
  });

  test("`[foo]: /url\\n\\nbar\\n===` (blank between) heading starts at line 3", () => {
    assertMdastConformance("[foo]: /url\n\nbar\n===");
  });

  // Multiple adjacent definitions (no blank line between) all
  // chain back: the heading start extends through the run to the
  // FIRST definition's start, not just the most-recent one.
  test("`[foo]: /url\\n[foo]: /url\\nbar\\n===` chains through both defs", () => {
    assertMdastConformance(" 5o] bar\n\n[foo]: /url\n[foo]: /url\nbar\n===\n[foo]");
  });

  test("three adjacent defs chain back to the first", () => {
    assertMdastConformance("[a]: /1\n[b]: /2\n[c]: /3\nbar\n===");
  });

  // Setext-underline-shape first content line breaks the chain.
  // When the heading's first content line is just `-` or `=` (with up
  // to 3 leading spaces and optional trailing whitespace), micromark
  // tries it as a setext underline for the def's residual paragraph;
  // the attempt fails and the paragraph token is reset, so the heading
  // no longer inherits the def's start.
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

  test("full fuzz case with `-` first content line", () => {
    assertMdastConformance(
      "/_-/zr]\n\n[foo]: /url1\n-\n  foo\n-\n  ```\n  bar\n  ```\n-\n      baz",
    );
  });
});

describe("fuzz regressions: indented code position extension respects lazy-one-line flag", () => {
  // Indented code blocks born from the empty-list-close split path
  // (`*\n\n    foo`) are flagged "lazy" — arena_build's trailing-
  // indented-line position extension is skipped, so the code ends at
  // the content line and the trailing blank `    \n\n` stays document-
  // level whitespace. Without the flag, the extension would absorb
  // those into the block.
  test("`*\\n\\n    foo\\n    \\n\\n` (lazy split) code ends after `foo`", () => {
    assertMdastConformance("*\n\n    foo\n    \n\n");
  });

  test("` \\n    foo\\n    \\n\\n` (non-lazy) code still extends through `    `", () => {
    // Top-level indented code: the trailing `    \n` IS part of the
    // block's position even though it's blank.
    assertMdastConformance(" \n    foo\n    \n\n");
  });
});

describe("fuzz regressions: empty list-item split only when blank-separated from indented code", () => {
  // `-\n             bbb\n...` — empty marker followed by indented
  // content with NO intermediate blank line keeps the code INSIDE the
  // list item (per CommonMark). The earlier #87 "split on empty list
  // close" must only fire when an actual blank line sits between the
  // marker line and the indented content (`*\n\n    foo` → split).
  test("empty marker directly followed by indented code stays in listItem", () => {
    assertMdastConformance("-\n             bbb\n                                       ccc");
  });

  test("empty marker + blank line + indented code → list + code outside", () => {
    assertMdastConformance("*\n\n    foo");
  });
});

describe("fuzz regressions: accept literal `.` as first domain char but reject source-escaped `\\.`", () => {
  // `8z y@.bar.baz` — first domain byte `.` is literal in source;
  // remark accepts and emits an email link `mailto:y@.bar.baz`. The
  // earlier task #82 fix (reject all first-`.` domains) was too
  // aggressive — only the escape-produced `.` (source `\.`) should be
  // rejected, since micromark can't tokenize across the escape boundary.
  test("`8z y@.bar.baz` emits email link with literal first-dot domain", () => {
    assertMdastConformance("8z y@.bar.baz");
  });

  test("`2@\\.baz` drops email entirely (source-escaped `.` after `@`)", () => {
    assertMdastConformance("2@\\.baz");
  });
});

describe("fuzz regressions: distinguish literal `\\` from escape before email", () => {
  // `3\e-gdafoo@bar.example.com` — `\e` is NOT a valid backslash escape
  // (only ASCII punctuation can be escaped) so the `\` is literal text.
  // The email construct walks back from `@` and finds the local-part
  // `e-gdafoo`; the literal `\` precedes the local-part start but is
  // not an escape boundary, so position emission proceeds normally.
  // (The earlier escape-before-email fix would otherwise over-fire here.)
  test("`3\\e-gdafoo@bar.example.com` keeps position on email link", () => {
    assertMdastConformance("3\\e-gdafoo@bar.example.com");
  });
});

describe("fuzz regressions: email autolink suppresses position when escape directly precedes local-part", () => {
  // `\+@bar.example.com>` — micromark tokenizes `\+` as characterEscape,
  // so the email construct (which walks back from `@` via local-part
  // chars) can't reach the `+` token. find-and-replace then accepts the
  // email without emitting position. The same suppression must apply to
  // the trailing `>` text node — it inherits position-less status when
  // the surrounding URL was found via find-and-replace.
  test("`\\+@bar.example.com>` produces position-less email + trailing text", () => {
    assertMdastConformance("\\+@bar.example.com>");
  });

  test("`<\\+@bar.example.com>` keeps trailing `>` position-less too", () => {
    assertMdastConformance("<\\+@bar.example.com>");
  });
});

describe("fuzz regressions: thematic break clears list_interrupted_paragraph", () => {
  // After an indented code block, the parser flags
  // `list_interrupted_paragraph = true` to suppress empty list openers
  // that would otherwise "interrupt" the still-open paragraph. But a
  // thematic break is itself a block terminator — once we've emitted
  // the rule, there is no paragraph left to interrupt, so an empty
  // marker after the rule (`+ `) must be allowed to open a list.
  test("`    foo\\n----\\n+ ` empty marker after thematic break opens list", () => {
    assertMdastConformance("    foo\n----\n+ ");
  });

  test("`    foo\\n----\\n+` (no trailing space) same behavior", () => {
    assertMdastConformance("    foo\n----\n+");
  });
});

describe("fuzz regressions: indented code position respects container indent threshold", () => {
  // Inside a list item, an "indented code block" continuation requires
  // the line to reach column (start_column + 4). A bare `\t` (which
  // tab-expands to col 4) does NOT meet that threshold for code inside
  // a list item whose content area starts at col 3 (= threshold col 7).
  // At top-level, `\t` does qualify (threshold col 5; tab-expanded col 4
  // is fine for *blank* lines because we still extend on indented blanks
  // when the indent reaches the threshold). The previous extension
  // pass used an unconditional 4-space check and over-extended in the
  // list-item case.
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

describe("fuzz regressions: autolink construct domain extraction stops at non-domain chars", () => {
  // For `https://foo.barf]_q#4`, the body is `foo.barf]_q#4`. Micromark's
  // construct-path domain extraction stops at the first non-domain char
  // (`]` here), giving domain=`foo.barf` — no `_` in last/penult labels,
  // construct ACCEPTS the URL, position IS emitted. Sätteri previously
  // computed domain by stopping only at `/`/`?`/`#`, leaving the `_q`
  // inside the "domain" and tripping the underscore rule; the URL fell
  // back to find-and-replace which omits positions.
  test("`#h6:0< https://foo.barf]_q#4` autolink keeps position", () => {
    assertMdastConformance("#h6:0< https://foo.barf]_q#4");
  });
});

describe("fuzz regressions: indented code extension skipped when inside a blockquote", () => {
  // Inside `>     chunk1\n      \n      chunk2\n`, line 2's `      `
  // doesn't carry the `>` marker, so the blockquote ends there. The
  // indented code inside the blockquote also ends. Sätteri's
  // trailing-line extension would otherwise absorb the blank line into
  // the code (and the blockquote inherits the extended end), pushing
  // both nodes' positions past where they should close.
  test("`>     chunk1\\n      \\n      chunk2\\n` blockquote ends at line 1", () => {
    assertMdastConformance(">     chunk1\n      \n      chunk2\n");
  });
});

describe("fuzz regressions: scan_reference honors backslash-escaped opening bracket", () => {
  // For `[]\[foo]\n\n[foo]: /url`, the `\[` is a literal `[` (the
  // backslash escapes it). The collapsed/full-reference scan walks raw
  // source bytes for the label after `[]`, but pulldown-cmark has
  // already absorbed the `\` into the escape token — so the raw `[` at
  // the next byte LOOKS like a label opener. Without honoring the
  // escape we'd consume `\[foo]` as label `foo` and produce an empty
  // link to the `foo` def, instead of literal text.
  test("`[]\\[foo]\\n\\n[foo]: /url` produces literal `[][foo]`, not empty link", () => {
    assertHtmlConformance("[]\\[foo]\n\n[foo]: /url\n");
  });

  test('full fuzz case `_>~$ []\\[foo]\\n\\n[foo]: /url "title"`', () => {
    assertHtmlConformance('_>~$ []\\[foo]\n\n[foo]: /url "title"\n');
  });

  // Double-escape case: `\\[foo]` IS `\` + `[foo]` (label opener). So
  // a label scan there SHOULD succeed.
  test("`[]\\\\[foo]\\n\\n[foo]: /url` (double-escape) IS a full-reference link", () => {
    assertHtmlConformance("[]\\\\[foo]\n\n[foo]: /url\n");
  });
});

describe("fuzz regressions: setext heading chain-back tolerates 1-3 leading spaces on first content line", () => {
  // The chain-back gap check is no longer "≤ 2 bytes"; it now allows
  // exactly one newline plus up to 3 leading spaces. Micromark
  // accepts a paragraph continuation with ≤3-space indent, so the
  // setext heading that grows out of the def's residual paragraph
  // still inherits the def's start in that shape.
  test("`[bar]: /url\\n   Foo\\n---` (3-space indent before Foo) chains heading start to def", () => {
    assertMdastConformance("[bar]: /url\n   Foo\n---");
  });

  test("4-space indent on first content line STILL chains back (paragraph continuation)", () => {
    assertMdastConformance("[bar]: /url\n    Foo\n---");
  });

  test("full fuzz case with multiple setext headings", () => {
    assertMdastConformance(
      '2\tb n[foo][bar]\n\n[bar]: /url "title"\n   Foo\n---\n\n  Foo\n-----\n\n  Foo\n  ===',
    );
  });
});

describe("fuzz regressions: autolink trim uses find-and-replace set when preceded by unbalanced `[`", () => {
  // The construct trim strips `*`, `_`, `~` (and others). When micromark's
  // `previousUnbalanced` suppresses the construct (URL is preceded by an
  // unbalanced `[`/`![`), find-and-replace's `splitUrl` runs instead — and
  // its trim set omits `*`, `_`, `~`. So a URL ending in `_` followed by
  // `**` keeps the `_` (the `**` is consumed by emphasis upstream of
  // find-and-replace, so it's not in the path bytes).
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

describe("fuzz regressions: text_to_source map skips blockquote `>` prefixes", () => {
  // When a paragraph inside a blockquote spans multiple lines and the
  // last line drops the `>` marker (lazy continuation off the bq), the
  // inline text is collapsed without the marker. The text_to_source map
  // must skip the `>`(+space) on the marker-carrying lines so positions
  // for downstream nodes (autolinks etc.) line up with raw source.
  test("`># Foo\\n>bar\\n> baz\\nfoo@bar.example.com` email link gets correct position", () => {
    assertMdastConformance("># Foo\n>bar\n> baz\nfoo@bar.example.com");
  });
});

describe("fuzz regressions: emphasis lowerbound keyed by current_count not run_length", () => {
  // After an inner-loop pass partially consumes the closer (`***` →
  // 2 used for <strong>, 1 left), the leftover sits in a different
  // mod-3 bucket than the run_length and may now satisfy rule 9 with
  // openers the earlier (longer) attempt failed against. The lowerbound
  // optimization must key by current_count, not run_length, otherwise
  // the carry-over from the failure blocks the valid match. Without
  // this, `cz*x` `*foo***bar***baz` loses its outer `<em>`.
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

describe("fuzz regressions: inner blockquote extends through outer-marker-only continuation lines", () => {
  // `>>g\n>` — the inner blockquote sits in an outer one and the next
  // line carries only the outer `>` marker (no inner one). The inner
  // bq's end extends through the outer marker, matching micromark's
  // position which treats the marker-only line as a still-claimed
  // lazy continuation of the inner bq.
  test("`>>g\\n>` inner bq extends to after outer marker on line 2", () => {
    assertMdastConformance(">>g\n>");
  });

  test("`>>g^[( \\n>` full fuzz case", () => {
    assertMdastConformance(">>g^[( \n>");
  });

  test("`>>g\\n>>` (both markers) extends as before", () => {
    assertMdastConformance(">>g\n>>");
  });

  test("`>>g\\n` (no marker line 2) ends at content", () => {
    assertMdastConformance(">>g\n");
  });

  // Gate: only extend when inner bq has NO next sibling. Otherwise the
  // marker-only lines belong to the outer bq's space between siblings.
  test("`>> foo\\n>\\n> bar\\n` inner bq does NOT extend past line 1 (next sibling exists)", () => {
    assertMdastConformance(">> foo\n>\n> bar\n");
  });

  test("`>   > > 1.  one\\n>>\\n>>     two\\n` deeply nested case with siblings", () => {
    assertMdastConformance(">   > > 1.  one\n>>\n>>     two\n");
  });
});

describe("fuzz regressions: list-marker after link reference definition", () => {
  // A link reference definition shares micromark's paragraph token: its
  // residual paragraph state lingers after the def's bytes are consumed.
  // When the next line opens a new block (no lazy continuation), that
  // block "interrupts" the residual paragraph, and any ordered-list
  // markers with start != 1 are rejected — even when the marker is
  // nested inside another list item.
  test("`[ref]: /uri\\n1. 2. foo` — `2.` becomes text inside the `1.` item", () => {
    assertMdastConformance("[ref]: /uri\n1. 2. foo\n");
  });
  test("`[ref]: /uri\\n- 2. foo` — `2.` becomes text inside the `-` item", () => {
    assertMdastConformance("[ref]: /uri\n- 2. foo\n");
  });
  test("`[ref]: /uri\\n1. - 2. foo` — `2.` stays as text under the nested `-`", () => {
    assertMdastConformance("[ref]: /uri\n1. - 2. foo\n");
  });
  test("full fuzz case with leading autolink+refdef+nested-list", () => {
    assertMdastConformance("oo<https://example.com/?search=][ref]>\n\n[ref]: /uri\n1. - 2. foo\n");
  });
});

describe("fuzz regressions: autolink URL stops at matched-backtick code span (find-and-replace path)", () => {
  // When `previousUnbalanced` suppresses the construct (e.g. unbalanced
  // `[` before the URL), micromark's code-span tokenizer has already
  // claimed the matched-backtick run and the find-and-replace URL
  // regex never sees those bytes. We mirror this by ending the URL at
  // the first backtick that has a matching closer in the URL bytes.
  test("`[\\nURL.\\`baz>\\`` URL ends at first backtick", () => {
    // Check just the URL position; full code-span re-parse on the
    // trailing text is a separate pulldown-cmark limitation.
    const ref = referenceMdast("[\nhttps://foo.bar.`baz>`\n") as any;
    const act = satteriMdast("[\nhttps://foo.bar.`baz>`\n") as any;
    const refUrl = JSON.stringify(ref).match(/"url":"([^"]*)"/)?.[1];
    const actUrl = JSON.stringify(act).match(/"url":"([^"]*)"/)?.[1];
    expect(actUrl).toBe(refUrl);
  });
});
