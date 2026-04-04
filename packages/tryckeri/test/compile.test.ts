import { describe, test, expect } from "vitest";
import {
  compileMarkdownToHtml,
  compileMdxToJs,
  defineMdastPlugin,
  defineHastPlugin,
} from "../src/index.js";
import type { HastNode } from "../src/hast/hast-materializer.js";
import type { HastVisitorContext } from "../src/hast/hast-visitor.js";
import type { MdastNode } from "../src/types.js";
import type { Element } from "hast";
import type { MdxJsxTextElementHast } from "mdast-util-mdx-jsx";

// ---------------------------------------------------------------------------
// compileMarkdownToHtml — no plugins
// ---------------------------------------------------------------------------

describe("compileMarkdownToHtml", () => {
  test("basic markdown to HTML", () => {
    const html = compileMarkdownToHtml("# Hello\n\nWorld");
    expect(html).toContain("<h1>");
    expect(html).toContain("Hello");
    expect(html).toContain("<p>");
    expect(html).toContain("World");
  });

  test("empty string produces empty output", () => {
    const html = compileMarkdownToHtml("");
    expect(html).toBe("");
  });

  test("inline formatting", () => {
    const html = compileMarkdownToHtml("**bold** and *italic*");
    expect(html).toContain("<strong>bold</strong>");
    expect(html).toContain("<em>italic</em>");
  });

  test("link renders as anchor", () => {
    const html = compileMarkdownToHtml("[click](https://example.com)");
    expect(html).toContain('<a href="https://example.com">click</a>');
  });

  test("code block with language", () => {
    const html = compileMarkdownToHtml("```js\nconsole.log(1)\n```");
    expect(html).toContain('<code class="language-js">');
    expect(html).toContain("console.log(1)");
  });

  // ---------------------------------------------------------------------------
  // with MDAST plugins only
  // ---------------------------------------------------------------------------

  test("MDAST plugin removes headings", () => {
    const removeHeadings = defineMdastPlugin({
      name: "remove-headings",
      createOnce: () => ({
        heading(node: MdastNode, ctx: { removeNode(n: MdastNode): void }) {
          ctx.removeNode(node);
        },
      }),
    });

    const html = compileMarkdownToHtml("# Title\n\nKeep this", {
      mdastPlugins: [removeHeadings],
    });
    expect(html).not.toContain("<h1>");
    expect(html).not.toContain("Title");
    expect(html).toContain("Keep this");
  });

  test("MDAST plugin replaces text with raw markdown", () => {
    const uppercaseHeadings = defineMdastPlugin({
      name: "uppercase-headings",
      createOnce: () => ({
        heading(_node: MdastNode) {
          return { raw: "# REPLACED" };
        },
      }),
    });

    const html = compileMarkdownToHtml("# Original\n\npara", {
      mdastPlugins: [uppercaseHeadings],
    });
    expect(html).toContain("REPLACED");
    expect(html).not.toContain("Original");
  });

  // ---------------------------------------------------------------------------
  // with HAST plugins only
  // ---------------------------------------------------------------------------

  test("HAST plugin adds class to all elements", () => {
    const addClasses = defineHastPlugin({
      name: "add-classes",
      createOnce: () => ({
        element: {
          filter: [],
          visit(node, ctx) {
            ctx.setProperty(node, "class", "styled");
          },
        },
      }),
    });

    const html = compileMarkdownToHtml("# Hello\n\nWorld", {
      hastPlugins: [addClasses],
    });
    expect(html).toContain('<h1 class="styled">');
    expect(html).toContain('<p class="styled">');
  });

  test("HAST plugin removes elements", () => {
    const removeHeadings = defineHastPlugin({
      name: "remove-h1",
      createOnce: () => ({
        element: {
          filter: [],
          visit(node, ctx) {
            if (node.tagName === "h1") {
              ctx.removeNode(node);
            }
          },
        },
      }),
    });

    const html = compileMarkdownToHtml("# Gone\n\nStays", {
      hastPlugins: [removeHeadings],
    });
    expect(html).not.toContain("<h1>");
    expect(html).not.toContain("Gone");
    expect(html).toContain("Stays");
  });

  test("HAST plugin replaces element via return value", () => {
    const replaceH1 = defineHastPlugin({
      name: "demote-h1",
      createOnce: () => ({
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
      }),
    });

    const html = compileMarkdownToHtml("# Title", {
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
      createOnce: () => ({
        element: {
          filter: [],
          visit(node, ctx) {
            if (node.tagName === "h1") {
              ctx.setProperty(node, "id", "main-title");
            }
          },
        },
      }),
    });

    const html = compileMarkdownToHtml("# Hello", {
      hastPlugins: [addIds],
    });
    expect(html).toContain('id="main-title"');
  });


  test("no mutations — fast Rust path still works", () => {
    const noopPlugin = defineHastPlugin({
      name: "noop",
      createOnce: () => ({
        element: {
          filter: [],
          visit() {
            // inspect but don't mutate
          },
        },
      }),
    });

    const html = compileMarkdownToHtml("# Test\n\nParagraph", {
      hastPlugins: [noopPlugin],
    });
    expect(html).toContain("<h1>");
    expect(html).toContain("Test");
    expect(html).toContain("<p>");
  });

  // ---------------------------------------------------------------------------
  // with both MDAST and HAST plugins
  // ---------------------------------------------------------------------------

  test("MDAST plugin removes headings, HAST plugin adds class", () => {
    const removeHeadings = defineMdastPlugin({
      name: "remove-headings",
      createOnce: () => ({
        heading(node: MdastNode, ctx: { removeNode(n: MdastNode): void }) {
          ctx.removeNode(node);
        },
      }),
    });

    const addClasses = defineHastPlugin({
      name: "add-classes",
      createOnce: () => ({
        element: {
          filter: [],
          visit(node, ctx) {
            ctx.setProperty(node, "class", "styled");
          },
        },
      }),
    });

    const html = compileMarkdownToHtml("# Gone\n\nKeep", {
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
      createOnce: () => ({
        element: {
          filter: [],
          visit(node, ctx) {
            if (node.tagName === "h1") {
              ctx.setProperty(node, "id", "title");
            }
          },
        },
      }),
    });

    const addClasses = defineHastPlugin({
      name: "add-classes",
      createOnce: () => ({
        element: {
          filter: [],
          visit(node, ctx) {
            ctx.setProperty(node, "class", "styled");
          },
        },
      }),
    });

    const html = compileMarkdownToHtml("# Hello", {
      hastPlugins: [addIds, addClasses],
    });
    expect(html).toContain('id="title"');
    expect(html).toContain('class="styled"');
  });
});

// ---------------------------------------------------------------------------
// compileMdxToJs
// ---------------------------------------------------------------------------

describe("compileMdxToJs", () => {
  test("basic MDX compilation", () => {
    const js = compileMdxToJs("# Hello\n\nWorld");
    expect(js).toContain("function");
    expect(js).toContain("Hello");
  });

  test("MDX with JSX element", () => {
    const js = compileMdxToJs("<MyComponent />", {});
    expect(js).toContain("MyComponent");
  });

  test("MDAST plugin affects MDX output", () => {
    const removeHeadings = defineMdastPlugin({
      name: "remove-headings",
      createOnce: () => ({
        heading(node: MdastNode, ctx: { removeNode(n: MdastNode): void }) {
          ctx.removeNode(node);
        },
      }),
    });

    const js = compileMdxToJs("# Gone\n\nKept", {
      mdastPlugins: [removeHeadings],
    });
    expect(js).not.toContain("Gone");
    expect(js).toContain("Kept");
  });

  test("MDAST plugin can read JSX attributes", () => {
    const collected: unknown[] = [];
    const readAttrs = defineMdastPlugin({
      name: "read-attrs",
      createOnce: () => ({
        mdxJsxFlowElement(node: MdastNode) {
          if (node.type === "mdxJsxFlowElement") {
            collected.push({
              name: node.name,
              attributes: node.attributes,
            });
          }
        },
      }),
    });

    compileMdxToJs('<Component foo="bar" disabled count={42} />', {
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
      createOnce: () => ({
        mdxJsxFlowElement(node: MdastNode) {
          if (node.type === "mdxJsxFlowElement" && node.name === "Component") {
            return {
              type: "mdxJsxFlowElement",
              name: "Component",
              attributes: [{ type: "mdxJsxAttribute", name: "added", value: "yes" }],
              children: [],
            };
          }
        },
      }),
    });

    const js = compileMdxToJs("<Component />\n", {
      mdastPlugins: [addAttr],
    });
    // The compiled output should reference the "added" attribute
    expect(js).toContain("added");
    expect(js).toContain("yes");
  });

  test("MDAST plugin can replace JSX element removing all attributes", () => {
    const stripAttrs = defineMdastPlugin({
      name: "strip-attrs",
      createOnce: () => ({
        mdxJsxFlowElement(node: MdastNode) {
          if (node.type === "mdxJsxFlowElement" && node.name === "Component") {
            return {
              type: "mdxJsxFlowElement",
              name: "Component",
              attributes: [],
              children: [],
            };
          }
        },
      }),
    });

    const js = compileMdxToJs('<Component foo="bar" />\n', {
      mdastPlugins: [stripAttrs],
    });
    expect(js).toContain("Component");
    expect(js).not.toContain("foo");
    expect(js).not.toContain("bar");
  });

  test("HAST plugin setProperty on MDX JSX element preserves existing attributes", () => {
    const injectMeta = defineHastPlugin({
      name: "inject-meta",
      createOnce: () => ({
        mdxJsxTextElement: {
          filter: [],
          visit(node, ctx) {
            ctx.setProperty(
              node as unknown as HastNode,
              "client:component-path",
              "/absolute/path/B.jsx",
            );
            ctx.setProperty(node as unknown as HastNode, "client:component-export", "default");
            ctx.setProperty(node as unknown as HastNode, "client:component-hydration", "");
          },
        },
      }),
    });

    const js = compileMdxToJs('import B from "./B.jsx"\n\n<B client:load foo="bar">hi</B>', {
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

  test("HAST plugin setProperty on MDX JSX element — no-op plugin preserves all attributes", () => {
    const noop = defineHastPlugin({
      name: "noop",
      createOnce: () => ({
        mdxJsxTextElement: {
          filter: [],
          visit() {
            // do nothing
          },
        },
      }),
    });

    const withPlugin = compileMdxToJs(
      'import B from "./B.jsx"\n\n<B client:load foo="bar">hi</B>',
      { hastPlugins: [noop] },
    );
    const without = compileMdxToJs('import B from "./B.jsx"\n\n<B client:load foo="bar">hi</B>');

    expect(withPlugin).toBe(without);
  });

  test("HAST plugin setProperty overwrites existing MDX JSX attribute", () => {
    const overwrite = defineHastPlugin({
      name: "overwrite-attr",
      createOnce: () => ({
        mdxJsxTextElement: {
          filter: [],
          visit(node: HastNode, ctx: HastVisitorContext) {
            ctx.setProperty(node, "foo", "replaced");
          },
        },
      }),
    });

    const js = compileMdxToJs('import B from "./B.jsx"\n\n<B foo="bar">hi</B>', {
      hastPlugins: [overwrite],
    });

    expect(js).toContain('foo: "replaced"');
    expect(js).not.toContain('"bar"');
  });

  // ---------------------------------------------------------------------------
  // optimizeStatic
  // ---------------------------------------------------------------------------

  test("optimizeStatic collapses static subtrees (Astro-style)", () => {
    const js = compileMdxToJs("# Hello\n\nWorld", {
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
    const js = compileMdxToJs("# Hello", {
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
    const js = compileMdxToJs("# Static\n\n<Dynamic />\n\nAlso static", {
      optimizeStatic: {
        component: "Fragment",
        prop: "set:html",
      },
    });
    expect(js).toContain("set:html");
    expect(js).toContain("Dynamic");
  });

  test("optimizeStatic off by default", () => {
    const js = compileMdxToJs("# Hello\n\nWorld");
    expect(js).not.toContain("set:html");
    expect(js).toContain('"h1"');
  });

  test("rawHtml preserves curly braces as literal text", () => {
    const plugin = defineMdastPlugin({
      name: "raw-html-braces",
      createOnce: () => ({
        code() {
          return {
            rawHtml:
              '<pre class="shiki"><code><span style="color:red">{foo: 1}</span></code></pre>',
          };
        },
      }),
    });

    const js = compileMdxToJs("```js\nconst x = {foo: 1}\n```", {
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
      createOnce: () => ({
        code() {
          return { rawHtml: shikiHtml };
        },
      }),
    });

    const js = compileMdxToJs("```js\nconst x = {\n  foo: 1\n}\n```", {
      mdastPlugins: [plugin],
    });

    expect(js).toContain("const");
    expect(js).toContain("foo");
    expect(js).toContain("shiki");
    expect(js).not.toContain("Could not parse");
  });

  test("MDX expression in heading is preserved", () => {
    const js = compileMdxToJs("# {title}");
    expect(js).toContain("children: title");
  });

  test("MDX expression mixed with text in heading", () => {
    const js = compileMdxToJs("## Hello {name}");
    expect(js).toContain('"Hello "');
    expect(js).toContain("name");
  });

  test("MDX frontmatter expression in heading", () => {
    const js = compileMdxToJs("# {frontmatter.title}");
    expect(js).toContain("frontmatter.title");
  });

  test("sync HAST plugin works", () => {
    const plugin = defineHastPlugin({
      name: "class-adder",
      createOnce: () => ({
        element: {
          filter: [],
          visit(node: HastNode, ctx: HastVisitorContext) {
            ctx.setProperty(node, "class", "added");
          },
        },
      }),
    });

    const html = compileMarkdownToHtml("# Hello", {
      hastPlugins: [plugin],
    });
    expect(html).toContain('class="added"');
  });

  // -------------------------------------------------------------------------
  // Filtered (selective) HAST visitors
  // -------------------------------------------------------------------------

  test("filtered element visitor — single tag", () => {
    const plugin = defineHastPlugin({
      name: "link-class",
      createOnce: () => ({
        element: {
          filter: ["a"],
          visit(node: HastNode, ctx: HastVisitorContext) {
            ctx.setProperty(node, "class", "link");
          },
        },
      }),
    });

    const html = compileMarkdownToHtml("# Hello\n\n[click](https://example.com)", {
      hastPlugins: [plugin],
    });
    expect(html).toContain('class="link"');
    expect(html).toContain("click");
    // Heading should NOT have the class
    expect(html).toMatch(/<h1>Hello<\/h1>/);
  });

  test("filtered element visitor — multiple tags", () => {
    const plugin = defineHastPlugin({
      name: "heading-class",
      createOnce: () => ({
        element: {
          filter: ["h1", "h2"],
          visit(node: HastNode, ctx: HastVisitorContext) {
            ctx.setProperty(node, "class", "heading");
          },
        },
      }),
    });

    const html = compileMarkdownToHtml("# One\n\n## Two\n\nParagraph", {
      hastPlugins: [plugin],
    });
    expect(html).toContain('<h1 class="heading">');
    expect(html).toContain('<h2 class="heading">');
    expect(html).not.toContain('<p class="heading">');
  });

  test("filtered element visitor — array of filter groups", () => {
    const plugin = defineHastPlugin({
      name: "multi-filter",
      createOnce: () => ({
        element: [
          {
            filter: ["h1"],
            visit(node: HastNode, ctx: HastVisitorContext) {
              ctx.setProperty(node, "id", "title");
            },
          },
          {
            filter: ["a"],
            visit(node: HastNode, ctx: HastVisitorContext) {
              ctx.setProperty(node, "target", "_blank");
            },
          },
        ],
      }),
    });

    const html = compileMarkdownToHtml("# Title\n\n[link](https://example.com)", {
      hastPlugins: [plugin],
    });
    expect(html).toContain('id="title"');
    expect(html).toContain('target="_blank"');
  });

  test("filtered visitor mixed with unfiltered falls back to JS walk", () => {
    // This plugin has a bare `text` function (unfiltered), so it should
    // fall back to the JS walk path — but still produce correct results.
    const plugin = defineHastPlugin({
      name: "mixed",
      createOnce: () => ({
        element: {
          filter: ["h1"],
          visit(node: HastNode, ctx: HastVisitorContext) {
            ctx.setProperty(node, "class", "heading");
          },
        },
        text(node: HastNode, _ctx: HastVisitorContext) {
          // noop — but being a bare function forces JS-walk fallback
        },
      }),
    });

    const html = compileMarkdownToHtml("# Hello", {
      hastPlugins: [plugin],
    });
    // The filter still works via fallback JS walk
    expect(html).toContain("Hello");
  });

  // -----------------------------------------------------------------------
  // Async visitors
  // -----------------------------------------------------------------------

  test("async MDAST visitor — replaces code block after await", async () => {
    const plugin = defineMdastPlugin({
      name: "async-code",
      createOnce: () => ({
        async code(node: MdastNode) {
          await new Promise((r) => setTimeout(r, 1));
          return { rawHtml: "<pre>async-highlighted</pre>" };
        },
      }),
    });

    const result = compileMdxToJs("```js\ncode\n```", { mdastPlugins: [plugin] });
    expect(result).toBeInstanceOf(Promise);
    const js = await result;
    expect(js).toContain("async-highlighted");
  });


  test("sync MDAST plugins return string not Promise", () => {
    const plugin = defineMdastPlugin({
      name: "sync-mdast",
      createOnce: () => ({
        heading(node: MdastNode, ctx: any) {
          ctx.setProperty(node, "depth", 2);
        },
      }),
    });

    const result = compileMdxToJs("# Title", { mdastPlugins: [plugin] });
    expect(typeof result).toBe("string");
  });

  test("async HAST visitor — replaces element after await", async () => {
    const plugin = defineHastPlugin({
      name: "async-replace",
      createOnce: () => ({
        element: {
          filter: ["pre"],
          async visit(node: Element, ctx: HastVisitorContext) {
            // Simulate async work (e.g. shiki language loading)
            await new Promise((r) => setTimeout(r, 1));
            ctx.replaceNode(node, { type: "raw", value: "<pre>highlighted</pre>" } as HastNode);
          },
        },
      }),
    });

    const result = compileMarkdownToHtml("```js\ncode\n```", { hastPlugins: [plugin] });
    expect(result).toBeInstanceOf(Promise);
    const html = await result;
    expect(html).toContain("highlighted");
  });

  test("async HAST visitor — multiple async visitors run in parallel", async () => {
    const order: string[] = [];
    const plugin = defineHastPlugin({
      name: "async-parallel",
      createOnce: () => ({
        element: {
          filter: ["h1", "h2"],
          async visit(node: Element, ctx: HastVisitorContext) {
            const tag = node.tagName;
            order.push(`start:${tag}`);
            await new Promise((r) => setTimeout(r, tag === "h1" ? 10 : 1));
            order.push(`end:${tag}`);
            ctx.setProperty(node, "class", "processed");
          },
        },
      }),
    });

    const html = await compileMarkdownToHtml("# One\n\n## Two", { hastPlugins: [plugin] });
    expect(html).toContain('class="processed"');
    // Both should start before either ends (parallel execution)
    expect(order[0]).toBe("start:h1");
    expect(order[1]).toBe("start:h2");
  });

  test("mixed sync and async plugins — sync has zero overhead", async () => {
    const syncPlugin = defineHastPlugin({
      name: "sync-class",
      createOnce: () => ({
        element: {
          filter: ["h1"],
          visit(node: Element, ctx: HastVisitorContext) {
            ctx.setProperty(node, "id", "sync");
          },
        },
      }),
    });

    const asyncPlugin = defineHastPlugin({
      name: "async-class",
      createOnce: () => ({
        element: {
          filter: ["p"],
          async visit(node: Element, ctx: HastVisitorContext) {
            await new Promise((r) => setTimeout(r, 1));
            ctx.setProperty(node, "class", "async");
          },
        },
      }),
    });

    const result = compileMarkdownToHtml("# Title\n\nParagraph", {
      hastPlugins: [syncPlugin, asyncPlugin],
    });
    const html = await result;
    expect(html).toContain('id="sync"');
    expect(html).toContain('class="async"');
  });

  test("sync-only plugins still return string (not Promise)", () => {
    const plugin = defineHastPlugin({
      name: "sync-only",
      createOnce: () => ({
        element: {
          filter: ["h1"],
          visit(node: Element, ctx: HastVisitorContext) {
            ctx.setProperty(node, "id", "test");
          },
        },
      }),
    });

    const result = compileMarkdownToHtml("# Hello", { hastPlugins: [plugin] });
    expect(typeof result).toBe("string");
    expect(result).toContain('id="test"');
  });

  test("mdast setProperty + returning same node preserves mutation", () => {
    const plugin = defineMdastPlugin({
      name: "bump-heading",
      createOnce: () => ({
        heading(node: MdastNode, ctx) {
          if ((node as { depth: number }).depth < 6) {
            ctx.setProperty(node, "depth", (node as { depth: number }).depth + 1);
          }
          return node; // returning same node should NOT clobber setProperty
        },
      }),
    });

    const html = compileMarkdownToHtml("# Hello", { mdastPlugins: [plugin] });
    expect(html).toContain("<h2>");
    expect(html).not.toContain("<h1>");
  });

  test("hast setProperty on text node updates value", () => {
    const plugin = defineHastPlugin({
      name: "uppercase-text",
      createOnce: () => ({
        text(node: HastNode, ctx: HastVisitorContext) {
          ctx.setProperty(node, "value", (node as unknown as { value: string }).value.toUpperCase());
        },
      }),
    });

    const html = compileMarkdownToHtml("hello world", { hastPlugins: [plugin] });
    expect(html).toContain("HELLO WORLD");
  });

});
