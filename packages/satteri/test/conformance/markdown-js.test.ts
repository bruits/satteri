import { describe, test, expect } from "vitest";
import { createElement } from "react";
import { evaluate as mdxEvaluate } from "@mdx-js/mdx";
import { renderToStaticMarkup } from "react-dom/server";
import * as runtime from "react/jsx-runtime";
import { assertMarkdownJsConformance } from "./helpers.js";
import { markdownToJs } from "../../src/index.js";

describe("markdownToJs conformance: MDX expression syntax stays literal", () => {
  test("flow expression position", async () => {
    await assertMarkdownJsConformance("{1 + 2}");
  });

  test("inline expression in paragraph", async () => {
    await assertMarkdownJsConformance("result: {1 + 2}");
  });

  test("identifier in braces", async () => {
    await assertMarkdownJsConformance("Hello {xxx} world");
  });

  test("comment expression syntax", async () => {
    await assertMarkdownJsConformance("{/* comment */}");
  });

  test("empty braces", async () => {
    await assertMarkdownJsConformance("{}");
  });

  test("spread syntax", async () => {
    await assertMarkdownJsConformance("{...props}");
  });

  test("unbalanced open brace", async () => {
    await assertMarkdownJsConformance("a { b");
  });

  test("lone close brace", async () => {
    await assertMarkdownJsConformance("a } b");
  });

  test("double braces", async () => {
    await assertMarkdownJsConformance("{{a: 1}}");
  });

  test("multi-line braces without blank line", async () => {
    await assertMarkdownJsConformance("{1 +\n2}");
  });

  test("braces spanning a blank line", async () => {
    await assertMarkdownJsConformance("{a +\n\nb}");
  });

  test("braces in heading", async () => {
    await assertMarkdownJsConformance("# {title}");
  });

  test("braces in blockquote", async () => {
    await assertMarkdownJsConformance("> {quote}");
  });

  test("braces in list item", async () => {
    await assertMarkdownJsConformance("- {item}\n- plain");
  });

  test("braces in link text", async () => {
    await assertMarkdownJsConformance("[a {b}](https://example.com)");
  });

  test("braces in image alt", async () => {
    await assertMarkdownJsConformance("![{alt}](https://example.com/i.png)");
  });

  test("braces in link title", async () => {
    await assertMarkdownJsConformance('[a](https://example.com "{t}")');
  });

  test("braces in inline code", async () => {
    await assertMarkdownJsConformance("`{code}`");
  });

  test("braces in fenced code", async () => {
    await assertMarkdownJsConformance("```\n{not(code)}\n```");
  });

  test("braces immediately after inline code", async () => {
    await assertMarkdownJsConformance("`code`{' suffix'}");
  });

  test("braces inside emphasis", async () => {
    await assertMarkdownJsConformance("*{x}*");
  });

  test("template-literal-shaped text", async () => {
    await assertMarkdownJsConformance("cost ${price} here");
  });
});

describe("markdownToJs conformance: ESM syntax stays literal", () => {
  test("import statement", async () => {
    await assertMarkdownJsConformance('import x from "y"');
  });

  test("import with named bindings", async () => {
    await assertMarkdownJsConformance("import {a, b} from 'mod'");
  });

  test("export const", async () => {
    await assertMarkdownJsConformance("export const a = 1");
  });

  test("export default function", async () => {
    await assertMarkdownJsConformance("export default function () {}");
  });

  test("export named", async () => {
    await assertMarkdownJsConformance("export { a }");
  });

  test("import line followed by content", async () => {
    await assertMarkdownJsConformance("import x from 'y'\n\nreal text");
  });
});

describe("markdownToJs conformance: JSX-like input is raw HTML", () => {
  // Intentional divergence in default mode: the reference silently strips
  // unknown raw HTML (keeping inner text), satteri refuses to compile it.
  // Pin both so a change on either side is noticed.
  test("default mode: reference strips raw HTML, satteri throws", async () => {
    const input = "a <b>bold</b> word";
    const { default: MdxComponent } = (await mdxEvaluate(input, {
      ...(runtime as object),
      format: "md",
    } as Parameters<typeof mdxEvaluate>[1])) as { default: React.FC };
    expect(renderToStaticMarkup(createElement(MdxComponent))).toBe("<p>a bold word</p>");
    expect(() => markdownToJs(input)).toThrow(/rawHtml/);
  });

  test("inline element", async () => {
    await assertMarkdownJsConformance("a <b>bold</b> word", { rawHtml: true });
  });

  test("component-cased tag is a lowercased element", async () => {
    await assertMarkdownJsConformance("<MyComponent />", { rawHtml: true });
  });

  test("JSX-style attribute braces are literal attribute text", async () => {
    await assertMarkdownJsConformance("<Foo bar={1}/>", { rawHtml: true });
  });

  test("nested inline elements", async () => {
    await assertMarkdownJsConformance('before <span class="x">in <em>deep</em></span> after', {
      rawHtml: true,
    });
  });

  test("void element without slash", async () => {
    await assertMarkdownJsConformance("line<br>break", { rawHtml: true });
  });

  test("block-level html", async () => {
    await assertMarkdownJsConformance("<div>\n  <p>hi</p>\n</div>", { rawHtml: true });
  });

  test("markdown inside block html", async () => {
    await assertMarkdownJsConformance("<div>\n\n*em*\n\n</div>", { rawHtml: true });
  });

  test("html comment", async () => {
    await assertMarkdownJsConformance("a <!-- note --> b", { rawHtml: true });
  });

  test("details and summary", async () => {
    await assertMarkdownJsConformance("<details><summary>more</summary>body</details>", {
      rawHtml: true,
    });
  });

  test("attribute value containing braces", async () => {
    await assertMarkdownJsConformance('<span title="{x}">t</span>', { rawHtml: true });
  });

  test("fragment syntax is not a tag", async () => {
    await assertMarkdownJsConformance("<>hello</>", { rawHtml: true });
  });
});

describe("markdownToJs conformance: core Markdown", () => {
  test("atx headings", async () => {
    await assertMarkdownJsConformance("# h1\n\n## h2\n\n###### h6");
  });

  test("setext headings", async () => {
    await assertMarkdownJsConformance("Title\n=====\n\nSub\n-----");
  });

  test("nested emphasis", async () => {
    await assertMarkdownJsConformance("***both*** and **strong _em_**");
  });

  test("link with title", async () => {
    await assertMarkdownJsConformance('[a](https://example.com "title")');
  });

  test("reference link", async () => {
    await assertMarkdownJsConformance("[a][r]\n\n[r]: https://example.com");
  });

  test("image with alt and title", async () => {
    await assertMarkdownJsConformance('![alt](https://example.com/i.png "title")');
  });

  test("autolink", async () => {
    await assertMarkdownJsConformance("<https://example.com>");
  });

  test("gfm autolink literal", async () => {
    await assertMarkdownJsConformance("visit www.example.com now");
  });

  test("inline code with inner backtick", async () => {
    await assertMarkdownJsConformance("``a ` b``");
  });

  test("fenced code with language", async () => {
    await assertMarkdownJsConformance("```js\nconst a = {x: 1};\n```");
  });

  test("fenced code containing MDX syntax", async () => {
    await assertMarkdownJsConformance("```mdx\n<Foo bar={1}/>\nimport x from 'y'\n```");
  });

  test("indented code", async () => {
    await assertMarkdownJsConformance("    indented code\n    line two");
  });

  test("nested blockquote", async () => {
    await assertMarkdownJsConformance("> a\n>\n> > b");
  });

  test("nested lists", async () => {
    await assertMarkdownJsConformance("1. one\n   - a\n   - b\n2. two");
  });

  test("loose list", async () => {
    await assertMarkdownJsConformance("- a\n\n- b");
  });

  test("thematic break", async () => {
    await assertMarkdownJsConformance("a\n\n---\n\nb");
  });

  test("hard breaks", async () => {
    await assertMarkdownJsConformance("back\\\nslash and two  \nspaces");
  });

  test("backslash escapes", async () => {
    await assertMarkdownJsConformance("\\*not em\\* and \\{literal\\}");
  });

  test("entities", async () => {
    await assertMarkdownJsConformance("&amp; &lt; &copy; &#35;");
  });

  test("table with alignment", async () => {
    await assertMarkdownJsConformance("| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |");
  });

  test("strikethrough", async () => {
    await assertMarkdownJsConformance("~~gone~~ kept");
  });

  test("task list", async () => {
    await assertMarkdownJsConformance("- [x] done\n- [ ] todo");
  });

  test("footnote with repeated reference", async () => {
    await assertMarkdownJsConformance("text[^1] and again[^1]\n\n[^1]: note");
  });
});

describe("markdownToJs conformance: text survives JS codegen escaping", () => {
  test("double and single quotes", async () => {
    await assertMarkdownJsConformance("She said \"hi\" and it's fine");
  });

  test("backslashes in text", async () => {
    await assertMarkdownJsConformance("path C:\\\\dir\\\\file");
  });

  test("template-injection-shaped text", async () => {
    await assertMarkdownJsConformance("${process.exit(1)} stays text");
  });

  test("line separator U+2028", async () => {
    await assertMarkdownJsConformance("a b");
  });

  test("paragraph separator U+2029", async () => {
    await assertMarkdownJsConformance("a b");
  });

  test("unicode text", async () => {
    await assertMarkdownJsConformance("emoji 🎉 CJK 你好 RTL שלום combining é");
  });
});

describe("markdownToJs conformance: frontmatter", () => {
  test("yaml frontmatter is stripped", async () => {
    await assertMarkdownJsConformance("---\ntitle: x\n---\n\n# Body", { frontmatter: true });
  });

  test("toml frontmatter is stripped", async () => {
    await assertMarkdownJsConformance('+++\ntitle = "x"\n+++\n\nBody', { frontmatter: true });
  });

  test("delimiters mid-document are not frontmatter", async () => {
    await assertMarkdownJsConformance("text\n\n---\ntitle: x\n---", { frontmatter: true });
  });
});

describe("markdownToJs conformance: components mapping", () => {
  test("h1 override", async () => {
    const h1 = (props: any) => createElement("h1", { className: "custom" }, props.children);
    await assertMarkdownJsConformance("# Hello {x}", { components: { h1 } });
  });

  test("link override receives href", async () => {
    const a = (props: any) =>
      createElement("a", { href: props.href, "data-ext": true }, props.children);
    await assertMarkdownJsConformance("[text](https://example.com)", { components: { a } });
  });

  test("code override receives className", async () => {
    const code = (props: any) =>
      createElement("code", { className: `${props.className ?? ""} hl` }, props.children);
    await assertMarkdownJsConformance("```js\n1\n```", { components: { code } });
  });

  test("wrapper layout", async () => {
    const wrapper = (props: any) => createElement("main", null, props.children);
    await assertMarkdownJsConformance("# In layout", { components: { wrapper } });
  });
});
