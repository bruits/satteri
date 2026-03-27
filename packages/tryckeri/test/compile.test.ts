import { describe, test, expect } from "vitest";
import {
  compileMarkdownToHtml,
  compileMdxToJs,
  defineMdastPlugin,
  defineHastPlugin,
} from "../src/index.js";
import type { HastNode } from "../src/hast-materializer.js";
import type { HastVisitorContext } from "../src/hast-visitor.js";
import type { MdastNode } from "../src/types.js";

// ---------------------------------------------------------------------------
// compileMarkdownToHtml — no plugins
// ---------------------------------------------------------------------------

describe("compileMarkdownToHtml", () => {
  test("basic markdown to HTML", async () => {
    const html = await compileMarkdownToHtml("# Hello\n\nWorld");
    expect(html).toContain("<h1>");
    expect(html).toContain("Hello");
    expect(html).toContain("<p>");
    expect(html).toContain("World");
  });

  test("empty string produces empty output", async () => {
    const html = await compileMarkdownToHtml("");
    expect(html).toBe("");
  });

  test("inline formatting", async () => {
    const html = await compileMarkdownToHtml("**bold** and *italic*");
    expect(html).toContain("<strong>bold</strong>");
    expect(html).toContain("<em>italic</em>");
  });

  test("link renders as anchor", async () => {
    const html = await compileMarkdownToHtml("[click](https://example.com)");
    expect(html).toContain('<a href="https://example.com">click</a>');
  });

  test("code block with language", async () => {
    const html = await compileMarkdownToHtml("```js\nconsole.log(1)\n```");
    expect(html).toContain('<code class="language-js">');
    expect(html).toContain("console.log(1)");
  });

  // ---------------------------------------------------------------------------
  // with MDAST plugins only
  // ---------------------------------------------------------------------------

  test("MDAST plugin removes headings", async () => {
    const removeHeadings = defineMdastPlugin({
      name: "remove-headings",
      createOnce: () => ({
        heading(node: MdastNode, ctx: { removeNode(n: MdastNode): void }) {
          ctx.removeNode(node);
        },
      }),
    });

    const html = await compileMarkdownToHtml("# Title\n\nKeep this", {
      mdastPlugins: [removeHeadings],
    });
    expect(html).not.toContain("<h1>");
    expect(html).not.toContain("Title");
    expect(html).toContain("Keep this");
  });

  test("MDAST plugin replaces text with raw markdown", async () => {
    const uppercaseHeadings = defineMdastPlugin({
      name: "uppercase-headings",
      createOnce: () => ({
        heading(_node: MdastNode) {
          return { raw: "# REPLACED" };
        },
      }),
    });

    const html = await compileMarkdownToHtml("# Original\n\npara", {
      mdastPlugins: [uppercaseHeadings],
    });
    expect(html).toContain("REPLACED");
    expect(html).not.toContain("Original");
  });

  // ---------------------------------------------------------------------------
  // with HAST plugins only
  // ---------------------------------------------------------------------------

  test("HAST plugin adds class to all elements", async () => {
    const addClasses = defineHastPlugin({
      name: "add-classes",
      createOnce: () => ({
        element(node: HastNode, ctx: HastVisitorContext) {
          ctx.setProperty(node, "class", "styled");
        },
      }),
    });

    const html = await compileMarkdownToHtml("# Hello\n\nWorld", {
      hastPlugins: [addClasses],
    });
    expect(html).toContain('<h1 class="styled">');
    expect(html).toContain('<p class="styled">');
  });

  test("HAST plugin removes elements", async () => {
    const removeHeadings = defineHastPlugin({
      name: "remove-h1",
      createOnce: () => ({
        element(node: HastNode, ctx: HastVisitorContext) {
          if (node.tagName === "h1") {
            ctx.removeNode(node);
          }
        },
      }),
    });

    const html = await compileMarkdownToHtml("# Gone\n\nStays", {
      hastPlugins: [removeHeadings],
    });
    expect(html).not.toContain("<h1>");
    expect(html).not.toContain("Gone");
    expect(html).toContain("Stays");
  });

  test("HAST plugin replaces element via return value", async () => {
    const replaceH1 = defineHastPlugin({
      name: "demote-h1",
      createOnce: () => ({
        element(node: HastNode) {
          if (node.tagName === "h1") {
            return {
              type: "element",
              _nodeId: -1,
              tagName: "h2",
              properties: { class: "demoted" },
              children: node.children ?? [],
              data: null,
            };
          }
        },
      }),
    });

    const html = await compileMarkdownToHtml("# Title", {
      hastPlugins: [replaceH1],
    });
    expect(html).toContain("<h2");
    expect(html).toContain('class="demoted"');
    expect(html).toContain("Title");
    expect(html).not.toContain("<h1");
  });

  test("HAST plugin sets id on heading", async () => {
    const addIds = defineHastPlugin({
      name: "add-ids",
      createOnce: () => ({
        element(node: HastNode, ctx: HastVisitorContext) {
          if (node.tagName === "h1") {
            ctx.setProperty(node, "id", "main-title");
          }
        },
      }),
    });

    const html = await compileMarkdownToHtml("# Hello", {
      hastPlugins: [addIds],
    });
    expect(html).toContain('id="main-title"');
  });

  test("HAST plugin wraps text in span via transformRoot", async () => {
    const wrapTexts = defineHastPlugin({
      name: "wrap-texts",
      createOnce: () => ({
        transformRoot(root: HastNode) {
          function walk(node: HastNode): HastNode {
            if (node.type === "text") {
              return {
                type: "element",
                _nodeId: -1,
                tagName: "span",
                properties: { class: "text-wrap" },
                children: [node],
                data: null,
              };
            }
            if (node.children) {
              return { ...node, children: node.children.map(walk) };
            }
            return node;
          }
          return walk(root);
        },
      }),
    });

    const html = await compileMarkdownToHtml("Hello", {
      hastPlugins: [wrapTexts],
    });
    expect(html).toContain('<span class="text-wrap">Hello</span>');
  });

  test("no mutations — fast Rust path still works", async () => {
    const noopPlugin = defineHastPlugin({
      name: "noop",
      createOnce: () => ({
        element() {
          // inspect but don't mutate
        },
      }),
    });

    const html = await compileMarkdownToHtml("# Test\n\nParagraph", {
      hastPlugins: [noopPlugin],
    });
    expect(html).toContain("<h1>");
    expect(html).toContain("Test");
    expect(html).toContain("<p>");
  });

  // ---------------------------------------------------------------------------
  // with both MDAST and HAST plugins
  // ---------------------------------------------------------------------------

  test("MDAST plugin removes headings, HAST plugin adds class", async () => {
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
        element(node: HastNode, ctx: HastVisitorContext) {
          ctx.setProperty(node, "class", "styled");
        },
      }),
    });

    const html = await compileMarkdownToHtml("# Gone\n\nKeep", {
      mdastPlugins: [removeHeadings],
      hastPlugins: [addClasses],
    });
    expect(html).not.toContain("<h1>");
    expect(html).toContain('<p class="styled">');
    expect(html).toContain("Keep");
  });

  test("multiple HAST plugins compose", async () => {
    const addIds = defineHastPlugin({
      name: "add-ids",
      createOnce: () => ({
        element(node: HastNode, ctx: HastVisitorContext) {
          if (node.tagName === "h1") {
            ctx.setProperty(node, "id", "title");
          }
        },
      }),
    });

    const addClasses = defineHastPlugin({
      name: "add-classes",
      createOnce: () => ({
        element(node: HastNode, ctx: HastVisitorContext) {
          ctx.setProperty(node, "class", "styled");
        },
      }),
    });

    const html = await compileMarkdownToHtml("# Hello", {
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
  test("basic MDX compilation", async () => {
    const js = await compileMdxToJs("# Hello\n\nWorld");
    expect(js).toContain("function");
    expect(js).toContain("Hello");
  });

  test("MDX with JSX element", async () => {
    const js = await compileMdxToJs("<MyComponent />", {});
    expect(js).toContain("MyComponent");
  });

  test("MDAST plugin affects MDX output", async () => {
    const removeHeadings = defineMdastPlugin({
      name: "remove-headings",
      createOnce: () => ({
        heading(node: MdastNode, ctx: { removeNode(n: MdastNode): void }) {
          ctx.removeNode(node);
        },
      }),
    });

    const js = await compileMdxToJs("# Gone\n\nKept", {
      mdastPlugins: [removeHeadings],
    });
    expect(js).not.toContain("Gone");
    expect(js).toContain("Kept");
  });

  test("MDAST plugin can read JSX attributes", async () => {
    const collected: unknown[] = [];
    const readAttrs = defineMdastPlugin({
      name: "read-attrs",
      createOnce: () => ({
        mdxJsxFlowElement(node: MdastNode) {
          collected.push({
            name: node.name,
            attributes: node.attributes,
          });
        },
      }),
    });

    await compileMdxToJs('<Component foo="bar" disabled count={42} />', {
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

  test("MDAST plugin can replace JSX element with modified attributes", async () => {
    const addAttr = defineMdastPlugin({
      name: "add-attr",
      createOnce: () => ({
        mdxJsxFlowElement(node: MdastNode) {
          if (node.name === "Component") {
            return {
              type: "mdxJsxFlowElement",
              name: "Component",
              attributes: [
                { type: "mdxJsxAttribute", name: "added", value: "yes" },
              ],
              children: [],
            };
          }
        },
      }),
    });

    const js = await compileMdxToJs("<Component />\n", {
      mdastPlugins: [addAttr],
    });
    // The compiled output should reference the "added" attribute
    expect(js).toContain("added");
    expect(js).toContain("yes");
  });

  test("MDAST plugin can replace JSX element removing all attributes", async () => {
    const stripAttrs = defineMdastPlugin({
      name: "strip-attrs",
      createOnce: () => ({
        mdxJsxFlowElement(node: MdastNode) {
          if (node.name === "Component") {
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

    const js = await compileMdxToJs('<Component foo="bar" />\n', {
      mdastPlugins: [stripAttrs],
    });
    expect(js).toContain("Component");
    expect(js).not.toContain("foo");
    expect(js).not.toContain("bar");
  });

  test("HAST plugin setProperty on MDX JSX element preserves existing attributes", async () => {
    const injectMeta = defineHastPlugin({
      name: "inject-meta",
      createOnce: () => ({
        mdxJsxTextElement(node: HastNode, ctx: HastVisitorContext) {
          ctx.setProperty(node, "client:component-path", "/absolute/path/B.jsx");
          ctx.setProperty(node, "client:component-export", "default");
          ctx.setProperty(node, "client:component-hydration", "");
        },
      }),
    });

    const js = await compileMdxToJs(
      'import B from "./B.jsx"\n\n<B client:load foo="bar">hi</B>',
      { hastPlugins: [injectMeta] },
    );

    // Original attributes must be preserved
    expect(js).toContain('"client:load": true');
    expect(js).toContain('foo: "bar"');
    // Injected attributes must appear
    expect(js).toContain('"client:component-path": "/absolute/path/B.jsx"');
    expect(js).toContain('"client:component-export": "default"');
    expect(js).toContain('"client:component-hydration": ""');
  });

  test("HAST plugin setProperty on MDX JSX element — no-op plugin preserves all attributes", async () => {
    const noop = defineHastPlugin({
      name: "noop",
      createOnce: () => ({
        mdxJsxTextElement() {
          // do nothing
        },
      }),
    });

    const withPlugin = await compileMdxToJs(
      'import B from "./B.jsx"\n\n<B client:load foo="bar">hi</B>',
      { hastPlugins: [noop] },
    );
    const without = await compileMdxToJs(
      'import B from "./B.jsx"\n\n<B client:load foo="bar">hi</B>',
    );

    expect(withPlugin).toBe(without);
  });

  test("HAST plugin setProperty overwrites existing MDX JSX attribute", async () => {
    const overwrite = defineHastPlugin({
      name: "overwrite-attr",
      createOnce: () => ({
        mdxJsxTextElement(node: HastNode, ctx: HastVisitorContext) {
          ctx.setProperty(node, "foo", "replaced");
        },
      }),
    });

    const js = await compileMdxToJs(
      'import B from "./B.jsx"\n\n<B foo="bar">hi</B>',
      { hastPlugins: [overwrite] },
    );

    expect(js).toContain('foo: "replaced"');
    expect(js).not.toContain('"bar"');
  });

  // ---------------------------------------------------------------------------
  // optimizeStatic
  // ---------------------------------------------------------------------------

  test("optimizeStatic collapses static subtrees (Astro-style)", async () => {
    const js = await compileMdxToJs("# Hello\n\nWorld", {
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

  test("optimizeStatic React-style with wrapPropValue", async () => {
    const js = await compileMdxToJs("# Hello", {
      optimizeStatic: {
        component: "div",
        prop: "dangerouslySetInnerHTML",
        wrapPropValue: true,
      },
    });
    expect(js).toContain("dangerouslySetInnerHTML");
    expect(js).toContain("__html");
  });

  test("optimizeStatic preserves dynamic MDX components", async () => {
    const js = await compileMdxToJs("# Static\n\n<Dynamic />\n\nAlso static", {
      optimizeStatic: {
        component: "Fragment",
        prop: "set:html",
      },
    });
    expect(js).toContain("set:html");
    expect(js).toContain("Dynamic");
  });

  test("optimizeStatic off by default", async () => {
    const js = await compileMdxToJs("# Hello\n\nWorld");
    expect(js).not.toContain("set:html");
    expect(js).toContain('"h1"');
  });

  test("async HAST plugin works", async () => {
    const asyncPlugin = defineHastPlugin({
      name: "async-class-adder",
      createOnce: () => ({
        async element(node: HastNode, ctx: HastVisitorContext) {
          // Simulate async work
          await new Promise((resolve) => setTimeout(resolve, 1));
          ctx.setProperty(node, "class", "async-added");
        },
      }),
    });

    const html = await compileMarkdownToHtml("# Hello", {
      hastPlugins: [asyncPlugin],
    });
    expect(html).toContain('class="async-added"');
  });
});
