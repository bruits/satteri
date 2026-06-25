import { describe, test, expect } from "vitest";
import {
  markdownToHtml,
  mdxToJs,
  markdownToMdast,
  mdxToMdast,
  markdownToHast,
  mdxToHast,
  defineMdastPlugin,
  defineHastPlugin,
} from "../src/index.js";
import type { HastNode } from "../src/hast/hast-materializer.js";
import type { HastVisitorContext } from "../src/hast/hast-visitor.js";
import type { MdastNode } from "../src/types.js";
import type { Element } from "hast";
import type { MdxJsxTextElementHast } from "../src/mdx-types.js";

describe("frontmatter extraction", () => {
  test("returns null when there is no frontmatter", () => {
    const result = markdownToHtml("# Hello");
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.frontmatter).toBeNull();
  });

  test("returns yaml frontmatter raw value", () => {
    const source = `---\ntitle: Hi\nn: 1\n---\n# Body`;
    const result = markdownToHtml(source);
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.frontmatter).toEqual({ kind: "yaml", value: "title: Hi\nn: 1" });
  });

  test("returns toml frontmatter raw value", () => {
    const source = `+++\ntitle = "Hi"\n+++\n# Body`;
    const result = markdownToHtml(source);
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.frontmatter).toEqual({ kind: "toml", value: 'title = "Hi"' });
  });

  test("frontmatter is null when frontmatter feature is disabled", () => {
    const source = `---\ntitle: Hi\n---\n# Body`;
    const result = markdownToHtml(source, { features: { frontmatter: false } });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.frontmatter).toBeNull();
  });

  test("mdxToJs also returns frontmatter", () => {
    const source = `---\ntitle: MDX Hi\n---\n# Body`;
    const result = mdxToJs(source);
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.frontmatter).toEqual({ kind: "yaml", value: "title: MDX Hi" });
    expect(result.code).toContain("MDXContent");
  });
});

describe("features.superscript / features.subscript", () => {
  test("superscript renders <sup>", () => {
    const result = markdownToHtml("2^10^", { features: { superscript: true } });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain("<p>2<sup>10</sup></p>");
  });

  test("subscript renders <sub>", () => {
    const result = markdownToHtml("H~2~O", { features: { subscript: true } });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain("<p>H<sub>2</sub>O</p>");
  });

  test("both features together", () => {
    const result = markdownToHtml("H~2~O and 2^10^.", {
      features: { superscript: true, subscript: true },
    });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain("<p>H<sub>2</sub>O and 2<sup>10</sup>.</p>");
  });

  test("disabled by default: delimiters stay literal", () => {
    const result = markdownToHtml("H~2~O and 2^10^.");
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).not.toContain("<sub>");
    expect(result.html).not.toContain("<sup>");
  });

  test("mdast exposes superscript / subscript node types", () => {
    const ast = markdownToMdast("H~2~O and 2^10^.", {
      features: { superscript: true, subscript: true },
    });
    expect(ast.type).toBe("root");
    if (ast.type !== "root") return;
    const para = ast.children[0];
    expect(para?.type).toBe("paragraph");
    if (para?.type !== "paragraph") return;
    expect(para.children.map((c) => c.type)).toEqual([
      "text",
      "subscript",
      "text",
      "superscript",
      "text",
    ]);
  });

  test("mdast plugin visits superscript and replacement survives rebuild", () => {
    const plugin = defineMdastPlugin({
      name: "swap-sup-to-sub",
      superscript() {
        return { type: "subscript", children: [{ type: "text", value: "swapped" }] };
      },
    });
    const result = markdownToHtml("a^x^b", {
      features: { superscript: true, subscript: true },
      mdastPlugins: [plugin],
    });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain("<p>a<sub>swapped</sub>b</p>");
  });
});

describe("features.headingAttributes", () => {
  test("emits id and class", () => {
    const result = markdownToHtml("## Heading {#explicit .custom}", {
      features: { headingAttributes: true },
    });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain('<h2 id="explicit" class="custom">Heading</h2>');
  });

  test("emits custom attributes", () => {
    const result = markdownToHtml("# Title {#t data-role=heading flag}", {
      features: { headingAttributes: true },
    });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain('<h1 id="t" data-role="heading" flag="">Title</h1>');
  });

  test("merges shorthand and explicit id/class", () => {
    const result = markdownToHtml("## Heading {.c1 #x class=c2 id=y}", {
      features: { headingAttributes: true },
    });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain('<h2 id="y" class="c1 c2">Heading</h2>');
  });

  test("quoted values keep their spaces", () => {
    const result = markdownToHtml('# Title {data-label="hello world"}', {
      features: { headingAttributes: true },
    });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain('<h1 data-label="hello world">Title</h1>');
  });

  test("disabled by default: attribute block stays literal", () => {
    const result = markdownToHtml("## Heading {#explicit .custom}");
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain("<h2>Heading {#explicit .custom}</h2>");
  });

  test("mdast exposes attributes via data.hProperties", () => {
    const ast = markdownToMdast("## Heading {#explicit .custom}", {
      features: { headingAttributes: true },
    });
    expect(ast.type).toBe("root");
    if (ast.type !== "root") return;
    const heading = ast.children[0];
    expect(heading?.type).toBe("heading");
    if (heading?.type !== "heading") return;
    expect(heading.data?.hProperties).toEqual({ id: "explicit", className: ["custom"] });
  });
});

describe("features.math.singleDollarTextMath", () => {
  test("default keeps single-$ as inline math", () => {
    const result = markdownToHtml("inline $x$ here", { features: { math: true } });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain("language-math");
  });

  test("false keeps single-$ literal but still parses $$..$$", () => {
    const result = markdownToHtml("the deficit grew from $50 to $100 billion", {
      features: { math: { singleDollarTextMath: false } },
    });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain("$50");
    expect(result.html).toContain("$100");
    expect(result.html).not.toContain("language-math");

    const display = markdownToHtml("text $$x^2$$ end", {
      features: { math: { singleDollarTextMath: false } },
    });
    if (display instanceof Promise) throw new Error("expected sync");
    expect(display.html).toContain("language-math");
  });
});

describe("features.gfm.footnotes", () => {
  const SRC = "See[^a] and[^a] again.\n\n[^a]: Shared note.\n";

  test("default emits English strings", () => {
    const result = markdownToHtml(SRC);
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain(">Footnotes<");
    expect(result.html).toContain('aria-label="Back to reference 1"');
    expect(result.html).toContain('aria-label="Back to reference 1-2"');
  });

  test("footnotes: false drops the footnotes section entirely", () => {
    const result = markdownToHtml(SRC, {
      features: { gfm: { footnotes: false } },
    });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).not.toContain(">Footnotes<");
    // Without parsing as footnotes, the `[^a]` text should leak as-is.
    expect(result.html).toContain("[^a]");
  });

  test("footnote options localize label / back-content / back-label", () => {
    const result = markdownToHtml(SRC, {
      features: {
        gfm: {
          footnotes: {
            label: "Notas",
            backContent: "up",
            backLabel: "Volver a {reference}",
          },
        },
      },
    });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain(">Notas<");
    expect(result.html).not.toContain(">Footnotes<");
    expect(result.html).toContain(">up<");
    expect(result.html).toContain('aria-label="Volver a 1"');
    expect(result.html).toContain('aria-label="Volver a 1-2"');
  });

  test("backLabel callback receives (referenceNumber, rerunIndex)", () => {
    const seen: Array<[number, number]> = [];
    const result = markdownToHtml(SRC, {
      features: {
        gfm: {
          footnotes: {
            backLabel: (n, k) => {
              seen.push([n, k]);
              return k > 1 ? `cb n=${n} k=${k}` : `cb n=${n}`;
            },
          },
        },
      },
    });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(seen).toEqual([
      [1, 1],
      [1, 2],
    ]);
    expect(result.html).toContain('aria-label="cb n=1"');
    expect(result.html).toContain('aria-label="cb n=1 k=2"');
  });

  test("backContent callback returns per-backref text", () => {
    const result = markdownToHtml(SRC, {
      features: {
        gfm: {
          footnotes: {
            backContent: (_n, k) => (k === 1 ? "first" : "more"),
          },
        },
      },
    });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain(">first<");
    expect(result.html).toContain(">more<");
  });

  test("can mix template label and callback backLabel", () => {
    const result = markdownToHtml(SRC, {
      features: {
        gfm: {
          footnotes: {
            label: "Notas",
            backLabel: (n, k) => `#${n}.${k}`,
          },
        },
      },
    });
    if (result instanceof Promise) throw new Error("expected sync");
    expect(result.html).toContain(">Notas<");
    expect(result.html).toContain('aria-label="#1.1"');
    expect(result.html).toContain('aria-label="#1.2"');
  });
});

// markdownToHtml - no plugins

describe("markdownToHtml", () => {
  test("basic markdown to HTML", () => {
    const { html } = markdownToHtml("# Hello\n\nWorld");
    expect(html).toContain("<h1>");
    expect(html).toContain("Hello");
    expect(html).toContain("<p>");
    expect(html).toContain("World");
  });

  test("empty string produces empty output", () => {
    const { html } = markdownToHtml("");
    expect(html).toBe("");
  });

  test("inline formatting", () => {
    const { html } = markdownToHtml("**bold** and *italic*");
    expect(html).toContain("<strong>bold</strong>");
    expect(html).toContain("<em>italic</em>");
  });

  test("link renders as anchor", () => {
    const { html } = markdownToHtml("[click](https://example.com)");
    expect(html).toContain('<a href="https://example.com">click</a>');
  });

  test("code block with language", () => {
    const { html } = markdownToHtml("```js\nconsole.log(1)\n```");
    expect(html).toContain('<code class="language-js">');
    expect(html).toContain("console.log(1)");
  });

  // with MDAST plugins only

  test("MDAST plugin removes headings", () => {
    const removeHeadings = defineMdastPlugin({
      name: "remove-headings",
      heading(node, ctx) {
        ctx.removeNode(node);
      },
    });

    const { html } = markdownToHtml("# Title\n\nKeep this", {
      mdastPlugins: [removeHeadings],
    });
    expect(html).not.toContain("<h1>");
    expect(html).not.toContain("Title");
    expect(html).toContain("Keep this");
  });

  test("MDAST plugin replaces text with raw markdown", () => {
    const uppercaseHeadings = defineMdastPlugin({
      name: "uppercase-headings",
      heading(_node) {
        return { raw: "# REPLACED" };
      },
    });

    const { html } = markdownToHtml("# Original\n\npara", {
      mdastPlugins: [uppercaseHeadings],
    });
    expect(html).toContain("REPLACED");
    expect(html).not.toContain("Original");
  });

  test("raw return from a block (paragraph) visitor does not nest a root", () => {
    const replace = defineMdastPlugin({
      name: "replace-para",
      paragraph() {
        return { raw: "Lorem **ipsum** dolor." };
      },
    });

    const { html } = markdownToHtml("placeholder", { mdastPlugins: [replace] });
    expect(html.trim()).toBe("<p>Lorem <strong>ipsum</strong> dolor.</p>");
  });

  test("raw return parsing to several blocks splices them as siblings", () => {
    const replace = defineMdastPlugin({
      name: "expand-para",
      paragraph() {
        return { raw: "# Title\n\nBody." };
      },
    });

    const { html } = markdownToHtml("placeholder", { mdastPlugins: [replace] });
    expect(html).toContain("<h1>Title</h1>");
    expect(html).toContain("<p>Body.</p>");
    expect(html).not.toContain("</h1>\n<root");
  });

  // Option B unwraps only the document root. A raw return into an inline slot
  // (text visitor) still carries the parser's wrapping paragraph — raw is
  // block-level by design — but it must not produce a nested <root>/<p><p>.
  test("raw return from an inline (text) visitor keeps a single paragraph wrapper", () => {
    const decorate = defineMdastPlugin({
      name: "decorate-text",
      text() {
        return { raw: "Lorem **ipsum** dolor." };
      },
    });

    const { html } = markdownToHtml("placeholder", { mdastPlugins: [decorate] });
    // The parser's paragraph survives inside the original paragraph; what must
    // never appear is a literal nested document root.
    expect(html).not.toContain("<root>");
    expect(html).toContain("<strong>ipsum</strong>");
  });

  // with HAST plugins only

  test("HAST plugin adds class to all elements", () => {
    const addClasses = defineHastPlugin({
      name: "add-classes",
      element: {
        filter: [],
        visit(node, ctx) {
          ctx.setProperty(node, "class", "styled");
        },
      },
    });

    const { html } = markdownToHtml("# Hello\n\nWorld", {
      hastPlugins: [addClasses],
    });
    expect(html).toContain('<h1 class="styled">');
    expect(html).toContain('<p class="styled">');
  });

  test("HAST plugin removes elements", () => {
    const removeHeadings = defineHastPlugin({
      name: "remove-h1",
      element: {
        filter: [],
        visit(node, ctx) {
          if (node.tagName === "h1") {
            ctx.removeNode(node);
          }
        },
      },
    });

    const { html } = markdownToHtml("# Gone\n\nStays", {
      hastPlugins: [removeHeadings],
    });
    expect(html).not.toContain("<h1>");
    expect(html).not.toContain("Gone");
    expect(html).toContain("Stays");
  });

  test("HAST plugin replaces element via return value", () => {
    const replaceH1 = defineHastPlugin({
      name: "demote-h1",
      element: {
        filter: ["h1"],
        visit(node) {
          return {
            type: "element" as const,
            tagName: "h2",
            properties: { class: "demoted" },
            children: node.children,
            data: undefined,
          } as HastNode;
        },
      },
    });

    const { html } = markdownToHtml("# Title", {
      hastPlugins: [replaceH1],
    });
    expect(html).toContain("<h2");
    expect(html).toContain('class="demoted"');
    expect(html).toContain("Title");
    expect(html).not.toContain("<h1");
  });

  test("HAST plugin sets id on heading", () => {
    const addIds = defineHastPlugin({
      name: "add-ids",
      element: {
        filter: [],
        visit(node, ctx) {
          if (node.tagName === "h1") {
            ctx.setProperty(node, "id", "main-title");
          }
        },
      },
    });

    const { html } = markdownToHtml("# Hello", {
      hastPlugins: [addIds],
    });
    expect(html).toContain('id="main-title"');
  });

  test("no mutations - fast Rust path still works", () => {
    const noopPlugin = defineHastPlugin({
      name: "noop",
      element: {
        filter: [],
        visit() {
          // inspect but don't mutate
        },
      },
    });

    const { html } = markdownToHtml("# Test\n\nParagraph", {
      hastPlugins: [noopPlugin],
    });
    expect(html).toContain("<h1>");
    expect(html).toContain("Test");
    expect(html).toContain("<p>");
  });

  // with both MDAST and HAST plugins

  test("MDAST plugin removes headings, HAST plugin adds class", () => {
    const removeHeadings = defineMdastPlugin({
      name: "remove-headings",
      heading(node, ctx) {
        ctx.removeNode(node);
      },
    });

    const addClasses = defineHastPlugin({
      name: "add-classes",
      element: {
        filter: [],
        visit(node, ctx) {
          ctx.setProperty(node, "class", "styled");
        },
      },
    });

    const { html } = markdownToHtml("# Gone\n\nKeep", {
      mdastPlugins: [removeHeadings],
      hastPlugins: [addClasses],
    });
    expect(html).not.toContain("<h1>");
    expect(html).toContain('<p class="styled">');
    expect(html).toContain("Keep");
  });

  test("multiple HAST plugins compose", () => {
    const addIds = defineHastPlugin({
      name: "add-ids",
      element: {
        filter: [],
        visit(node, ctx) {
          if (node.tagName === "h1") {
            ctx.setProperty(node, "id", "title");
          }
        },
      },
    });

    const addClasses = defineHastPlugin({
      name: "add-classes",
      element: {
        filter: [],
        visit(node, ctx) {
          ctx.setProperty(node, "class", "styled");
        },
      },
    });

    const { html } = markdownToHtml("# Hello", {
      hastPlugins: [addIds, addClasses],
    });
    expect(html).toContain('id="title"');
    expect(html).toContain('class="styled"');
  });

  test("HAST plugin chain: data set on a fresh element is visible to a later plugin", () => {
    const tagFreshH2 = defineHastPlugin({
      name: "tag-fresh-h2",
      element: {
        filter: ["h1"],
        visit(node) {
          return {
            type: "element" as const,
            tagName: "h2",
            properties: {},
            children: node.children,
            data: { origin: "demoted-from-h1", depth: { from: 1, to: 2 } },
          } as HastNode;
        },
      },
    });

    const consumeData = defineHastPlugin({
      name: "consume-data",
      element: {
        filter: ["h2"],
        visit(node, ctx) {
          const origin = (node.data as { origin?: string } | undefined)?.origin;
          if (origin === "demoted-from-h1") {
            ctx.setProperty(node, "data-origin", origin);
          }
        },
      },
    });

    const { html } = markdownToHtml("# Title", {
      hastPlugins: [tagFreshH2, consumeData],
    });
    expect(html).toContain("<h2");
    expect(html).toContain('data-origin="demoted-from-h1"');
  });

  test("HAST plugin factory shape: invoked per document, closure state resets", () => {
    let factoryCalls = 0;
    const makePlugin = () => {
      factoryCalls++;
      let firstHeading = true;
      return defineHastPlugin({
        name: "tag-first-heading",
        element: {
          filter: ["h1"],
          visit(node, ctx) {
            ctx.setProperty(node, "data-first", firstHeading ? "yes" : "no");
            firstHeading = false;
          },
        },
      });
    };

    const { html: docA } = markdownToHtml("# A1\n\n# A2", { hastPlugins: [makePlugin] });
    const { html: docB } = markdownToHtml("# B1\n\n# B2", { hastPlugins: [makePlugin] });

    expect(factoryCalls).toBe(2);
    expect(docA).toContain('data-first="yes"');
    expect(docA).toContain('data-first="no"');
    expect(docB).toContain('data-first="yes"');
    expect(docB).toContain('data-first="no"');
  });

  test("HAST plugins: object and factory shapes can be mixed in one array", () => {
    const addBaseClass = defineHastPlugin({
      name: "base",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.setProperty(node, "class", "base");
        },
      },
    });

    const addOrderAttr = () => {
      let i = 0;
      return defineHastPlugin({
        name: "order",
        element: {
          filter: ["h1"],
          visit(node, ctx) {
            ctx.setProperty(node, "data-order", String(i++));
          },
        },
      });
    };

    const { html } = markdownToHtml("# One\n\n# Two", {
      hastPlugins: [addBaseClass, addOrderAttr],
    });
    expect(html).toContain('class="base"');
    expect(html).toContain('data-order="0"');
    expect(html).toContain('data-order="1"');
  });

  test("MDAST plugin factory shape: invoked per document", () => {
    let factoryCalls = 0;
    const makePlugin = () => {
      factoryCalls++;
      return defineMdastPlugin({
        name: "noop-mdast-factory",
        heading() {
          // observe only
        },
      });
    };

    markdownToHtml("# A", { mdastPlugins: [makePlugin] });
    markdownToHtml("# B", { mdastPlugins: [makePlugin] });
    expect(factoryCalls).toBe(2);
  });
});

// mdxToJs

describe("mdxToJs", () => {
  test("basic MDX compilation", () => {
    const { code: js } = mdxToJs("# Hello\n\nWorld");
    expect(js).toContain("function");
    expect(js).toContain("Hello");
  });

  test("MDX with JSX element", () => {
    const { code: js } = mdxToJs("<MyComponent />", {});
    expect(js).toContain("MyComponent");
  });

  test("MDAST plugin affects MDX output", () => {
    const removeHeadings = defineMdastPlugin({
      name: "remove-headings",
      heading(node, ctx) {
        ctx.removeNode(node);
      },
    });

    const { code: js } = mdxToJs("# Gone\n\nKept", {
      mdastPlugins: [removeHeadings],
    });
    expect(js).not.toContain("Gone");
    expect(js).toContain("Kept");
  });

  test("MDAST plugin can read JSX attributes", () => {
    const collected: unknown[] = [];
    const readAttrs = defineMdastPlugin({
      name: "read-attrs",
      mdxJsxFlowElement(node) {
        if (node.type === "mdxJsxFlowElement") {
          collected.push({
            name: node.name,
            attributes: node.attributes,
          });
        }
      },
    });

    mdxToJs('<Component foo="bar" disabled count={42} />', {
      mdastPlugins: [readAttrs],
    });

    expect(collected).toHaveLength(1);
    const el = collected[0] as { name: string; attributes: unknown[] };
    expect(el.name).toBe("Component");
    expect(el.attributes).toHaveLength(3);
    expect(el.attributes[0]).toEqual({
      type: "mdxJsxAttribute",
      name: "foo",
      value: "bar",
    });
    expect(el.attributes[1]).toEqual({
      type: "mdxJsxAttribute",
      name: "disabled",
      value: null,
    });
    expect(el.attributes[2]).toEqual({
      type: "mdxJsxAttribute",
      name: "count",
      value: { type: "mdxJsxAttributeValueExpression", value: "42" },
    });
  });

  test("MDAST plugin can replace JSX element with modified attributes", () => {
    const addAttr = defineMdastPlugin({
      name: "add-attr",
      mdxJsxFlowElement(node) {
        if (node.type === "mdxJsxFlowElement" && node.name === "Component") {
          return {
            type: "mdxJsxFlowElement",
            name: "Component",
            attributes: [{ type: "mdxJsxAttribute", name: "added", value: "yes" }],
            children: [],
          };
        }
      },
    });

    const { code: js } = mdxToJs("<Component />\n", {
      mdastPlugins: [addAttr],
    });
    // The compiled output should reference the "added" attribute
    expect(js).toContain("added");
    expect(js).toContain("yes");
  });

  test("MDAST plugin can replace JSX element removing all attributes", () => {
    const stripAttrs = defineMdastPlugin({
      name: "strip-attrs",
      mdxJsxFlowElement(node) {
        if (node.type === "mdxJsxFlowElement" && node.name === "Component") {
          return {
            type: "mdxJsxFlowElement",
            name: "Component",
            attributes: [],
            children: [],
          };
        }
      },
    });

    const { code: js } = mdxToJs('<Component foo="bar" />\n', {
      mdastPlugins: [stripAttrs],
    });
    expect(js).toContain("Component");
    expect(js).not.toContain("foo");
    expect(js).not.toContain("bar");
  });

  // Mirrors `_mdxExplicitJsx` in @mdx-js/mdx: source-parsed JSX stays literal,
  // plugin-inserted JSX routes through `_components` so users can override it.
  test("plugin-inserted mdxJsx with hyphenated name routes through _components", () => {
    const insertAstroImage = defineHastPlugin({
      name: "insert-astro-image",
      element: [
        {
          filter: ["h1"],
          visit(node, ctx) {
            ctx.insertAfter(node, {
              type: "mdxJsxFlowElement",
              name: "astro-image",
              attributes: [{ type: "mdxJsxAttribute", name: "src", value: "pic.png" }],
              children: [],
            } as unknown as HastNode);
          },
        },
      ],
    });

    const sourceWritten = mdxToJs('<astro-image src="x" />\n').code;
    expect(sourceWritten).toContain('_jsx("astro-image"');
    expect(sourceWritten).not.toContain('_components["astro-image"]');

    const pluginInserted = mdxToJs("# hi\n", { hastPlugins: [insertAstroImage] }).code;
    expect(pluginInserted).toContain('"astro-image": "astro-image"');
    expect(pluginInserted).toContain('_components["astro-image"]');
  });

  test("HAST plugin setProperty on MDX JSX element preserves existing attributes", () => {
    const injectMeta = defineHastPlugin({
      name: "inject-meta",
      mdxJsxFlowElement: {
        filter: [],
        visit(node, ctx) {
          ctx.setProperty(node, "client:component-path", "/absolute/path/B.jsx");
          ctx.setProperty(node, "client:component-export", "default");
          ctx.setProperty(node, "client:component-hydration", "");
        },
      },
    });

    const { code: js } = mdxToJs('import B from "./B.jsx"\n\n<B client:load foo="bar">hi</B>', {
      hastPlugins: [injectMeta],
    });

    // Original attributes must be preserved
    expect(js).toContain('"client:load": true');
    expect(js).toContain('foo: "bar"');
    // Injected attributes must appear
    expect(js).toContain('"client:component-path": "/absolute/path/B.jsx"');
    expect(js).toContain('"client:component-export": "default"');
    expect(js).toContain('"client:component-hydration": ""');
  });

  test("HAST plugin setProperty on MDX JSX element - no-op plugin preserves all attributes", () => {
    const noop = defineHastPlugin({
      name: "noop",
      mdxJsxFlowElement: {
        filter: [],
        visit() {
          // do nothing
        },
      },
    });

    const { code: withPlugin } = mdxToJs(
      'import B from "./B.jsx"\n\n<B client:load foo="bar">hi</B>',
      { hastPlugins: [noop] },
    );
    const { code: without } = mdxToJs('import B from "./B.jsx"\n\n<B client:load foo="bar">hi</B>');

    expect(withPlugin).toBe(without);
  });

  test("HAST plugin setProperty overwrites existing MDX JSX attribute", () => {
    const overwrite = defineHastPlugin({
      name: "overwrite-attr",
      mdxJsxFlowElement: {
        filter: [],
        visit(node, ctx) {
          ctx.setProperty(node, "foo", "replaced");
        },
      },
    });

    const { code: js } = mdxToJs('import B from "./B.jsx"\n\n<B foo="bar">hi</B>', {
      hastPlugins: [overwrite],
    });

    expect(js).toContain('foo: "replaced"');
    expect(js).not.toContain('"bar"');
  });

  // optimizeStatic

  test("optimizeStatic collapses static subtrees (Astro-style)", () => {
    const { code: js } = mdxToJs("# Hello\n\nWorld", {
      optimizeStatic: {
        component: "Fragment",
        prop: "set:html",
      },
    });
    expect(js).toContain("set:html");
    expect(js).toContain("<h1>Hello</h1>");
    expect(js).toContain("<p>World</p>");
    // Should NOT have individual element calls
    expect(js).not.toMatch(/"h1"/);
  });

  test("optimizeStatic React-style with wrapPropValue", () => {
    const { code: js } = mdxToJs("# Hello", {
      optimizeStatic: {
        component: "div",
        prop: "dangerouslySetInnerHTML",
        wrapPropValue: true,
      },
    });
    expect(js).toContain("dangerouslySetInnerHTML");
    expect(js).toContain("__html");
  });

  test("optimizeStatic preserves dynamic MDX components", () => {
    const { code: js } = mdxToJs("# Static\n\n<Dynamic />\n\nAlso static", {
      optimizeStatic: {
        component: "Fragment",
        prop: "set:html",
      },
    });
    expect(js).toContain("set:html");
    expect(js).toContain("Dynamic");
  });

  test("optimizeStatic off by default", () => {
    const { code: js } = mdxToJs("# Hello\n\nWorld");
    expect(js).not.toContain("set:html");
    expect(js).toContain('"h1"');
  });

  test("elementAttributeNameCase defaults to React casing", () => {
    const { code: js } = mdxToJs("```js\nconsole.log(1);\n```\n");
    expect(js).toContain('className: "language-js"');
    expect(js).not.toContain('class: "language-js"');
  });

  test("elementAttributeNameCase: 'html' emits HTML attribute names", () => {
    const { code: js } = mdxToJs("```js\nconsole.log(1);\n```\n\na[^1]\n\n[^1]: note\n", {
      elementAttributeNameCase: "html",
      features: { gfm: true },
    });
    expect(js).toContain('class: "language-js"');
    expect(js).not.toContain("className:");
    // GFM footnotes inject className + data-*/aria-*; the latter are already
    // kebab in both modes, but className must lowercase to class.
    expect(js).toContain('class: "footnotes"');
    expect(js).toContain('"data-footnote-ref"');
    expect(js).toContain('"aria-describedby"');
  });

  test("elementAttributeNameCase only affects HAST elements, not MDX-written JSX", () => {
    // User-written `className` on MDX-JSX is preserved verbatim regardless
    // of the casing option (mirrors @mdx-js/mdx).
    const { code: js } = mdxToJs('<div className="x">hi</div>\n', {
      elementAttributeNameCase: "html",
    });
    expect(js).toContain('className: "x"');
  });

  test("style attribute parses into an object by default (DOM casing)", () => {
    const { code: js } = mdxToJs("| a | b |\n|:--|--:|\n| c | d |\n", {
      features: { gfm: true },
    });
    expect(js).toContain('style: { textAlign: "right" }');
    expect(js).toContain('style: { textAlign: "left" }');
    expect(js).not.toContain('style: "text-align');
  });

  test("stylePropertyNameCase: 'css' keeps kebab-case keys", () => {
    const { code: js } = mdxToJs("| a | b |\n|:--|--:|\n| c | d |\n", {
      features: { gfm: true },
      stylePropertyNameCase: "css",
    });
    // `text-align` is not a valid JS identifier so it serializes as a string.
    expect(js).toContain('"text-align": "right"');
    expect(js).toContain('"text-align": "left"');
    expect(js).not.toContain("textAlign:");
  });

  test("stylePropertyNameCase via hast plugin: vendor prefixes and custom properties", () => {
    // Attach a complex style string via a plugin so we exercise the parsing
    // on something other than table-align.
    const setStyle = defineHastPlugin({
      name: "set-style",
      element: {
        filter: ["p"],
        visit(node, ctx) {
          ctx.setProperty(
            node,
            "style",
            "background-color: red; -webkit-line-clamp: 2; --tmLabel: blue; --x: 1",
          );
        },
      },
    });

    const dom = mdxToJs("hi\n", { hastPlugins: [setStyle] }).code;
    expect(dom).toContain('backgroundColor: "red"');
    expect(dom).toContain('WebkitLineClamp: "2"');
    // Custom properties are kept verbatim under both casings — including their
    // case, which is significant (`--tmLabel` ≠ `--tmlabel`). Regression test
    // for https://github.com/withastro/astro/issues/16940.
    expect(dom).toContain('"--tmLabel": "blue"');
    expect(dom).toContain('"--x": "1"');

    const css = mdxToJs("hi\n", {
      hastPlugins: [setStyle],
      stylePropertyNameCase: "css",
    }).code;
    expect(css).toContain('"background-color": "red"');
    expect(css).toContain('"-webkit-line-clamp": "2"');
    expect(css).toContain('"--tmLabel": "blue"');
    expect(css).toContain('"--x": "1"');
  });

  test("case-insensitive standard property names are lowercased", () => {
    // CSS standard property names are case-insensitive, so satteri normalizes
    // `COLOR` to `color`. Custom properties (`--*`) are case-sensitive and
    // exempt (covered above).
    const setStyle = defineHastPlugin({
      name: "set-style",
      element: {
        filter: ["p"],
        visit(node, ctx) {
          ctx.setProperty(node, "style", "COLOR: red");
        },
      },
    });

    const { code } = mdxToJs("hi\n", { hastPlugins: [setStyle] });
    expect(code).toContain('color: "red"');
    expect(code).not.toContain("COLOR");
  });

  test("style on MDX-written JSX is preserved as a string", () => {
    // Matches @mdx-js/mdx: only HAST elements get string-to-object conversion.
    const { code: js } = mdxToJs('<div style="color: red">hi</div>\n');
    expect(js).toContain('style: "color: red"');
    expect(js).not.toContain("style: {");
  });

  test("rawHtml preserves curly braces as literal text", () => {
    const plugin = defineMdastPlugin({
      name: "raw-html-braces",
      code() {
        return {
          rawHtml: '<pre class="shiki"><code><span style="color:red">{foo: 1}</span></code></pre>',
        };
      },
    });

    const { code: js } = mdxToJs("```js\nconst x = {foo: 1}\n```", {
      mdastPlugins: [plugin],
    });

    // Curly braces should appear as string content, not parsed as MDX expressions.
    // The escaping splits them into separate children: "{", "foo: 1", "}"
    expect(js).toContain('"{"');
    expect(js).toContain('"}"');
    expect(js).toContain("foo: 1");
    expect(js).not.toContain("Could not parse");
    expect(js).toContain("shiki");
  });

  test("rawHtml with multiline shiki output preserves all content", () => {
    const shikiHtml = `<pre class="shiki github-dark" style="background-color:#24292e"><code><span class="line"><span style="color:#F97583">const</span><span style="color:#E1E4E8"> x = </span><span style="color:#B392F0">{</span></span>\n<span class="line"><span style="color:#E1E4E8">  foo: </span><span style="color:#79B8FF">1</span></span>\n<span class="line"><span style="color:#B392F0">}</span></span></code></pre>`;

    const plugin = defineMdastPlugin({
      name: "raw-html-shiki",
      code() {
        return { rawHtml: shikiHtml };
      },
    });

    const { code: js } = mdxToJs("```js\nconst x = {\n  foo: 1\n}\n```", {
      mdastPlugins: [plugin],
    });

    expect(js).toContain("const");
    expect(js).toContain("foo");
    expect(js).toContain("shiki");
    expect(js).not.toContain("Could not parse");
  });

  test("MDX expression in heading is preserved", () => {
    const { code: js } = mdxToJs("# {title}");
    expect(js).toContain("children: title");
  });

  test("MDX expression mixed with text in heading", () => {
    const { code: js } = mdxToJs("## Hello {name}");
    expect(js).toContain('"Hello "');
    expect(js).toContain("name");
  });

  test("MDX frontmatter expression in heading", () => {
    const { code: js } = mdxToJs("# {frontmatter.title}");
    expect(js).toContain("frontmatter.title");
  });

  test("sync HAST plugin works", () => {
    const plugin = defineHastPlugin({
      name: "class-adder",
      element: {
        filter: [],
        visit(node, ctx) {
          ctx.setProperty(node, "class", "added");
        },
      },
    });

    const { html } = markdownToHtml("# Hello", {
      hastPlugins: [plugin],
    });
    expect(html).toContain('class="added"');
  });

  // Filtered (selective) HAST visitors

  test("filtered element visitor - single tag", () => {
    const plugin = defineHastPlugin({
      name: "link-class",
      element: {
        filter: ["a"],
        visit(node, ctx) {
          ctx.setProperty(node, "class", "link");
        },
      },
    });

    const { html } = markdownToHtml("# Hello\n\n[click](https://example.com)", {
      hastPlugins: [plugin],
    });
    expect(html).toContain('class="link"');
    expect(html).toContain("click");
    // Heading should NOT have the class
    expect(html).toMatch(/<h1>Hello<\/h1>/);
  });

  test("filtered element visitor - multiple tags", () => {
    const plugin = defineHastPlugin({
      name: "heading-class",
      element: {
        filter: ["h1", "h2"],
        visit(node, ctx) {
          ctx.setProperty(node, "class", "heading");
        },
      },
    });

    const { html } = markdownToHtml("# One\n\n## Two\n\nParagraph", {
      hastPlugins: [plugin],
    });
    expect(html).toContain('<h1 class="heading">');
    expect(html).toContain('<h2 class="heading">');
    expect(html).not.toContain('<p class="heading">');
  });

  test("filtered element visitor - array of filter groups", () => {
    const plugin = defineHastPlugin({
      name: "multi-filter",
      element: [
        {
          filter: ["h1"],
          visit(node, ctx) {
            ctx.setProperty(node, "id", "title");
          },
        },
        {
          filter: ["a"],
          visit(node, ctx) {
            ctx.setProperty(node, "target", "_blank");
          },
        },
      ],
    });

    const { html } = markdownToHtml("# Title\n\n[link](https://example.com)", {
      hastPlugins: [plugin],
    });
    expect(html).toContain('id="title"');
    expect(html).toContain('target="_blank"');
  });

  test("filtered visitor mixed with unfiltered falls back to JS walk", () => {
    // This plugin has a bare `text` function (unfiltered), so it should
    // fall back to the JS walk path, but still produce correct results.
    const plugin = defineHastPlugin({
      name: "mixed",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.setProperty(node, "class", "heading");
        },
      },
      text(node, _ctx) {
        // noop, but being a bare function forces JS-walk fallback
      },
    });

    const { html } = markdownToHtml("# Hello", {
      hastPlugins: [plugin],
    });
    // The filter still works via fallback JS walk
    expect(html).toContain("Hello");
  });

  // Async visitors

  test("async MDAST visitor - replaces code block after await", async () => {
    const plugin = defineMdastPlugin({
      name: "async-code",
      async code(node) {
        await new Promise((r) => setTimeout(r, 1));
        return { rawHtml: "<pre>async-highlighted</pre>" };
      },
    });

    const result = mdxToJs("```js\ncode\n```", { mdastPlugins: [plugin] });
    expect(result).toBeInstanceOf(Promise);
    const { code: js } = await result;
    expect(js).toContain("async-highlighted");
  });

  test("sync MDAST plugins return synchronously", () => {
    const plugin = defineMdastPlugin({
      name: "sync-mdast",
      heading(node, ctx) {
        ctx.setProperty(node, "depth", 2);
      },
    });

    const result = mdxToJs("# Title", { mdastPlugins: [plugin] });
    expect(result).not.toBeInstanceOf(Promise);
  });

  test("async HAST visitor - replaces element after await", async () => {
    const plugin = defineHastPlugin({
      name: "async-replace",
      element: {
        filter: ["pre"],
        async visit(node, ctx) {
          // Simulate async work (e.g. shiki language loading)
          await new Promise((r) => setTimeout(r, 1));
          ctx.replaceNode(node, { type: "raw", value: "<pre>highlighted</pre>" } as HastNode);
        },
      },
    });

    const result = markdownToHtml("```js\ncode\n```", { hastPlugins: [plugin] });
    expect(result).toBeInstanceOf(Promise);
    const { html } = await result;
    expect(html).toContain("highlighted");
  });

  test("async HAST visitor - multiple async visitors run in parallel", async () => {
    const order: string[] = [];
    const plugin = defineHastPlugin({
      name: "async-parallel",
      element: {
        filter: ["h1", "h2"],
        async visit(node, ctx) {
          const tag = node.tagName;
          order.push(`start:${tag}`);
          await new Promise((r) => setTimeout(r, tag === "h1" ? 10 : 1));
          order.push(`end:${tag}`);
          ctx.setProperty(node, "class", "processed");
        },
      },
    });

    const { html } = await markdownToHtml("# One\n\n## Two", { hastPlugins: [plugin] });
    expect(html).toContain('class="processed"');
    // Both should start before either ends (parallel execution)
    expect(order[0]).toBe("start:h1");
    expect(order[1]).toBe("start:h2");
  });

  test("mixed sync and async plugins - sync has zero overhead", async () => {
    const syncPlugin = defineHastPlugin({
      name: "sync-class",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.setProperty(node, "id", "sync");
        },
      },
    });

    const asyncPlugin = defineHastPlugin({
      name: "async-class",
      element: {
        filter: ["p"],
        async visit(node, ctx) {
          await new Promise((r) => setTimeout(r, 1));
          ctx.setProperty(node, "class", "async");
        },
      },
    });

    const result = markdownToHtml("# Title\n\nParagraph", {
      hastPlugins: [syncPlugin, asyncPlugin],
    });
    const { html } = await result;
    expect(html).toContain('id="sync"');
    expect(html).toContain('class="async"');
  });

  test("sync-only plugins return synchronously", () => {
    const plugin = defineHastPlugin({
      name: "sync-only",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.setProperty(node, "id", "test");
        },
      },
    });

    const result = markdownToHtml("# Hello", { hastPlugins: [plugin] });
    expect(result).not.toBeInstanceOf(Promise);
    if (result instanceof Promise) return;
    expect(result.html).toContain('id="test"');
  });

  test("mdast setProperty + returning same node preserves mutation", () => {
    const plugin = defineMdastPlugin({
      name: "bump-heading",
      heading(node, ctx) {
        if (node.depth < 6) {
          ctx.setProperty(node, "depth", (node.depth + 1) as 1 | 2 | 3 | 4 | 5 | 6);
        }
        return node; // returning same node should NOT clobber setProperty
      },
    });

    const { html } = markdownToHtml("# Hello", { mdastPlugins: [plugin] });
    expect(html).toContain("<h2>");
    expect(html).not.toContain("<h1>");
  });

  test("hast setProperty on text node updates value", () => {
    const plugin = defineHastPlugin({
      name: "uppercase-text",
      text(node, ctx) {
        ctx.setProperty(node, "value", node.value.toUpperCase());
      },
    });

    const { html } = markdownToHtml("hello world", { hastPlugins: [plugin] });
    expect(html).toContain("HELLO WORLD");
  });

  test("reading time plugin counts words across text nodes", () => {
    function createReadingTimePlugin() {
      let wordCount = 0;
      const plugin = defineMdastPlugin({
        name: "reading-time",
        text(node) {
          if (node.type === "text") {
            wordCount += node.value.split(/\s+/).length;
          }
        },
      });
      return {
        plugin,
        get minutes() {
          return Math.ceil(wordCount / 200);
        },
        get wordCount() {
          return wordCount;
        },
      };
    }

    const readingTime = createReadingTimePlugin();
    const { html } = markdownToHtml(
      "# Hello\n\nThis is a paragraph with some words in it.\n\nAnother paragraph here.",
      { mdastPlugins: [readingTime.plugin] },
    );
    expect(html).toContain("<h1>");
    expect(readingTime.wordCount).toBeGreaterThan(0);
    expect(readingTime.minutes).toBe(1);
  });

  test("spread node with override replaces correctly", () => {
    const plugin = defineMdastPlugin({
      name: "spread-replace",
      heading(node) {
        return { ...node, depth: 2 } as typeof node;
      },
    });

    const { html } = markdownToHtml("# Hello", { mdastPlugins: [plugin] });
    expect(html).toContain("<h2>");
    expect(html).toContain("Hello");
    expect(html).not.toContain("<h1>");
  });

  test("emoji shortcode replacement", () => {
    const emojis = defineMdastPlugin({
      name: "emojis",
      text(node, ctx) {
        if (node.type === "text" && node.value.includes(":wave:")) {
          ctx.setProperty(node, "value", node.value.replaceAll(":wave:", "\u{1F44B}"));
        }
      },
    });

    const { html } = markdownToHtml("Hello :wave: world :wave:", {
      mdastPlugins: [emojis],
    });
    expect(html).toContain("\u{1F44B}");
    expect(html).not.toContain(":wave:");
  });

  test("async shiki highlighting via mdast plugin", async () => {
    const { createHighlighter } = await import("shiki");
    const highlighter = await createHighlighter({ themes: ["github-dark"], langs: ["js"] });

    const asyncHighlight = defineMdastPlugin({
      name: "async-highlight",
      async code(node) {
        if (node.type === "code") {
          const html = await highlighter.codeToHtml(node.value, {
            lang: node.lang ?? "text",
            theme: "github-dark",
          });
          return { rawHtml: html };
        }
      },
    });

    const result = markdownToHtml("```js\nconsole.log(1)\n```", {
      mdastPlugins: [asyncHighlight],
    });
    expect(result).toBeInstanceOf(Promise);
    const { html } = await result;
    expect(html).toContain("shiki");
    expect(html).toContain("console");
    expect(html).not.toContain('<code class="language-js">');
  });

  test("unwrap images from paragraphs", () => {
    const unwrapImages = defineMdastPlugin({
      name: "unwrap-images",
      paragraph(node) {
        if (node.type === "paragraph") {
          const child = node.children[0];
          if (node.children.length === 1 && child?.type === "image") {
            return child;
          }
        }
      },
    });

    const { html } = markdownToHtml("![alt text](https://example.com/img.png)", {
      mdastPlugins: [unwrapImages],
    });
    expect(html).toContain('<img src="https://example.com/img.png" alt="alt text">');
    expect(html).not.toContain("<p>");
  });
});

// Step-by-step API

describe("markdownToMdast", () => {
  test("returns an mdast root", () => {
    const tree = markdownToMdast("# Hello\n\nWorld");
    expect(tree.type).toBe("root");
    if (tree.type !== "root") return;
    expect(tree.children).toHaveLength(2);
    const heading = tree.children[0]!;
    expect(heading.type).toBe("heading");
    if (heading.type !== "heading") return;
    expect(heading.depth).toBe(1);
    expect(tree.children[1]!.type).toBe("paragraph");
  });

  test("code block has lang and value", () => {
    const tree = markdownToMdast("```js\nconsole.log(1)\n```");
    if (tree.type !== "root") return;
    const code = tree.children[0]!;
    expect(code.type).toBe("code");
    if (code.type !== "code") return;
    expect(code.lang).toBe("js");
    expect(code.value).toContain("console.log(1)");
  });
});

describe("mdxToMdast", () => {
  test("parses JSX elements", () => {
    const tree = mdxToMdast('<MyComponent foo="bar" />');
    expect(tree.type).toBe("root");
    if (tree.type !== "root") return;
    const jsx = tree.children[0]!;
    expect(jsx.type).toBe("mdxJsxFlowElement");
    if (jsx.type !== "mdxJsxFlowElement") return;
    expect(jsx.name).toBe("MyComponent");
  });
});

describe("markdownToHast", () => {
  test("returns a hast root", () => {
    const tree = markdownToHast("# Hello\n\nWorld");
    expect(tree.type).toBe("root");
    if (tree.type !== "root") return;
    expect(tree.children.length).toBeGreaterThan(0);
    const h1 = tree.children[0]!;
    expect(h1.type).toBe("element");
    if (h1.type !== "element") return;
    expect(h1.tagName).toBe("h1");
  });
});

describe("mdxToHast", () => {
  test("returns a hast root with MDX elements", () => {
    const tree = mdxToHast("<MyComponent />");
    expect(tree.type).toBe("root");
    if (tree.type !== "root") return;
    const jsx = tree.children[0]!;
    expect(jsx.type).toBe("mdxJsxFlowElement");
    if (jsx.type !== "mdxJsxFlowElement") return;
    expect(jsx.name).toBe("MyComponent");
  });
});

describe("smartPunctuation options", () => {
  const input = `"Hello," she said -- it was... unexpected.`;

  test("boolean true enables all smart punctuation", () => {
    const { html } = markdownToHtml(input, {
      features: { smartPunctuation: true },
    });
    expect(html).toContain("\u201c");
    expect(html).toContain("\u201d");
    expect(html).toContain("\u2013");
    expect(html).toContain("\u2026");
    expect(html).not.toContain("--");
    expect(html).not.toContain("...");
  });

  test("boolean false disables all smart punctuation", () => {
    const { html } = markdownToHtml(input, {
      features: { smartPunctuation: false },
    });
    expect(html).toContain('"');
    expect(html).toContain("--");
    expect(html).toContain("...");
  });

  test("quotes only", () => {
    const { html } = markdownToHtml(input, {
      features: { smartPunctuation: { quotes: true, dashes: false, ellipses: false } },
    });
    expect(html).toContain("\u201c");
    expect(html).toContain("--");
    expect(html).toContain("...");
  });

  test("dashes only", () => {
    const { html } = markdownToHtml(input, {
      features: { smartPunctuation: { quotes: false, dashes: true, ellipses: false } },
    });
    expect(html).toContain('"');
    expect(html).toContain("\u2013");
    expect(html).toContain("...");
  });

  test("ellipses only", () => {
    const { html } = markdownToHtml(input, {
      features: { smartPunctuation: { quotes: false, dashes: false, ellipses: true } },
    });
    expect(html).toContain('"');
    expect(html).toContain("--");
    expect(html).toContain("\u2026");
  });

  test("omitted fields default to true", () => {
    const { html } = markdownToHtml(input, {
      features: { smartPunctuation: { dashes: false } },
    });
    expect(html).toContain("\u201c");
    expect(html).toContain("--");
    expect(html).toContain("\u2026");
  });

  test("empty object enables all", () => {
    const { html: all } = markdownToHtml(input, {
      features: { smartPunctuation: true },
    });
    const { html: empty } = markdownToHtml(input, {
      features: { smartPunctuation: {} },
    });
    expect(empty).toBe(all);
  });

  test("granular options work with mdxToJs", () => {
    const { code: js } = mdxToJs(input, {
      features: { smartPunctuation: { dashes: false } },
    });
    expect(js).toContain("\u201c");
    expect(js).toContain("--");
    expect(js).toContain("\u2026");
  });

  test("curls quotes that surround an MDX expression", () => {
    // Documented divergence: remark-smartypants curls each text node in
    // isolation, so an expression between the quotes leaves them straight.
    // satteri pairs across the expression.
    const { code: js } = mdxToJs('exports: "{value}"\n', {
      features: { smartPunctuation: true },
    });
    expect(js).toContain("\u201c");
    expect(js).toContain("\u201d");
  });
});
