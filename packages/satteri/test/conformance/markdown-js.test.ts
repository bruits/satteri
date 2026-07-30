import { describe, test } from "vitest";
import { createElement } from "react";
import {
  assertMarkdownJsConformance,
  assertMarkdownJsDevPositionConformance,
  assertMarkdownJsModuleConformance,
} from "./helpers.js";

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

// The reference drops it too, with no rehype-raw installed.
describe("markdownToJs conformance: raw HTML is dropped by default", () => {
  test("inline element", async () => {
    await assertMarkdownJsConformance("a <b>bold</b> word");
  });

  test("standalone html comment", async () => {
    await assertMarkdownJsConformance("<!-- prettier-ignore -->");
  });

  test("html comment between paragraphs", async () => {
    await assertMarkdownJsConformance("text\n\n<!-- note -->\n\nmore");
  });

  test("inline html comment", async () => {
    await assertMarkdownJsConformance("a <!-- inline --> b");
  });

  test("inline element with attributes", async () => {
    await assertMarkdownJsConformance('<span class="x">y</span>');
  });

  test("void element without slash", async () => {
    await assertMarkdownJsConformance("line<br>break");
  });

  test("semantic inline element", async () => {
    await assertMarkdownJsConformance("Press <kbd>Ctrl</kbd>");
  });

  test("block-level html wrapping markdown", async () => {
    await assertMarkdownJsConformance("<div>\n\n*em*\n\n</div>");
  });

  test("block-level html wrapping html", async () => {
    await assertMarkdownJsConformance("<div>\n  <p>hi</p>\n</div>");
  });

  test("details and summary", async () => {
    await assertMarkdownJsConformance("<details><summary>more</summary>body</details>");
  });

  test("component-cased tag", async () => {
    await assertMarkdownJsConformance("<MyComponent />");
  });

  test("document that is only html", async () => {
    await assertMarkdownJsConformance("<div></div>");
  });
});

describe("markdownToJs conformance: JSX-like input is raw HTML", () => {
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
    await assertMarkdownJsConformance('She said "hi" and it\'s fine');
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

// The comparisons above only see the rendered tree; these see the module the
// JS-output options actually shape.
describe("markdownToJs conformance: compiled module envelope", () => {
  const src = "# Head\n\ntext with a [link](https://e.com)\n";

  test("program output", async () => {
    await assertMarkdownJsModuleConformance(src);
  });

  test("jsxImportSource", async () => {
    await assertMarkdownJsModuleConformance(src, { jsxImportSource: "preact" });
  });

  test("classic runtime", async () => {
    await assertMarkdownJsModuleConformance(src, { jsxRuntime: "classic" });
  });

  test("classic runtime with custom pragmas", async () => {
    await assertMarkdownJsModuleConformance(src, {
      jsxRuntime: "classic",
      pragma: "h",
      pragmaFrag: "Fragment",
      pragmaImportSource: "preact",
    });
  });

  test("development imports the dev runtime", async () => {
    await assertMarkdownJsModuleConformance(src, { development: true });
  });

  test("providerImportSource", async () => {
    await assertMarkdownJsModuleConformance(src, { providerImportSource: "@mdx-js/react" });
  });

  test("document with frontmatter", async () => {
    await assertMarkdownJsModuleConformance("---\ntitle: x\n---\n\n# Body", { frontmatter: true });
  });

  test("document that compiles to nothing", async () => {
    await assertMarkdownJsModuleConformance("<!-- only a comment -->");
  });

  test("jsx: true carries the automatic runtime pragmas", async () => {
    await assertMarkdownJsModuleConformance(src, { jsx: true });
  });

  test("jsx: true with a custom import source", async () => {
    await assertMarkdownJsModuleConformance(src, { jsx: true, jsxImportSource: "preact" });
  });

  test("jsx: true with the classic runtime", async () => {
    await assertMarkdownJsModuleConformance(src, { jsx: true, jsxRuntime: "classic" });
  });

  test("jsx: true with custom pragmas", async () => {
    await assertMarkdownJsModuleConformance(src, {
      jsx: true,
      jsxRuntime: "classic",
      pragma: "h",
      pragmaFrag: "Fragment",
      pragmaImportSource: "preact",
    });
  });
});

describe("markdownToJs conformance: development positions", () => {
  test("headings and paragraphs", async () => {
    await assertMarkdownJsDevPositionConformance("# Head\n\ntext\n");
  });

  test("inline elements inside a paragraph", async () => {
    await assertMarkdownJsDevPositionConformance("para with *em* and `code`\n");
  });

  test("nested block structures", async () => {
    await assertMarkdownJsDevPositionConformance("> quote\n\n- a\n- b\n");
  });

  test("table", async () => {
    await assertMarkdownJsDevPositionConformance("| a |\n|---|\n| 1 |\n");
  });

  test("indented content", async () => {
    await assertMarkdownJsDevPositionConformance("  # indented\n\n    code block\n");
  });
});

// No KaTeX on either side: math renders as `<code>`/`<pre>` with a language class.
describe("markdownToJs conformance: math", () => {
  test("inline math", async () => {
    await assertMarkdownJsConformance("mass $E = mc^2$ here", { math: true });
  });

  test("display math", async () => {
    await assertMarkdownJsConformance("$$\na + b\n$$", { math: true });
  });

  test("braces inside math stay math, not an expression", async () => {
    await assertMarkdownJsConformance("$\\frac{a}{b}$", { math: true });
  });

  test("dollar signs with math off stay text", async () => {
    await assertMarkdownJsConformance("cost $5 and $6");
  });
});

// Raw HTML is dropped after the plugins run, not at parse time, so a plugin can
// still turn a `raw` node into something renderable. @mdx-js/mdx orders it the
// same way.
describe("markdownToJs conformance: plugins see raw HTML before it is dropped", () => {
  test("inline element", async () => {
    await assertMarkdownJsConformance("a <b>bold</b> word", { rewriteRaw: true });
  });

  test("block-level html", async () => {
    await assertMarkdownJsConformance("<div>\n\n*em*\n\n</div>", { rewriteRaw: true });
  });

  test("html comment", async () => {
    await assertMarkdownJsConformance("text\n\n<!-- note -->\n\nmore", { rewriteRaw: true });
  });

  test("html in a heading", async () => {
    await assertMarkdownJsConformance("# a <b>c</b>", { rewriteRaw: true });
  });

  test("nothing to rewrite when rawHtml already parsed it", async () => {
    await assertMarkdownJsConformance("a <b>bold</b> word", { rawHtml: true, rewriteRaw: true });
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
