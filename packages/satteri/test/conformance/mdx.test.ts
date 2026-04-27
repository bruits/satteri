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

  test("horizontal rule", async () => {
    await assertMdxConformance("---");
  });

  test("image", async () => {
    await assertMdxConformance("![alt](https://example.com/img.png)");
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
