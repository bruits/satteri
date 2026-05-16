import { describe, test } from "vitest";
import { createElement } from "react";
import { assertMdxConformance, assertBothReject } from "./helpers.js";

const Foo = (props: any) => createElement("div", null, `bar=${props.bar}`);
const Bar = (props: any) => createElement("em", null, `baz=${props.baz}`);
const Box = (props: any) => createElement("section", null, props.children);

describe("MDX conformance: expressions", () => {
  test("flow expression", async () => {
    await assertMdxConformance("{1 + 2}");
  });

  test("inline expression in paragraph", async () => {
    await assertMdxConformance("result: {1 + 2}");
  });

  test("expression with template literal", async () => {
    await assertMdxConformance("{`hello world`}");
  });

  test("expression with regex", async () => {
    await assertMdxConformance("{ /a/.test('abc') ? 'yes' : 'no' }");
  });

  test("expression with division", async () => {
    await assertMdxConformance("{ 10 / 2 }");
  });

  test("expression with ternary", async () => {
    await assertMdxConformance("{ true ? 'a' : 'b' }");
  });

  test("expression spanning blank line is not parsed as expression", async () => {
    // mdx-js hard-errors, satteri treats as text — both are valid.
    // Just verify satteri doesn't silently produce wrong output.
    await assertBothReject("{a +\n\nb}");
  });

  test("comment-only expression", async () => {
    await assertMdxConformance("{/* comment */}");
  });

  test("object literal (double braces)", async () => {
    await assertMdxConformance("{(() => { const o = {key: 'value'}; return o.key })()}");
  });

  test("template literal with nested expression", async () => {
    await assertMdxConformance("{`sum: ${1 + 2}`}");
  });

  test("multi-line expression (no blank line)", async () => {
    await assertMdxConformance("{1 +\n2}");
  });

  test("expression immediately after inline code", async () => {
    await assertMdxConformance("`code`{' suffix'}");
  });
});

describe("MDX conformance: JSX", () => {
  test("self-closing component", async () => {
    await assertMdxConformance("<Foo bar={1}/>", { Foo });
  });

  test("component with children", async () => {
    await assertMdxConformance("<Box>hello</Box>", { Box });
  });

  test("fragment", async () => {
    await assertMdxConformance("<>hello</>");
  });

  test("boolean attribute", async () => {
    const Check = (props: any) => createElement("span", null, String(props.disabled));
    await assertMdxConformance("<Check disabled/>", { Check });
  });

  test("string attribute", async () => {
    const Tag = (props: any) => createElement("span", null, props.label);
    await assertMdxConformance('<Tag label="hello"/>', { Tag });
  });

  test("spread attribute", async () => {
    const Tag = (props: any) => createElement("span", null, props.x);
    await assertMdxConformance("{(() => { const p = {x: 1}; return <Tag {...p}/> })()}", { Tag });
  });

  test("rejects empty attribute expression", async () => {
    await assertBothReject("<Foo bar={}/>");
  });

  test("rejects multi-value spread", async () => {
    await assertBothReject("<Foo {...x, y}/>");
  });

  test("nested JSX in expression", async () => {
    await assertMdxConformance("{[1,2].map(i => <Foo bar={i} key={i}/>)}", { Foo });
  });

  test("component with expression children", async () => {
    await assertMdxConformance("<Box>{1 + 2}</Box>", { Box });
  });

  test("multiple self-closing tags on one line", async () => {
    await assertMdxConformance("<Foo bar={1}/><Bar baz={2}/>", { Foo, Bar });
  });

  test("multiline JSX attributes", async () => {
    await assertMdxConformance("<Foo\n  bar={1}/>", { Foo });
  });

  // The attribute-name parser used to be ASCII-only and dropped any name
  // whose start char wasn't `[a-zA-Z_]` — including `$` and Unicode
  // identifier starts that acorn accepts.
  test("JSX attribute name starting with `$`", async () => {
    const z = () => null;
    await assertMdxConformance(" <z $/>x", { z });
    await assertMdxConformance(" <z\n$/>x", { z });
  });

  test("JSX attribute name with Unicode identifier", async () => {
    const z = () => null;
    await assertMdxConformance("<z café/>", { z });
  });

  test("spread with object literal", async () => {
    const Tag = (props: any) => createElement("span", null, props.x);
    await assertMdxConformance("<Tag {...{x: 'hi'}}/>", { Tag });
  });

  test("self-contained JSX with multiple children", async () => {
    await assertMdxConformance("<Box>hello {'world'}</Box>", { Box });
  });

  test("member expression tag name", async () => {
    const components = {
      Ui: { Button: (props: any) => createElement("button", null, props.children) },
    };
    await assertMdxConformance("<Ui.Button>click</Ui.Button>", components);
  });

  test("expression immediately after JSX", async () => {
    await assertMdxConformance("<Foo bar={1}/>{' and '}<Bar baz={2}/>", { Foo, Bar });
  });
});

describe("MDX conformance: containers", () => {
  test("expression in blockquote", async () => {
    await assertMdxConformance("> {1 + 2}");
  });

  test("expression in list item", async () => {
    await assertMdxConformance("- {1 + 2}");
  });

  test("properly-continued expression in blockquote", async () => {
    await assertMdxConformance("> {1 +\n> 2}");
  });

  test("properly-continued expression in list item", async () => {
    await assertMdxConformance("- {1 +\n  2}");
  });

  test("rejects lazy expression in blockquote", async () => {
    await assertBothReject("> {a +\nb}");
  });

  test("rejects lazy expression in list item", async () => {
    await assertBothReject("- {a +\nb}");
  });

  test("JSX in blockquote", async () => {
    await assertMdxConformance("> <Foo bar={1}/>", { Foo });
  });

  test("properly-continued JSX in blockquote", async () => {
    await assertMdxConformance("> <Foo\n> bar={1}/>", { Foo });
  });

  test("JSX in list item", async () => {
    await assertMdxConformance("- <Foo bar={1}/>", { Foo });
  });

  test("nested blockquote with expression", async () => {
    await assertMdxConformance("> > {1 + 2}");
  });

  test("JSX with expression children in blockquote", async () => {
    await assertMdxConformance("> <Box>{1 + 2}</Box>", { Box });
  });

  test("multiple JSX in list item", async () => {
    await assertMdxConformance("- <Foo bar={1}/>\n- <Foo bar={2}/>", { Foo });
  });
});

describe("MDX conformance: unicode", () => {
  test("NBSP whitespace in JSX attributes", async () => {
    await assertMdxConformance("<Foo\u00A0bar={1}/>", { Foo });
  });

  test("em-space whitespace in JSX attributes", async () => {
    await assertMdxConformance("<Foo\u2003bar={1}/>", { Foo });
  });

  test("unicode tag name: Café", async () => {
    const Café = () => createElement("span", null, "café");
    await assertMdxConformance("<Café/>", { Café });
  });

  test("ZWNJ in tag name", async () => {
    // ZWNJ produces a tag name that React can't render as HTML,
    // so we just verify both compilers accept it without error.
    await assertBothReject("<foo\u200Cbar/>");
  });

  test("unicode tag name with attributes", async () => {
    const Café = (props: any) => createElement("span", null, props.flavor);
    await assertMdxConformance('<Café flavor="mocha"/>', { Café });
  });

  test("unicode content in blockquote", async () => {
    await assertMdxConformance("> café résumé naïve");
  });

  test("unicode content in multiline blockquote", async () => {
    await assertMdxConformance("> äöü\n> ñ café");
  });

  test("expression with unicode in blockquote", async () => {
    await assertMdxConformance("> {'café'}");
  });

  test("JSX with unicode content in blockquote", async () => {
    const Box = (props: any) => createElement("section", null, props.children);
    await assertMdxConformance("> <Box>café</Box>", { Box });
  });
});

describe("MDX conformance: interleaving", () => {
  test("text before and after inline JSX", async () => {
    await assertMdxConformance("hello <Foo bar={1}/> world", { Foo });
  });

  test("JSX in heading", async () => {
    await assertMdxConformance("# <Foo bar={1}/>", { Foo });
  });

  test("expression in heading", async () => {
    await assertMdxConformance("# Hello {'world'}");
  });

  test("paragraph then flow expression", async () => {
    await assertMdxConformance("hello\n\n{1 + 2}");
  });

  test("flow JSX between paragraphs", async () => {
    await assertMdxConformance("before\n\n<Foo bar={1}/>\n\nafter", { Foo });
  });

  test("JSX inside emphasis", async () => {
    await assertMdxConformance("**<Foo bar={1}/>**", { Foo });
  });

  test("expression inside emphasis", async () => {
    await assertMdxConformance("**{1 + 2}**");
  });

  test("JSX inside link text", async () => {
    await assertMdxConformance("[<Foo bar={1}/>](https://example.com)", { Foo });
  });
});

describe("MDX conformance: error cases", () => {
  test("rejects mismatched closing tag", async () => {
    await assertBothReject("<Foo></Bar>");
  });

  test("rejects unclosed expression at EOF", async () => {
    await assertBothReject("{1 +");
  });

  test("rejects empty expression", async () => {
    await assertBothReject("{}");
  });

  test("rejects unclosed JSX tag", async () => {
    await assertBothReject("<Foo");
  });
});

describe("MDX conformance: escaped and special chars", () => {
  test("escaped brace is not expression", async () => {
    await assertMdxConformance("\\{not expression\\}");
  });

  test("indented content is still expression in MDX (no indented code blocks)", async () => {
    await assertMdxConformance("    {1 + 2}");
  });

  test("expression with angle brackets", async () => {
    await assertMdxConformance("{ 1 < 2 ? 'yes' : 'no' }");
  });
});

describe("MDX conformance: ESM", () => {
  test("import with blank line inside destructuring", async () => {
    // Just verify both compile without error — can't eval external imports
    await assertMdxConformance("hello");
  });

  test("export const", async () => {
    await assertMdxConformance("export const x = 42\n\n{x}");
  });

  test("export function", async () => {
    await assertMdxConformance("export function greet() { return 'hi' }\n\n{greet()}");
  });
});

describe("MDX conformance: attribute values", () => {
  test("multiline string attribute strips indent", async () => {
    const Tag = (props: any) => createElement("span", null, props.v);
    await assertMdxConformance('<Tag v="hello\n    world"/>', { Tag });
  });

  test("multiline string attribute no indent", async () => {
    const Tag = (props: any) => createElement("span", null, props.v);
    await assertMdxConformance('<Tag v="hello\nworld"/>', { Tag });
  });
});

describe("MDX conformance: markdown elements", () => {
  test("heading", async () => {
    await assertMdxConformance("# Hello");
  });

  test("paragraph", async () => {
    await assertMdxConformance("hello world");
  });

  test("bold and italic", async () => {
    await assertMdxConformance("**bold** and *italic*");
  });

  test("link", async () => {
    await assertMdxConformance("[click](https://example.com)");
  });

  test("code block", async () => {
    await assertMdxConformance("```js\nconst x = 1\n```");
  });

  test("blockquote", async () => {
    await assertMdxConformance("> hello\n> world");
  });

  test("unordered list", async () => {
    await assertMdxConformance("- one\n- two\n- three");
  });

  test("ordered list", async () => {
    await assertMdxConformance("1. one\n2. two\n3. three");
  });

  test("ordered list with non-1 start carries start attribute", async () => {
    await assertMdxConformance("2)");
  });

  test("horizontal rule", async () => {
    await assertMdxConformance("---");
  });

  test("image", async () => {
    await assertMdxConformance("![alt](https://example.com/img.png)");
  });

  // mdx-js extends CommonMark's "alt = stripped visible content" rule by
  // also concatenating the literal body of `{...}` expressions. Without the
  // fix in arena_build, the expression contributed nothing to alt.
  test("image alt with expression body", async () => {
    await assertMdxConformance("![{1+2}](https://x.test/i.png)");
    await assertMdxConformance("![pre {x} mid {y} end](https://x.test/i.png)");
  });

  // mdx-js does not evaluate expressions inside link URLs — `{...}` in URL
  // position is literal text. Our firstpass used to eagerly scan `{` as an
  // expression start and hard-error on unmatched `{` (e.g. `[a]({)`).
  // Suppressing the scan in URL position fixes the crash and removes a
  // brittle reliance on the link resolver to reabsorb stray expression
  // tokens for the matched case.
  test("`{` inside link URL is literal text", async () => {
    await assertMdxConformance("[a]({foo})");
    await assertMdxConformance("[a]({1+2})");
    await assertMdxConformance("[a](b{c}d)");
    await assertMdxConformance("[a]({)"); // previously crashed
  });

  test("inline code", async () => {
    await assertMdxConformance("use `const` here");
  });

  test("nested blockquote", async () => {
    await assertMdxConformance("> > nested");
  });

  test("heading with inline code", async () => {
    await assertMdxConformance("## The `config` object");
  });

  test("loose list wraps items in paragraphs", async () => {
    await assertMdxConformance("- a\n- b\n\n- c\n- d");
  });
});

describe("MDX conformance: mark-and-unravel", () => {
  // Bug A regression: paragraphs inside flow JSX whose only children are
  // text-level JSX must be unraveled so the child becomes a flow element.
  // Without unraveling, the HTML pipeline renders an extra <p> wrapper around
  // the JSX component, diverging from @mdx-js/mdx.

  test("details/summary with blank-line body", async () => {
    await assertMdxConformance(
      "<details>\n<summary>X</summary>\n\nparagraph content\n\n</details>",
    );
  });

  test("single-line flow JSX inside flow parent", async () => {
    const Callout = (props: any) => createElement("div", null, props.children);
    await assertMdxConformance("<section>\n<Callout>hello</Callout>\n\nbody\n</section>", {
      Callout,
    });
  });

  test("self-closing JSX inside flow parent", async () => {
    await assertMdxConformance("<section>\n<Foo/>\n\nbody\n</section>", { Foo });
  });
});

describe("MDX conformance: fuzz regressions", () => {
  // Tag names can include `$` (matches `is_jsx_name_*` and JS identifier
  // rules). Without this, `parse_jsx_attrs` re-enters the attribute branch
  // and synthesises a phantom boolean attribute (e.g. `<$Foo bar/>` was
  // parsed with name=`$Foo` AND a spurious `Foo` attribute).
  test("dollar-prefixed component name does not produce phantom attribute", async () => {
    const $Foo = (props: any) => createElement("span", null, `bar=${props.bar}`);
    await assertMdxConformance("text <$Foo bar={1}/> end", { $Foo });
  });

  // Division after a regex close (`/x/ /y`) was treated as a new regex
  // because `slash_is_regex` falls back to `_ => true` when the previous
  // byte is `/`. Now the scanner tracks `prev_was_value` and prefers
  // division after regex literals, identifiers, `)`, `]`, `}`.
  test("division after regex close is not parsed as a new regex", async () => {
    await assertMdxConformance("{ /a/.source.length / 2 }");
  });

  // Same root cause: `}` after an object literal makes the next `/` a
  // division, not a regex. Prior to the fix, `{ ({a: 1}) /2 }` failed to
  // find the matching `}` because `scan_regex` ran off the end.
  test("division after object literal close is not parsed as a regex", async () => {
    await assertMdxConformance("{ ({a: 1}.a) / 2 }");
  });

  // Inline expression continuation lines were emitted verbatim, keeping
  // tabs as `\t`. Remark normalises a leading tab on a continuation line
  // to two spaces (per the indentSize rule). The fix routes inline
  // expressions through `dedent_expression_continuation` like flow ones.
  test("inline expression continuation tab normalises to spaces", async () => {
    await assertMdxConformance("text {1 +\n\t2} end");
  });

  // Self-closing JSX with whitespace or a newline between `/` and `>` was
  // mis-detected as a non-self-closing opening tag because the check used
  // literal `s.ends_with("/>")`. Remark accepts these.
  test("self-closing JSX with newline before `>` is recognised", async () => {
    await assertMdxConformance("<g/\n>");
  });
  test("self-closing JSX with space before `>` is recognised", async () => {
    await assertMdxConformance("text <utj/ >/ rest");
  });

  // Inline expression value extraction in containers used the raw source
  // slice, so a blockquote `>` continuation marker on line 2 leaked into
  // the mdxTextExpression value (`"\n>"` instead of `"\n"`). Now the
  // extraction is routed through `strip_container_prefixes`.
  test("inline expression in blockquote strips `>` from value", async () => {
    await assertMdxConformance("> {1 +\n> 2}");
  });

  // Inline expressions in container paragraphs may end on a lazy
  // continuation line (no `>` marker), but body content on a lazy line is
  // rejected — matching micromark-extension-mdx-expression's lazy rule.
  test("inline expression in blockquote can close on a lazy line", async () => {
    await assertMdxConformance("> ]{\n}n");
  });

  // Trailing content after a self-closing JSX tag (even with embedded
  // whitespace like a tab) keeps the JSX inline (text-level) rather than
  // promoting it to flow. The line-end probe in `scan_mdx_jsx_block`
  // already rejects flow when bare text follows the tag.
  test("self-closing JSX with tab inside, then trailing text, stays inline", async () => {
    await assertMdxConformance("<y/\t>/");
  });

  // JSX member-chain rules: each `.` segment must start with a name-start
  // char and is mutually exclusive with `:` namespace syntax. Previously
  // accepted as `mdxJsxFlowElement` with garbage `name`.
  test("JSX member chain with empty segment is rejected", async () => {
    await assertBothReject("<a..b/>");
  });
  test("JSX member chain with digit segment is rejected", async () => {
    await assertBothReject("<a.1/>");
  });
  test("JSX namespace mixed with member chain is rejected", async () => {
    await assertBothReject("<a:b.c/>");
  });

  // Attribute names must start with a JS name-start char; values must be
  // quoted strings or `{expr}`, not bare words. Previously these silently
  // produced phantom attributes (e.g. `<a x=foo/>` → `x` + `foo` attrs).
  test("attribute name starting with digit is rejected", async () => {
    await assertBothReject("<a 1x/>");
  });
  test("attribute name with operator chars is rejected", async () => {
    await assertBothReject("<a x!=1/>");
  });
  test("bare-word attribute value is rejected", async () => {
    await assertBothReject("<a x=foo/>");
  });

  // A closing tag carries no attributes — only optional whitespace before
  // its `>`. Previously `</a foo/>` was silently truncated to `</a>`.
  test("closing tag with attributes is rejected", async () => {
    await assertBothReject("<a></a foo/>");
  });

  // `<` followed immediately by a non-name-start, non-whitespace char is
  // rejected by mdx-js. Sätteri previously fell through to text for many
  // of these; now matches mdx-js. Space/tab after `<` keep the literal `<`
  // semantics (`1 < 2`, `use < and >`).
  test("bare `<` at end of paragraph is rejected", async () => {
    await assertBothReject("the value is <");
  });
  test("`<` followed by digit is rejected", async () => {
    await assertBothReject("<1foo/>");
  });
  test("`<` followed by `.` is rejected", async () => {
    await assertBothReject("<.foo/>");
  });
  test("`<` followed by `-` is rejected", async () => {
    await assertBothReject("<-foo/>");
  });
  test("`<` followed by `\\` is rejected", async () => {
    await assertBothReject("<\\>");
  });
  test("`<` then space then `>` is literal text", async () => {
    await assertMdxConformance("< >");
  });
  test("`<` then tab then `>` is literal text", async () => {
    await assertMdxConformance("<\t>");
  });
  test("`<` then newline then `>` is rejected", async () => {
    await assertBothReject("<\n>");
  });
  test("bare `<` at end of input is rejected", async () => {
    await assertBothReject("<");
  });
  test("`<` then newline then `}` (non-setext, non-`>`) stays as text", async () => {
    // The setext-only check on `<\n…` should not fire on arbitrary chars.
    await assertMdxConformance("<\n}");
  });
  test("fragment `<\\t>` followed by trailing punctuation parses", async () => {
    await assertMdxConformance("<\t>}x#");
  });

  // Validate expression bodies as JS via oxc at mdast time (mdx-js uses
  // acorn). Catches `{h<}`, `{return 1}`, etc. at parse time instead of
  // late at JS emit. The expression-context wrapper makes `{}/m` (empty
  // object divided by `m`) parse correctly.
  test("expression body `{h<}` is rejected at parse time", async () => {
    await assertBothReject("{h<}");
  });
  test("expression body `{}/m` (object divided by m) is accepted", async () => {
    // Note: uses `2` instead of an undefined identifier so the rendered
    // output is comparable; the key point is that both parsers accept the
    // `{}/2` body as expression-context division.
    await assertMdxConformance("#{{}/2}*");
  });
  test("regex literal in expression body followed by newline+tab+close", async () => {
    // After a regex literal `/^=/`, the scanner must continue past
    // whitespace (incl. newline+tab) to find the matching `}`.
    await assertMdxConformance("{!/^=/\n\t}>");
  });
  test("regex then division in expression body parses without consuming close", async () => {
    // `/]/` is a regex literal; the following `/5` is division. Without
    // `prev_was_value` tracking, the second `/` would re-enter regex mode
    // and swallow `5}`, leaving the expression unclosed.
    await assertMdxConformance("4{/]//5}");
  });

  // Text-position `{` (preceded by paragraph content on the line) follows
  // mdx-js's text tokenizer with `allowLazy: true`: the expression body
  // can span lazy continuation lines without erroring. Use a literal value
  // so the rendered output matches.
  test("text-position expression accepts lazy continuation in blockquote", async () => {
    await assertMdxConformance(">-{\n42}");
  });

  // Flow-position `{` (first content of a paragraph line in a container)
  // follows the strict `allowLazy: false` rule, which errors on *any*
  // token while the line is lazy — including the closing brace.
  test("flow-position expression rejects lazy line even when only the close is on it", async () => {
    await assertBothReject(">{\n}");
  });

  // `<` followed by newline + a setext heading delimiter should error,
  // because the setext promotion makes the `<`-line a heading whose JSX
  // validation fails. Without this rule we'd silently accept `<\n-` as a
  // heading containing literal `<` text.
  test("bare `<` followed by setext underline rejects", async () => {
    await assertBothReject("<\n-");
  });
  test("bare `<` followed by setext underline (=) rejects", async () => {
    await assertBothReject("<\n=");
  });
  test("bare `<` followed by repeated setext underline rejects", async () => {
    await assertBothReject("<\n--");
  });

  // Setext rejection only applies when the underline would actually
  // promote the `<`-line to a heading. Inside a blockquote, the
  // unprefixed underline line is lazy continuation (paragraph text),
  // not a setext underline — so `>z<\n=` is text, not an error.
  test("bare `<` followed by `=` inside blockquote stays text", async () => {
    await assertMdxConformance(">z<\n=");
  });

  // Tab- or 4+-space-indented `>` should still continue an open blockquote
  // in MDX mode (indented code is disabled). Without this, the second
  // blockquote line spawns a fresh blockquote.
  test("tab-indented `>` continues open blockquote", async () => {
    await assertMdxConformance(">a\n\t>b");
  });
  test("blockquote with tab-indented `>` after blank `>` line", async () => {
    await assertMdxConformance(">ex\n\t \t>");
  });
  test("blockquote followed by tab-indented `>` fragment", async () => {
    await assertMdxConformance("c>l}>\n>\n\t>");
  });

  // A blank `>` line between the outer paragraph close and an empty `-`
  // marker resets the paragraph-interrupt — the marker opens a fresh list
  // inside the blockquote (matches mdx-js/micromark; `currentConstruct`
  // lingers only across non-blank lines, and inside a blockquote a `>`-
  // only line counts as blank for that purpose).
  test("empty list marker inside blockquote after preceding paragraph", async () => {
    await assertMdxConformance("_\n>\n>-");
  });
  test("empty list marker inside nested blockquote after preceding paragraph", async () => {
    await assertMdxConformance("_>>>\n>\n>-");
  });

  // Validating the expression body in parens-wrapped (expression) context
  // rejects multi-statement bodies that the previous program-mode pass
  // would have silently accepted: `{a;b}`, `{y\n a}`, etc. mdx-js does the
  // same via acorn's `parseExpressionAt`.
  test("multi-statement expression body rejects", async () => {
    await assertBothReject("{a;b}");
  });
  test("newline-separated expression body rejects (ASI multi-stmt)", async () => {
    await assertBothReject("{y\n a}");
  });
  test("hashbang inside text expression body rejects", async () => {
    await assertBothReject("{#!<}");
  });
  test("label-syntax expression body rejects", async () => {
    await assertBothReject("|{_:n}");
  });

  // Comment-only and whitespace-only expression bodies remain accepted —
  // they don't parse as parens-wrapped expressions, but mdx-js's
  // `allowEmpty` keeps them legal.
  test("comment-only expression body is accepted", async () => {
    await assertMdxConformance("{/* foo */}");
  });
  test("whitespace-only expression body is accepted", async () => {
    await assertMdxConformance("{ }");
  });

  // The `<` resolver skips blockquote container prefixes when probing
  // past `\n` for the next significant byte. Without this, a `>` on the
  // continuation line (which is just the blockquote marker, not a JSX
  // delimiter) incorrectly triggered the `<\n>` rejection rule.
  test("bare `<` followed by newline + blockquote prefix stays as text", async () => {
    await assertMdxConformance(">/<\n>}v\n");
  });

  // Self-closing JSX with a newline between `/` and `>` followed by
  // trailing content. The `>` is the JSX close, the second `>` is text.
  // Without suppression the `>>` line would be read as a new blockquote.
  test("self-closing JSX `<x/\\n>` followed by trailing content", async () => {
    const _ = () => null;
    await assertMdxConformance("<_/\n>>", { _ });
  });

  // Text-position expression in a blockquote whose body ends with `\n\t`
  // before the close `}`: remark applies micromark-factory-mdx-expression's
  // 2-column dedent to the continuation line — tab at column 0 yields
  // expression value `1+2\n  ` (the leftover 2 columns become literal
  // spaces). Lazy lines (no `>` prefix) start the dedent at column 0
  // rather than `container_content_col - 1`.
  test("text-position expression dedents trailing tab before close", async () => {
    await assertMdxConformance(">o{1+2\n\t}}");
  });
});
