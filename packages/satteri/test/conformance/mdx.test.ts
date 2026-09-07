import { describe, expect, test } from "vitest";
import { createElement } from "react";
import {
  assertMdxConformance,
  assertMdxDevPositionConformance,
  assertMdxMathConformance,
  assertMdxPluginConformance,
  assertBothReject,
} from "./helpers.js";
import { mdxToJs, mdxToMdast, defineMdastPlugin, defineHastPlugin } from "../../src/index.js";
import type { Element } from "hast";
import type { MdxJsxFlowElement, MdxJsxFlowElementData } from "../../src/mdx-types.js";

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
    const { satteriMdxMdast, referenceMdxMdast } = await import("./fuzz/shared.js");
    expect(satteriMdxMdast("{a +\n\nb}")).toEqual(referenceMdxMdast("{a +\n\nb}"));
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

  test("HTML entity in JSX text content", async () => {
    await assertMdxConformance("<p>tab &gt; arrow</p>");
    await assertMdxConformance("<p>amp &amp; sand</p>");
    await assertMdxConformance("<p>numeric &#62; ref</p>");
  });

  test("HTML entity in JSX text inside flow expression", async () => {
    await assertMdxConformance("{ true && (<ol><li>foo &gt; bar</li></ol>) }");
  });

  test("multi-line JSX attribute expression with indent", async () => {
    await assertMdxConformance("<Foo bar={\n  1 +\n    2\n}/>", { Foo });
  });

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

  // React rejects the ZWNJ tag name at render time, so compare trees instead.
  test("ZWNJ in tag name", async () => {
    const { satteriMdxMdast, referenceMdxMdast } = await import("./fuzz/shared.js");
    expect(satteriMdxMdast("<foo\u200Cbar/>")).toEqual(referenceMdxMdast("<foo\u200Cbar/>"));
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

  test("empty expression is accepted by both", async () => {
    await assertMdxConformance("{}");
  });

  test("rejects unclosed JSX tag", async () => {
    await assertBothReject("<Foo");
  });

  test("rejects legacy octal literal `01`", async () => {
    await assertBothReject("{01}");
  });

  test("rejects legacy octal literal `0123`", async () => {
    await assertBothReject("{0123}");
  });

  test("rejects non-octal-decimal `09`", async () => {
    await assertBothReject("{09}");
  });

  test("rejects legacy octal in nested expression", async () => {
    await assertBothReject("{1 + 02}");
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
    // External imports can be compiled here but cannot be evaluated without their modules.
    await assertMdxConformance("hello");
  });

  test("export const", async () => {
    await assertMdxConformance("export const x = 42\n\n{x}");
  });

  test("export function", async () => {
    await assertMdxConformance("export function greet() { return 'hi' }\n\n{greet()}");
  });

  test("blank line inside template literal in export (#111)", async () => {
    await assertMdxConformance("export const code = `first line\n\nsecond line`;\n\n{code}");
  });

  test("blank line between template literals in export (#111)", async () => {
    await assertMdxConformance("export const x = `a` +\n\n`b`;\n\n{x}");
  });

  test("blank line inside block comment in export (#111)", async () => {
    await assertMdxConformance("export const y = 1; /* note\n\nstill note */\n\n{y}");
  });

  test("regex with backtick in export (#111)", async () => {
    await assertMdxConformance("export const re = /a`b/;\n\n{re.source}");
  });

  test("regex with quotes in export (#111)", async () => {
    await assertMdxConformance("export const re = /[\"']/g;\n\n{re.source}");
  });

  test("export const used as JSX component resolves to module binding", async () => {
    await assertMdxConformance("export const Comp = () => <span>local</span>\n\n<Comp />");
  });

  test("export function used as JSX component resolves to module binding", async () => {
    await assertMdxConformance("export function FnComp() { return <span>fn</span> }\n\n<FnComp />");
  });

  test("mixed module-bound and prop-provided JSX components", async () => {
    const Provided = (props: any) => createElement("em", null, `provided:${props.label ?? ""}`);
    await assertMdxConformance(
      'export const Local = () => <span>local</span>\n\n<Local /> then <Provided label="x" />',
      { Provided },
    );
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

  test("close tag inside string attribute is text, not a real close", async () => {
    const Demo = (props: any) => createElement("div", null, props.code, props.children);
    await assertMdxConformance('<Demo code="</Demo>">child</Demo>', { Demo });
  });

  test("self-referential close tag inside template-literal attribute (#74)", async () => {
    const CodePreview = (props: any) =>
      createElement("figure", null, createElement("pre", null, props.code), props.children);
    const src = [
      "<CodePreview",
      "  code={`<CodePreview",
      '    code="The code to preview"',
      '    lang="astro"',
      ">",
      "    The preview can be manually added here.",
      "</CodePreview>`}",
      '  label="Using a code sample with a preview"',
      '  lang="astro"',
      ">",
      '  <CodePreview code="The code to preview" lang="astro">',
      "    The preview can be manually added here.",
      "  </CodePreview>",
      "</CodePreview>",
    ].join("\n");
    await assertMdxConformance(src, { CodePreview });
  });

  test("regex with quotes in attribute expression (#112)", async () => {
    const LinkedCode = (props: any) => createElement("code", null, String(props.ins[0]));
    const src = [
      "<LinkedCode",
      '  lang="angular-html"',
      `  ins={[/icon="[^"]+"/g, 'useFilledIcon="true"']}`,
      "/>",
    ].join("\n");
    await assertMdxConformance(src, { LinkedCode });
  });

  test("inline regex with quotes in attribute expression (#112)", async () => {
    const Tag = (props: any) => createElement("span", null, String(props.re));
    await assertMdxConformance(`<Tag re={/a="b"/g} />`, { Tag });
  });

  test("different self-closing component inside template-literal attribute (#74)", async () => {
    const CodePreview = (props: any) =>
      createElement("figure", null, createElement("pre", null, props.code), props.children);
    const CodeBlock = (props: any) => createElement("span", null, String(props.lineStart));
    const src = [
      "<CodePreview",
      "  code={`<CodeBlock",
      "    lineStart={1505}",
      "    showLineNumbers",
      "/>`}",
      '  lang="astro"',
      ">",
      "  <CodeBlock lineStart={1505} showLineNumbers />",
      "</CodePreview>",
    ].join("\n");
    await assertMdxConformance(src, { CodePreview, CodeBlock });
  });

  test("JSX element/fragment/conditional in attribute expression (#119)", async () => {
    const Slot = (props: any) => createElement("div", null, props.d);
    await assertMdxConformance("<Slot d={<p>hi there</p>} />", { Slot });
    await assertMdxConformance("<Slot d={<>hi</>} />", { Slot });
    await assertMdxConformance("<Slot d={true ? <a>x</a> : <b>y</b>} />", { Slot });
  });

  test("quotes in JSX text inside attribute expression (#119)", async () => {
    const Slot = (props: any) => createElement("div", null, props.d);
    await assertMdxConformance("<Slot d={<p>a<b>x</b>'s</p>} />", { Slot });
    await assertMdxConformance("<Slot d={<p>Acme Corp.'s view</p>} />", { Slot });
    await assertMdxConformance('<Slot d={<p>a "!?" badge here</p>} />', { Slot });
  });

  // Keep the space between text nodes so HTML normalization cannot erase it.
  test("significant whitespace between JSX elements in attribute expression (#129)", async () => {
    const Slot = (props: any) => createElement("div", null, props.d);
    const Pass = (props: any) => props.children;
    await assertMdxConformance("<Slot d={<><x>a</x> <y>b</y></>} />", { Slot, x: Pass, y: Pass });
    await assertMdxConformance("<Slot d={<>a<em> </em>b</>} />", { Slot, em: Pass });
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

  test("image alt with expression body", async () => {
    await assertMdxConformance("![{1+2}](https://x.test/i.png)");
    await assertMdxConformance("![pre {x} mid {y} end](https://x.test/i.png)");
  });

  test("`{` inside link URL is literal text", async () => {
    await assertMdxConformance("[a]({foo})");
    await assertMdxConformance("[a]({1+2})");
    await assertMdxConformance("[a](b{c}d)");
    await assertMdxConformance("[a]({)");
  });

  test("unmatched `(` after `]` doesn't suppress `{` expression scan", async () => {
    await assertBothReject("[>>](}{{");
  });

  test("unclosed link title with `{` falls through to expression scan", async () => {
    await assertBothReject('[link](/uri "ti{w)');
    await assertBothReject('\\\n     bar\n[link](/uri "ti\0{w)');
  });

  test("`{` inside a parenthesized link title is literal text", async () => {
    await assertMdxConformance("[a](/u (title{w))");
    await assertMdxConformance("[a](/u (ti(tle{w))");
  });

  test("a paren title closes at its first `)`, with or without a nested `(`", async () => {
    await assertMdxConformance("[a](/u (ti(tle))");
    await assertMdxConformance("[a](/u (t) {1})");
  });

  test("`{` where a link title should open is an expression", async () => {
    await assertBothReject("[a](/u {w)");
  });

  test("a `)` before the `{` that isn't the tail close keeps the tail", async () => {
    await assertMdxConformance("[a](\\){)");
    await assertMdxConformance('[a](/u "){")');
    await assertMdxConformance("[a](<){>)");
    await assertMdxConformance("![a](\\){)");
    await assertMdxConformance("[a](/u (a\\){b))");
  });

  test("a `](` inside a title doesn't hide the tail that encloses it", async () => {
    await assertMdxConformance('[a](/u "b](c {z")');
    await assertMdxConformance("[a](/u (b](c {z))");
    await assertMdxConformance('[a](/u "b](c {z}")');
  });

  test("a tail that closes before the `{` leaves it an expression", async () => {
    await assertMdxConformance("[a](\\){)}");
    await assertBothReject("[a](/u){w");
  });

  test("a resource spanning lines keeps its `{` literal", async () => {
    await assertMdxConformance("[a]({{{\n)");
    await assertMdxConformance('[a](/u{\n"t")');
    await assertMdxConformance('[a](/u "ti{\ntle")');
    await assertMdxConformance('x\n[a](/u\n"ti{tle") y');
  });

  test("a resource opening on an earlier line still encloses the `{`", async () => {
    await assertMdxConformance('[a](/u\n"ti{tle")');
    await assertMdxConformance("[a](\nu{v})");
    await assertMdxConformance("[a](}\n({))");
    await assertMdxConformance('[a](/u "ti\ntl{e")');
    await assertMdxConformance('![a](/u\n"ti{tle")');
    await assertMdxConformance('[a](/u\r\n"ti{tle")');
    await assertMdxConformance('> [a](/u\n> "ti{tle")');
  });

  test("a block boundary ends the resource, leaving the `{` an expression", async () => {
    await assertBothReject('[a](/u "a{\n# b")');
    await assertBothReject('[a](/u "a{\n``` b")');
    await assertBothReject('[a](/u "a{\n***\nb")');
    await assertBothReject('[a](/u "a{\n- b")');
    await assertBothReject('[a](/u "a{\n\nb")');
    await assertBothReject('[a](/u "a{\n> b")');
  });

  test("a line ending crosses neither a destination nor a closed tail", async () => {
    await assertBothReject("[a](/u{\nmore)");
    await assertBothReject("[a](/u\n) {w");
  });

  test("a block boundary keeps a valid `{}` an expression, not title text", async () => {
    await assertMdxConformance('[a](/u "ti{1+1}\n# tle")');
    await assertMdxConformance('[a](/u "ti{1+1}\n``` tle")');
    await assertMdxConformance('[a](/u "ti{1+1}\n- tle")');
    await assertMdxConformance('[a](/u "ti{1+1}\n> tle")');
    await assertMdxConformance('[a](/u "ti{1+1}\n***\ntle")');
    await assertMdxConformance('[a](/u "ti{1+1}\n\ntle")');
  });

  test("an escaped label opens no tail, so the `{}` stays an expression", async () => {
    await assertMdxConformance('\\[a](/u "ti{1+1}\ntle")');
    await assertMdxConformance('x\\[a](/u "ti{1+1}\ntle")');
    await assertMdxConformance('\\[a](/u "ti{1+1}tle")');
    await assertMdxConformance('a\\](/u "ti{1+1}\ntle")');
  });

  // This reference has no GFM support, so it cannot validate footnote behavior.
  test("a footnote definition ends the resource, keeping the `{}` an expression", () => {
    const tree = mdxToMdast('[a](/u "ti{1+1}\n[^a]: tle")') as { children: unknown[] };
    const types: string[] = [];
    const walk = (n: { type?: string; children?: unknown[] }) => {
      if (n.type) types.push(n.type);
      for (const c of n.children ?? []) walk(c as { type?: string; children?: unknown[] });
    };
    walk(tree as { children?: unknown[] });
    expect(types).toContain("mdxTextExpression");
    expect(types).toContain("footnoteDefinition");
  });

  test("an escaped bracket is not a label delimiter, so no tail forms", async () => {
    await assertBothReject("\\[a](\\){)");
    await assertBothReject("[\\](\\){)");
    await assertBothReject("[a\\](\\){)");
    await assertBothReject("!\\[a](\\){)");
    await assertBothReject('\\[a](/u "b](c {z")');
  });

  test("a `[` inside a code span is not a label start", async () => {
    await assertBothReject("`[a`](\\){)");
    await assertBothReject('`x[y` a](/u "b](c {z")');
    await assertMdxConformance("`[a` [b](/u{z})");
  });

  test("a label start is consumed by the first `]`, not reused", async () => {
    await assertBothReject("[a](/u) b](\\){)");
    await assertMdxConformance("[a](/u) [b](\\){)");
  });

  test("an inner link deactivates the label starts around it", async () => {
    await assertBothReject("[[a](/u)](\\){)");
    await assertBothReject("[x [a](/u) y](\\){)");
    await assertMdxConformance("[![a](/i)](/u{z})");
    await assertMdxConformance("[![a](/i)](\\){)");
  });

  test("a `{` inside a code span stays code text", async () => {
    await assertMdxConformance("[a(`](!{{`[`})");
    await assertMdxConformance("`a](b {c`");
  });

  test("a label start on an earlier line of the paragraph still counts", async () => {
    await assertMdxConformance("[x\n\\[a]({)");
    await assertMdxConformance("[a\n\\[](\\){)");
    await assertMdxConformance("> [x\n\\[a]({)");
  });

  test("many malformed tails before the `{` do not change its meaning", async () => {
    await assertMdxConformance("[a](x ".repeat(32) + "[b](/u{)");
    await assertMdxConformance("[a](x ".repeat(33) + "[b](/u{)");
  });

  test("a label start in an earlier block does not open a tail", async () => {
    await assertBothReject("# h [x\n](\\){)");
    await assertBothReject("# h [x\n\\[a](\\){)");
    await assertBothReject("---\n](\\){)");
    await assertBothReject("```\nfence [x\n```\n](\\){)");
  });

  test("a backtick closing an earlier line's span doesn't open a new one", async () => {
    await assertMdxConformance("`x\n`a](b{1}`");
    await assertMdxConformance("`x\n`a](b{1}`y");
    await assertMdxConformance("``x\n``a](b{1}``");
  });

  test("a line of links with braces parses like any other", async () => {
    await assertMdxConformance("[a](/u{x}) ".repeat(20));
  });

  test("an earlier line holding an open construct withholds its label start", async () => {
    await assertBothReject("<div>html [x</div>\n]({)");
    await assertBothReject("[`\n{`)]({)");
    await assertBothReject("[`\n<`](><\\]`{! )");
  });

  test("an earlier line whose span closes here still lends its label start", async () => {
    await assertMdxConformance("[`{\n`](}u{>!`{}[\\\\)");
    await assertMdxConformance("[`x`\n\\[a]({)");
  });

  test("an angle bracket that cannot open a tag still lends its label start", async () => {
    await assertMdxConformance("See the 5 < 6\n[a\nb](/u{z)");
    await assertMdxConformance("a < b [x](/u{z)");
    await assertBothReject("See a<3\n[a\nb](/u{z)");
  });

  test("a label start earlier in the same paragraph does open one", async () => {
    await assertMdxConformance("[x\n\\[a]({)");
    await assertMdxConformance("> [x\n\\[a]({)");
    await assertMdxConformance("- [x\n\\[a]({)");
  });

  test("a title jammed against a pointy destination leaves the `<` to JSX", async () => {
    await assertBothReject("[a](<>())");
    await assertBothReject('[a](<x>"")');
    await assertBothReject('[a](<}>"")');
    await assertBothReject("[a](</>'')");
    await assertBothReject("![a](<>())");
    await assertBothReject('[a]( <>"")');
  });

  test("a separated title still forms a link and `< >` stays text", async () => {
    await assertMdxConformance('[a](<x> "t")');
    await assertMdxConformance('[a](<x>\n"t")');
    await assertMdxConformance("[a](<x>)");
    await assertMdxConformance('[a](/u"t")');
    await assertMdxConformance("[a](< >())");
    await assertMdxConformance('[a](<u> "t") and [b](<v>) end');
  });

  test("inline `<div>...\\n.../</div>` with trailing text matches reference", async () => {
    await assertMdxConformance("pre<div>xxx</div>after");
    await assertMdxConformance("pre<div>\nxxx\n</div>after");
  });

  test("multi-line expression body inside heading rejects", async () => {
    await assertBothReject("# {1 +\n2}q");
    await assertBothReject("## {a\nb}");
  });

  test("trailing text after `</Name>` on a flow line rejects", async () => {
    await assertBothReject("<Foo>\n</Foo>X");
    await assertBothReject("<Foo>\n</Foo>3c");
    await assertBothReject("<Box>\n  child\n</Box>3c");
    await assertBothReject("<Foo>\nbar</Foo>");
    await assertBothReject("<Foo>\nbar</Foo>baz");
  });

  test("JSX opened in blockquote without proper continuation rejects", async () => {
    await assertBothReject("><Box>\n  child\n</Box>");
    await assertBothReject("> <Box>\n  child\n</Box>");
    await assertBothReject("- <Box>\n  child");
  });

  test("JSX inside blockquote with proper `>` continuation accepts", async () => {
    await assertMdxConformance("> <Box>\n>   child\n> </Box>", { Box });
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
  test("dollar-prefixed component name does not produce phantom attribute", async () => {
    const $Foo = (props: any) => createElement("span", null, `bar=${props.bar}`);
    await assertMdxConformance("text <$Foo bar={1}/> end", { $Foo });
  });

  test("division after regex close is not parsed as a new regex", async () => {
    await assertMdxConformance("{ /a/.source.length / 2 }");
  });

  test("division after object literal close is not parsed as a regex", async () => {
    await assertMdxConformance("{ ({a: 1}.a) / 2 }");
  });

  test("inline expression continuation tab normalises to spaces", async () => {
    await assertMdxConformance("text {1 +\n\t2} end");
  });

  test("self-closing JSX with newline before `>` is recognised", async () => {
    await assertMdxConformance("<g/\n>");
  });
  test("self-closing JSX with space before `>` is recognised", async () => {
    await assertMdxConformance("text <utj/ >/ rest");
  });

  test("inline expression in blockquote strips `>` from value", async () => {
    await assertMdxConformance("> {1 +\n> 2}");
  });

  test("inline expression in blockquote can close on a lazy line", async () => {
    await assertMdxConformance("> ]{\n}n");
  });

  test("self-closing JSX with tab inside, then trailing text, stays inline", async () => {
    await assertMdxConformance("<y/\t>/");
  });

  test("JSX member chain with empty segment is rejected", async () => {
    await assertBothReject("<a..b/>");
  });
  test("JSX member chain with digit segment is rejected", async () => {
    await assertBothReject("<a.1/>");
  });
  test("JSX namespace mixed with member chain is rejected", async () => {
    await assertBothReject("<a:b.c/>");
  });

  test("attribute name starting with digit is rejected", async () => {
    await assertBothReject("<a 1x/>");
  });
  test("attribute name with operator chars is rejected", async () => {
    await assertBothReject("<a x!=1/>");
  });
  test("bare-word attribute value is rejected", async () => {
    await assertBothReject("<a x=foo/>");
  });

  test("closing tag with attributes is rejected", async () => {
    await assertBothReject("<a></a foo/>");
  });

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
    await assertMdxConformance("<\n}");
  });
  test("fragment `<\\t>` followed by trailing punctuation parses", async () => {
    await assertMdxConformance("<\t>}x#");
  });

  test("expression body `{h<}` is rejected at parse time", async () => {
    await assertBothReject("{h<}");
  });
  test("expression body `{}/m` (object divided by m) is accepted", async () => {
    await assertMdxConformance("#{{}/2}*");
  });
  test("regex literal in expression body followed by newline+tab+close", async () => {
    await assertMdxConformance("{!/^=/\n\t}>");
  });
  test("regex then division in expression body parses without consuming close", async () => {
    await assertMdxConformance("4{/]//5}");
  });

  test("text-position expression accepts lazy continuation in blockquote", async () => {
    await assertMdxConformance(">-{\n42}");
  });

  test("flow-position expression rejects lazy line even when only the close is on it", async () => {
    await assertBothReject(">{\n}");
  });

  test("bare `<` followed by setext underline rejects", async () => {
    await assertBothReject("<\n-");
  });
  test("bare `<` followed by setext underline (=) rejects", async () => {
    await assertBothReject("<\n=");
  });
  test("bare `<` followed by repeated setext underline rejects", async () => {
    await assertBothReject("<\n--");
  });

  test("bare `<` followed by `=` inside blockquote stays text", async () => {
    await assertMdxConformance(">z<\n=");
  });

  test("tab-indented `>` continues open blockquote", async () => {
    await assertMdxConformance(">a\n\t>b");
  });
  test("blockquote with tab-indented `>` after blank `>` line", async () => {
    await assertMdxConformance(">ex\n\t \t>");
  });
  test("blockquote followed by tab-indented `>` fragment", async () => {
    await assertMdxConformance("c>l}>\n>\n\t>");
  });

  test("empty list marker inside blockquote after preceding paragraph", async () => {
    await assertMdxConformance("_\n>\n>-");
  });
  test("empty list marker inside nested blockquote after preceding paragraph", async () => {
    await assertMdxConformance("_>>>\n>\n>-");
  });

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

  test("comment-only expression body is accepted", async () => {
    await assertMdxConformance("{/* foo */}");
  });
  test("whitespace-only expression body is accepted", async () => {
    await assertMdxConformance("{ }");
  });

  test("bare `<` followed by newline + blockquote prefix stays as text", async () => {
    await assertMdxConformance(">/<\n>}v\n");
  });

  test("self-closing JSX `<x/\\n>` followed by trailing content", async () => {
    const _ = () => null;
    await assertMdxConformance("<_/\n>>", { _ });
  });

  test("text-position expression dedents trailing tab before close", async () => {
    await assertMdxConformance(">o{1+2\n\t}}");
  });
});

describe("MDX conformance: math interaction", () => {
  test("braces inside inline math are not an expression (#110)", async () => {
    await assertMdxMathConformance("$\\frac{-b}{2a}$ and {1 + 1}");
  });

  test("expression between dollar amounts is math text", async () => {
    await assertMdxMathConformance("Price is $5 and {x} costs $10 today");
  });

  test("expression after a real math span is evaluated", async () => {
    await assertMdxMathConformance("Euler $e^{i\\pi}$ then {3 * 7}");
  });

  test("`<` inside math does not suppress a following blockquote", async () => {
    await assertMdxMathConformance("$<$\n>");
  });

  test("brace before a span-closing escaped dollar is math text", async () => {
    await assertMdxMathConformance("e$}}_{\\$h");
  });

  test("inline `$$` does not pair across a display-math fence", async () => {
    await assertMdxMathConformance("See:$$\n\\frac{1}{2}\n$$");
  });
});

describe("MDX conformance: development positions", () => {
  test("ascii source", async () => {
    await assertMdxDevPositionConformance("ascii <Foo />\n");
  });

  test("multibyte characters before a JSX element", async () => {
    await assertMdxDevPositionConformance("# Café été\n\né <Foo />\n");
  });

  test("astral characters count as two columns", async () => {
    await assertMdxDevPositionConformance("😀 <Foo />\n");
  });

  test("astral characters on an earlier line", async () => {
    await assertMdxDevPositionConformance("😀 emoji\n\n<Foo>🎉 <Bar /></Foo>\n");
  });

  test("__source column matches the mdast position", () => {
    const source = "# Café été\n\né <Foo />\n";
    const tree = mdxToMdast(source);
    if (tree.type !== "root") throw new Error("expected a root");
    const paragraph = tree.children[1];
    if (paragraph?.type !== "paragraph") throw new Error("expected a paragraph");
    const jsx = paragraph.children.find((child) => child.type === "mdxJsxTextElement");
    const { code } = mdxToJs(source, { development: true });
    expect(code).toContain(`columnNumber: ${jsx?.position?.start.column}`);
  });

  test("parse error column counts UTF-16 code units", () => {
    expect(() => mdxToJs("😀 <Foo>\n")).toThrow(/^1:4: Expected a closing tag for `<Foo>` \(1:4\)/);
  });
});

// Declared locally because `_mdxExplicitJsx` is private to the Data interfaces.
interface ExplicitJsxData extends MdxJsxFlowElementData {
  _mdxExplicitJsx: true;
}
const explicitJsxData: ExplicitJsxData = { _mdxExplicitJsx: true };

interface TreeNode {
  type: string;
  tagName?: string;
  children?: unknown[];
}

function mapChildren(tree: TreeNode, fn: (node: TreeNode) => unknown): void {
  if (!tree.children) return;
  const next: unknown[] = [];
  for (const child of tree.children as TreeNode[]) {
    mapChildren(child, fn);
    const replacement = fn(child);
    if (Array.isArray(replacement)) next.push(...replacement);
    else next.push(replacement ?? child);
  }
  tree.children = next;
}

const widget = (name: string, explicit: boolean): MdxJsxFlowElement => {
  const node: MdxJsxFlowElement = {
    type: "mdxJsxFlowElement",
    name,
    attributes: [{ type: "mdxJsxAttribute", name: "foo", value: "bar" }],
    children: [],
  };
  if (explicit) node.data = explicitJsxData;
  return node;
};

const insertedBuilding = {
  type: "mdxJsxFlowElement",
  name: "Building",
  attributes: [],
  children: [],
  data: explicitJsxData,
} satisfies MdxJsxFlowElement;

const highlightedPre = () =>
  ({
    type: "element",
    tagName: "pre",
    properties: { className: ["shiki"] },
    children: [{ type: "text", value: "highlighted" }],
  }) satisfies Element;

describe("MDX conformance: plugin-inserted explicit JSX", () => {
  const Building = () => createElement("aside", null, "building");
  const OverriddenPre = (props: any) =>
    createElement("pre", { "data-from": "components" }, props.children);
  const OverriddenWidget = () => createElement("span", null, "overridden");

  test("one explicit inserted node doesn't stop the others reaching _components", async () => {
    await assertMdxPluginConformance(
      "# Title\n\n```js\nconsole.log('hi');\n```\n\nSome paragraph.\n",
      {
        reference: {
          remarkPlugins: [
            () => (tree: TreeNode) =>
              mapChildren(tree, (node) =>
                node.type === "paragraph" ? insertedBuilding : undefined,
              ),
          ],
          rehypePlugins: [
            () => (tree: TreeNode) =>
              mapChildren(tree, (node) =>
                node.type === "element" && node.tagName === "pre" ? highlightedPre() : undefined,
              ),
          ],
        },
        satteri: {
          mdastPlugins: [
            defineMdastPlugin({
              name: "insert-explicit",
              paragraph(node, ctx) {
                ctx.replaceNode(node, insertedBuilding);
              },
            }),
          ],
          hastPlugins: [
            defineHastPlugin({
              name: "highlight",
              element: {
                filter: ["pre"],
                visit(node, ctx) {
                  ctx.replaceNode(node, highlightedPre());
                },
              },
            }),
          ],
        },
        components: { Building, pre: OverriddenPre },
      },
    );
  });

  test("explicit JSX is honoured per node, not per span", async () => {
    await assertMdxPluginConformance("# hi\n\npara\n", {
      reference: {
        remarkPlugins: [
          () => (tree: TreeNode) =>
            mapChildren(tree, (node) => {
              if (node.type === "paragraph") return widget("my-widget", true);
              if (node.type === "heading") return [node, widget("other-widget", false)];
              return undefined;
            }),
        ],
      },
      satteri: {
        mdastPlugins: [
          defineMdastPlugin({
            name: "insert-widgets",
            paragraph(node, ctx) {
              ctx.replaceNode(node, widget("my-widget", true));
            },
            heading(node, ctx) {
              ctx.insertAfter(node, widget("other-widget", false));
            },
          }),
        ],
      },
      components: { "my-widget": OverriddenWidget, "other-widget": OverriddenWidget },
    });
  });
});
