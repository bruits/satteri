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

// markdownToHtml - no plugins

describe("markdownToHtml", () => {
  test("basic markdown to HTML", () => {
    const html = markdownToHtml("# Hello\n\nWorld");
    expect(html).toContain("<h1>");
    expect(html).toContain("Hello");
    expect(html).toContain("<p>");
    expect(html).toContain("World");
  });

  test("empty string produces empty output", () => {
    const html = markdownToHtml("");
    expect(html).toBe("");
  });

  test("inline formatting", () => {
    const html = markdownToHtml("**bold** and *italic*");
    expect(html).toContain("<strong>bold</strong>");
    expect(html).toContain("<em>italic</em>");
  });

  test("link renders as anchor", () => {
    const html = markdownToHtml("[click](https://example.com)");
    expect(html).toContain('<a href="https://example.com">click</a>');
  });

  test("code block with language", () => {
    const html = markdownToHtml("```js\nconsole.log(1)\n```");
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

    const html = markdownToHtml("# Title\n\nKeep this", {
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

    const html = markdownToHtml("# Original\n\npara", {
      mdastPlugins: [uppercaseHeadings],
    });
    expect(html).toContain("REPLACED");
    expect(html).not.toContain("Original");
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

    const html = markdownToHtml("# Hello\n\nWorld", {
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

    const html = markdownToHtml("# Gone\n\nStays", {
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

    const html = markdownToHtml("# Title", {
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

    const html = markdownToHtml("# Hello", {
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

    const html = markdownToHtml("# Test\n\nParagraph", {
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

    const html = markdownToHtml("# Gone\n\nKeep", {
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

    const html = markdownToHtml("# Hello", {
      hastPlugins: [addIds, addClasses],
    });
    expect(html).toContain('id="title"');
    expect(html).toContain('class="styled"');
  });
});

// mdxToJs

describe("mdxToJs", () => {
  test("basic MDX compilation", () => {
    const js = mdxToJs("# Hello\n\nWorld");
    expect(js).toContain("function");
    expect(js).toContain("Hello");
  });

  test("MDX with JSX element", () => {
    const js = mdxToJs("<MyComponent />", {});
    expect(js).toContain("MyComponent");
  });

  test("MDAST plugin affects MDX output", () => {
    const removeHeadings = defineMdastPlugin({
      name: "remove-headings",
      heading(node, ctx) {
        ctx.removeNode(node);
      },
    });

    const js = mdxToJs("# Gone\n\nKept", {
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

    const js = mdxToJs("<Component />\n", {
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

    const js = mdxToJs('<Component foo="bar" />\n', {
      mdastPlugins: [stripAttrs],
    });
    expect(js).toContain("Component");
    expect(js).not.toContain("foo");
    expect(js).not.toContain("bar");
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

    const js = mdxToJs('import B from "./B.jsx"\n\n<B client:load foo="bar">hi</B>', {
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

    const withPlugin = mdxToJs('import B from "./B.jsx"\n\n<B client:load foo="bar">hi</B>', {
      hastPlugins: [noop],
    });
    const without = mdxToJs('import B from "./B.jsx"\n\n<B client:load foo="bar">hi</B>');

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

    const js = mdxToJs('import B from "./B.jsx"\n\n<B foo="bar">hi</B>', {
      hastPlugins: [overwrite],
    });

    expect(js).toContain('foo: "replaced"');
    expect(js).not.toContain('"bar"');
  });

  // optimizeStatic

  test("optimizeStatic collapses static subtrees (Astro-style)", () => {
    const js = mdxToJs("# Hello\n\nWorld", {
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
    const js = mdxToJs("# Hello", {
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
    const js = mdxToJs("# Static\n\n<Dynamic />\n\nAlso static", {
      optimizeStatic: {
        component: "Fragment",
        prop: "set:html",
      },
    });
    expect(js).toContain("set:html");
    expect(js).toContain("Dynamic");
  });

  test("optimizeStatic off by default", () => {
    const js = mdxToJs("# Hello\n\nWorld");
    expect(js).not.toContain("set:html");
    expect(js).toContain('"h1"');
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

    const js = mdxToJs("```js\nconst x = {foo: 1}\n```", {
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

    const js = mdxToJs("```js\nconst x = {\n  foo: 1\n}\n```", {
      mdastPlugins: [plugin],
    });

    expect(js).toContain("const");
    expect(js).toContain("foo");
    expect(js).toContain("shiki");
    expect(js).not.toContain("Could not parse");
  });

  test("MDX expression in heading is preserved", () => {
    const js = mdxToJs("# {title}");
    expect(js).toContain("children: title");
  });

  test("MDX expression mixed with text in heading", () => {
    const js = mdxToJs("## Hello {name}");
    expect(js).toContain('"Hello "');
    expect(js).toContain("name");
  });

  test("MDX frontmatter expression in heading", () => {
    const js = mdxToJs("# {frontmatter.title}");
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

    const html = markdownToHtml("# Hello", {
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

    const html = markdownToHtml("# Hello\n\n[click](https://example.com)", {
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

    const html = markdownToHtml("# One\n\n## Two\n\nParagraph", {
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

    const html = markdownToHtml("# Title\n\n[link](https://example.com)", {
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

    const html = markdownToHtml("# Hello", {
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
    const js = await result;
    expect(js).toContain("async-highlighted");
  });

  test("sync MDAST plugins return string not Promise", () => {
    const plugin = defineMdastPlugin({
      name: "sync-mdast",
      heading(node, ctx) {
        ctx.setProperty(node, "depth", 2);
      },
    });

    const result = mdxToJs("# Title", { mdastPlugins: [plugin] });
    expect(typeof result).toBe("string");
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
    const html = await result;
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

    const html = await markdownToHtml("# One\n\n## Two", { hastPlugins: [plugin] });
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
    const html = await result;
    expect(html).toContain('id="sync"');
    expect(html).toContain('class="async"');
  });

  test("sync-only plugins still return string (not Promise)", () => {
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
    expect(typeof result).toBe("string");
    expect(result).toContain('id="test"');
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

    const html = markdownToHtml("# Hello", { mdastPlugins: [plugin] });
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

    const html = markdownToHtml("hello world", { hastPlugins: [plugin] });
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
    const html = markdownToHtml(
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

    const html = markdownToHtml("# Hello", { mdastPlugins: [plugin] });
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

    const html = markdownToHtml("Hello :wave: world :wave:", {
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
    const html = await result;
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

    const html = markdownToHtml("![alt text](https://example.com/img.png)", {
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
    const html = markdownToHtml(input, {
      features: { smartPunctuation: true },
    }) as string;
    expect(html).toContain("\u201c");
    expect(html).toContain("\u201d");
    expect(html).toContain("\u2013");
    expect(html).toContain("\u2026");
    expect(html).not.toContain("--");
    expect(html).not.toContain("...");
  });

  test("boolean false disables all smart punctuation", () => {
    const html = markdownToHtml(input, {
      features: { smartPunctuation: false },
    }) as string;
    expect(html).toContain('"');
    expect(html).toContain("--");
    expect(html).toContain("...");
  });

  test("quotes only", () => {
    const html = markdownToHtml(input, {
      features: { smartPunctuation: { quotes: true, dashes: false, ellipses: false } },
    }) as string;
    expect(html).toContain("\u201c");
    expect(html).toContain("--");
    expect(html).toContain("...");
  });

  test("dashes only", () => {
    const html = markdownToHtml(input, {
      features: { smartPunctuation: { quotes: false, dashes: true, ellipses: false } },
    }) as string;
    expect(html).toContain('"');
    expect(html).toContain("\u2013");
    expect(html).toContain("...");
  });

  test("ellipses only", () => {
    const html = markdownToHtml(input, {
      features: { smartPunctuation: { quotes: false, dashes: false, ellipses: true } },
    }) as string;
    expect(html).toContain('"');
    expect(html).toContain("--");
    expect(html).toContain("\u2026");
  });

  test("omitted fields default to true", () => {
    const html = markdownToHtml(input, {
      features: { smartPunctuation: { dashes: false } },
    }) as string;
    expect(html).toContain("\u201c");
    expect(html).toContain("--");
    expect(html).toContain("\u2026");
  });

  test("empty object enables all", () => {
    const all = markdownToHtml(input, {
      features: { smartPunctuation: true },
    }) as string;
    const empty = markdownToHtml(input, {
      features: { smartPunctuation: {} },
    }) as string;
    expect(empty).toBe(all);
  });

  test("granular options work with mdxToJs", () => {
    const js = mdxToJs(input, {
      features: { smartPunctuation: { dashes: false } },
    }) as string;
    expect(js).toContain("\u201c");
    expect(js).toContain("--");
    expect(js).toContain("\u2026");
  });
});
