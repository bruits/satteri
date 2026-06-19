import { describe, test, expect } from "vitest";
import { HastReader } from "../src/hast/hast-reader.js";
import { visitHastHandle, resolveSubscriptions } from "../src/hast/hast-visitor.js";
import { materializeHastTree } from "../src/hast/hast-materializer.js";
import {
  createHastHandle,
  createMdxHastHandle,
  serializeHandle,
  renderHandle,
  getHandleSource,
} from "../index.js";
import { defineHastPlugin } from "../src/plugin.js";
import { dropHandle } from "../src/index.js";
import { collect } from "./fixtures.js";
import type { HastNode } from "../src/hast/hast-materializer.js";
import type { HastVisitorContext } from "../src/hast/hast-visitor.js";
import type { Element, ElementContent, Text } from "hast";
import type { Position } from "unist";

function setup(source = "# Hello\n\nWorld") {
  const handle = createHastHandle(source);
  const src = getHandleSource(handle);
  return { handle, source: src };
}

// Basic visitor behaviour (handle-based)

describe("visitHastHandle - basic behaviour", () => {
  test("visitor with no subscriptions produces no mutations", () => {
    const { handle, source } = setup();
    const plugin = {};
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    // No crash, no mutations, handle still renders
    expect(renderHandle(handle)).toContain("Hello");
  });

  test("element() callback fires for each element node", () => {
    const { handle, source } = setup();
    const tags: string[] = [];
    const plugin = {
      element: {
        filter: [] as string[],
        visit(node: HastNode) {
          if (node.type === "element") tags.push(node.tagName);
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    expect(tags).toContain("h1");
    expect(tags).toContain("p");
  });

  test("text() callback fires for each text node", () => {
    const { handle, source } = setup();
    const texts: string[] = [];
    const plugin = {
      text(node: HastNode) {
        if (node.type === "text") texts.push(node.value);
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    expect(texts).toContain("Hello");
    expect(texts).toContain("World");
  });

  test("matched element nodes carry a source position", () => {
    const { handle, source } = setup();
    const positions: Record<string, Position | undefined> = {};
    const plugin = {
      element: {
        filter: [] as string[],
        visit(node: HastNode) {
          if (node.type === "element") positions[node.tagName] = node.position;
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    expect(positions.h1?.start.line).toBe(1);
    expect(positions.h1?.start.column).toBe(1);
    expect(positions.p?.start.line).toBe(3);
  });

  test("matched text nodes carry a source position", () => {
    const { handle, source } = setup();
    const positions: Record<string, Position | undefined> = {};
    const plugin = {
      text(node: HastNode) {
        if (node.type === "text") positions[node.value] = node.position;
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    expect(positions.Hello?.start.line).toBe(1);
    expect(positions.World?.start.line).toBe(3);
  });
});

// Mutations (end-to-end via handle)

describe("visitHastHandle - mutations", () => {
  test("returning a node from element() creates a replace mutation", () => {
    const { handle, source } = setup();
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode) {
          if (node.type === "element" && node.tagName === "h1") {
            return {
              type: "element" as const,
              tagName: "h2",
              properties: {},
              children: node.children ?? [],
              data: undefined,
              _nodeId: -1,
            };
          }
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h2>");
    expect(html).not.toContain("<h1>");
  });

  // Regression: returning a wrapper whose children include the visited node
  // re-parents it. The node is spliced back by reference, and the rebuild must
  // splice it once rather than re-applying the replacement and recursing.
  test("element() may re-parent the visited node into its replacement", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "reparent-heading-into-div",
      element: {
        filter: ["h1"],
        visit(node) {
          return {
            type: "element",
            tagName: "div",
            properties: {},
            children: [node],
          };
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html.trim()).toBe("<div><h1>Hello</h1></div>");
  });

  // The real Starlight autolink-headings shape: wrap each heading in a <div>
  // holding the original heading plus a freshly built anchor-link sibling.
  test("element() re-parents the visited node alongside a freshly built sibling", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "reparent-heading-with-anchor",
      element: {
        filter: ["h1"],
        visit(node) {
          return {
            type: "element",
            tagName: "div",
            properties: { className: ["heading-wrapper"] },
            children: [
              node,
              {
                type: "element",
                tagName: "a",
                properties: { href: "#hello" },
                children: [{ type: "text", value: "#" }],
              },
            ],
          };
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain('<div class="heading-wrapper">');
    expect(html).toContain("<h1>Hello</h1>");
    expect(html).toContain('<a href="#hello">#</a>');
  });

  test("context.removeNode() removes a node", () => {
    const { handle, source } = setup();
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode, ctx: HastVisitorContext) {
          ctx.removeNode(node);
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).not.toContain("<h1>");
    expect(html).toContain("World");
  });

  test("context.setProperty() modifies element attributes", () => {
    const { handle, source } = setup();
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode, ctx: HastVisitorContext) {
          ctx.setProperty(node, "id", "title");
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain('id="title"');
  });

  test("context.setProperty() modifies text node value", () => {
    const { handle, source } = setup("hello");
    const plugin = {
      text(node: HastNode, ctx: HastVisitorContext) {
        ctx.setProperty(node, "value", (node as unknown as { value: string }).value.toUpperCase());
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("HELLO");
    expect(html).not.toContain("hello");
  });

  test("context.insertAfter() inserts a sibling element", () => {
    const { handle, source } = setup("# Hello");
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode, ctx: HastVisitorContext) {
          ctx.insertAfter(node, {
            type: "element",
            tagName: "hr",
            properties: {},
            children: [],
          });
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>Hello</h1><hr>");
  });

  test("context.wrapNode() wraps an element", () => {
    const { handle, source } = setup("# Hello");
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode, ctx: HastVisitorContext) {
          ctx.wrapNode(node, {
            type: "element",
            tagName: "div",
            properties: {},
            children: [],
          });
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<div><h1>Hello</h1></div>");
  });

  // A wrapper may declare its own children; they are kept as siblings after the
  // wrapped node. The Starlight autolink shape (heading + anchor link),
  // expressed without re-parenting the visited node into a replacement.
  test("context.wrapNode() keeps the wrapper's own children after the wrapped node", () => {
    const { handle, source } = setup("# Hello");
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode, ctx: HastVisitorContext) {
          ctx.wrapNode(node, {
            type: "element",
            tagName: "div",
            properties: { className: ["heading-wrapper"] },
            children: [
              {
                type: "element",
                tagName: "a",
                properties: { href: "#hello" },
                children: [{ type: "text", value: "#" }],
              },
            ],
          });
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain(
      '<div class="heading-wrapper"><h1>Hello</h1><a href="#hello">#</a></div>',
    );
  });

  test("context.appendChild() adds a child to an element", () => {
    const { handle, source } = setup("# Hello");
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode, ctx: HastVisitorContext) {
          ctx.appendChild(node, { type: "text", value: "!" });
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>Hello!</h1>");
  });

  test("context.insertBefore() inserts a sibling before an element", () => {
    const { handle, source } = setup("# Hello");
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode, ctx: HastVisitorContext) {
          ctx.insertBefore(node, {
            type: "element",
            tagName: "hr",
            properties: {},
            children: [],
          });
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<hr><h1>Hello</h1>");
  });

  test("context.prependChild() adds a child at the start of an element", () => {
    const { handle, source } = setup("# Hello");
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode, ctx: HastVisitorContext) {
          ctx.prependChild(node, { type: "text", value: ">> " });
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>&gt;&gt; Hello</h1>");
  });

  test("context.replaceNode() replaces a node via context method", () => {
    const { handle, source } = setup("# Hello");
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode, ctx: HastVisitorContext) {
          ctx.replaceNode(node, {
            type: "element",
            tagName: "h3",
            properties: {},
            children: [{ type: "text", value: "Replaced" }],
          });
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h3>Replaced</h3>");
    expect(html).not.toContain("<h1>");
  });

  test("mutations work on child nodes accessed via node.children", () => {
    const { handle, source } = setup("# Hello");
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode, ctx: HastVisitorContext) {
          const textChild = "children" in node ? node.children?.[0] : undefined;
          if (textChild) {
            ctx.insertAfter(textChild, { type: "text", value: " World" });
          }
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>Hello World</h1>");
  });

  test("context.setProperty(node, 'children', ...) replaces an element's children", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "replace-heading-children",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.setProperty(node, "children", [{ type: "text", value: "New heading" }]);
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>New heading</h1>");
  });

  test("context.setProperty 'children' composes with a scalar setProperty on the same node", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "set-id-and-replace-children",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.setProperty(node, "id", "title");
          ctx.setProperty(node, "children", [{ type: "text", value: "Hi" }]);
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain(`<h1 id="title">Hi</h1>`);
  });

  test("context.setProperty(node, 'children', ...) preserves the element's properties", () => {
    const { handle, source } = setup("[x](https://example.com)");
    const plugin = defineHastPlugin({
      name: "replace-anchor-children",
      element: {
        filter: ["a"],
        visit(node, ctx) {
          ctx.setProperty(node, "children", [{ type: "text", value: "Y" }]);
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain(`<a href="https://example.com">Y</a>`);
  });

  test("context.insertChildAt() prepends at index 0", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "prepend-marker-at-index-0",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.insertChildAt(node, 0, { type: "text", value: ">> " });
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>&gt;&gt; Hello</h1>");
  });

  test("context.removeChildAt() removes the index-th child of an element", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "remove-first-child",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.removeChildAt(node, 0);
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1></h1>");
  });

  test("context.appendChild() accepts an array of nodes on an element, in order", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "append-text-array",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.appendChild(node, [
            { type: "text", value: " A" },
            { type: "text", value: " B" },
          ]);
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>Hello A B</h1>");
  });

  test("context.insertChildAt() inserts before the index-th child of an element", () => {
    const { handle, source } = setup("# Hello *world*");
    const plugin = defineHastPlugin({
      name: "insert-before-second-child",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.insertChildAt(node, 1, { type: "text", value: "X" });
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>Hello X<em>world</em></h1>");
  });

  test("context.insertChildAt() appends past the end", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "append-via-clamped-index",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.insertChildAt(node, 99, { type: "text", value: "!" });
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>Hello!</h1>");
  });

  test("context.insertChildAt() accepts an array, keeping order at the index", () => {
    const { handle, source } = setup("# Hello *world*");
    const plugin = defineHastPlugin({
      name: "insert-array-between-children",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.insertChildAt(node, 1, [
            { type: "text", value: "X" },
            { type: "text", value: "Y" },
          ]);
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>Hello XY<em>world</em></h1>");
  });

  test("context.insertChildAt() treats a negative index as a prepend", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "prepend-via-negative-index",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.insertChildAt(node, -3, { type: "text", value: ">> " });
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>&gt;&gt; Hello</h1>");
  });

  test("context.setProperty(node, 'children', ...) keeps reused children", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "replace-children-keep-original",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          const original = node.children[0];
          if (original) {
            ctx.setProperty(node, "children", [{ type: "text", value: "> " }, original]);
          }
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>&gt; Hello</h1>");
  });

  test("context.setProperty(node, 'children', []) clears the children", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "clear-children",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.setProperty(node, "children", []);
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1></h1>");
  });

  test("context.removeChildAt() is a no-op for an out-of-range or negative index", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "remove-child-noop-out-of-range",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.removeChildAt(node, 9);
          ctx.removeChildAt(node, -1);
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>Hello</h1>");
  });

  test("context.insertBefore() accepts an array of siblings", () => {
    const { handle, source } = setup("# Hello\n\nWorld");
    const plugin = defineHastPlugin({
      name: "insert-two-hr-before",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.insertBefore(node, [
            { type: "element", tagName: "hr", properties: {}, children: [] },
            { type: "element", tagName: "hr", properties: {}, children: [] },
          ]);
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect((html.match(/<hr>/g) ?? []).length).toBe(2);
    expect(html.lastIndexOf("<hr>")).toBeLessThan(html.indexOf("<h1>"));
  });

  test("context.insertAfter() accepts an array of siblings", () => {
    const { handle, source } = setup("# Hello\n\nWorld");
    const plugin = defineHastPlugin({
      name: "insert-two-hr-after",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.insertAfter(node, [
            { type: "element", tagName: "hr", properties: {}, children: [] },
            { type: "element", tagName: "hr", properties: {}, children: [] },
          ]);
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect((html.match(/<hr>/g) ?? []).length).toBe(2);
    expect(html.indexOf("<hr>")).toBeGreaterThan(html.indexOf("<h1>"));
  });

  test("context.prependChild() accepts an array of nodes, in order", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "prepend-text-array",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.prependChild(node, [
            { type: "text", value: "A " },
            { type: "text", value: "B " },
          ]);
        },
      },
    });
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>A B Hello</h1>");
  });

  test("context mutations reject plugin-built nodes with no arena id", () => {
    const { handle, source } = setup();
    const plugin = defineHastPlugin({
      name: "remove-fresh-node",
      element: {
        filter: ["h1"],
        visit(_node, ctx) {
          ctx.removeNode({ type: "text", value: "x" });
        },
      },
    });
    expect(() =>
      visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined),
    ).toThrow(/no arena id/);
  });
});

// Diagnostics

describe("visitHastHandle - diagnostics", () => {
  test("context.report() collects diagnostics", () => {
    const { handle, source } = setup();
    let diags: { message: string; severity: string }[] = [];
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode, ctx: HastVisitorContext) {
          ctx.report({ message: "heading found", node, severity: "info" });
          diags = ctx.getDiagnostics();
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    expect(diags.length).toBe(1);
    expect(diags[0]!.message).toBe("heading found");
    expect(diags[0]!.severity).toBe("info");
  });
});

// Context properties

describe("visitHastHandle - context", () => {
  test("ctx.source and ctx.fileURL are available", () => {
    const handle = createHastHandle("# Hello\n\nWorld");
    // Pass the original source explicitly (the real pipeline does this)
    const originalSource = "# Hello\n\nWorld";
    let capturedSource = "";
    let capturedFileURL: URL | undefined;
    const plugin = {
      element: {
        filter: ["h1"],
        visit(_node: HastNode, ctx: HastVisitorContext) {
          capturedSource = ctx.source;
          capturedFileURL = ctx.fileURL;
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, originalSource, new URL("file:///project/test.md"));
    expect(capturedSource).toBe("# Hello\n\nWorld");
    expect(capturedFileURL?.href).toBe("file:///project/test.md");
  });

  test("ctx.textContent() returns concatenated text", () => {
    const { handle, source } = setup();
    let text = "";
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode, ctx: HastVisitorContext) {
          text = ctx.textContent(node);
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    expect(text).toBe("Hello");
  });

  test("code fence data.lang and data.meta are available on code elements", () => {
    const handle = createHastHandle("```typescript {highlight=[1]}\nconst x = 1;\n```");
    const source = "```typescript {highlight=[1]}\nconst x = 1;\n```";
    let lang: string | undefined;
    let meta: string | undefined;
    const plugin = {
      element: {
        filter: ["code"],
        visit(node: HastNode) {
          if (node.type === "element") {
            lang = (node.data as Record<string, string> | null)?.lang;
            meta = (node.data as Record<string, string> | null)?.meta;
          }
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    expect(lang).toBe("typescript");
    expect(meta).toBe("{highlight=[1]}");
  });

  test("code fence data flows through child resolution", () => {
    const handle = createHastHandle("```js\ncode\n```");
    const source = "```js\ncode\n```";
    let childLang: string | undefined;
    const plugin = {
      element: {
        filter: ["pre"],
        visit(node: HastNode) {
          if (node.type === "element") {
            const code = node.children?.[0];
            if (code?.type === "element" && code.tagName === "code") {
              childLang = (code.data as Record<string, string> | null)?.lang;
            }
          }
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    expect(childLang).toBe("js");
  });
});

// Materialize (still uses buffer path, independent of visitor changes)

describe("materializeHastTree", () => {
  test("materializes a tree from a HAST buffer", () => {
    const handle = createHastHandle("# Hello\n\nWorld");
    const reader = new HastReader(serializeHandle(handle));
    const tree = materializeHastTree(reader);
    expect(tree.type).toBe("root");
    if (tree.type !== "root") throw new Error("expected root");
    expect(tree.children).toBeDefined();
    expect(tree.children.length).toBeGreaterThan(0);
  });

  test("element nodes have tagName and properties", () => {
    const handle = createHastHandle("# Hello\n\nWorld");
    const reader = new HastReader(serializeHandle(handle));
    const tree = materializeHastTree(reader);
    if (tree.type !== "root") throw new Error("expected root");
    const h1 = tree.children.find(
      (n): n is Extract<HastNode, { type: "element" }> =>
        n.type === "element" && n.tagName === "h1",
    );
    expect(h1).toBeDefined();
    expect(h1!.type).toBe("element");
    expect(h1!.properties).toBeDefined();
  });

  test("text nodes have value", () => {
    const handle = createHastHandle("# Hello\n\nWorld");
    const reader = new HastReader(serializeHandle(handle));
    const tree = materializeHastTree(reader);
    if (tree.type !== "root") throw new Error("expected root");
    const h1 = tree.children.find(
      (n): n is Extract<HastNode, { type: "element" }> =>
        n.type === "element" && n.tagName === "h1",
    );
    expect(h1).toBeDefined();
    const textNode = h1!.children[0]!;
    expect(textNode.type).toBe("text");
    if (textNode.type === "text") {
      expect(textNode.value).toBe("Hello");
    }
  });
});

// MDX JSX attributes on HAST nodes

function findHastNode(node: HastNode, type: string): HastNode | null {
  if (node.type === type) return node;
  if ("children" in node && node.children) {
    for (const child of node.children) {
      const found = findHastNode(child as HastNode, type);
      if (found) return found;
    }
  }
  return null;
}

describe("MDX JSX attributes on HAST nodes", () => {
  test("self-closing element with no attributes", () => {
    const buf = serializeHandle(createMdxHastHandle("<Component />\n"));
    const reader = new HastReader(buf);
    const tree = materializeHastTree(reader);
    const jsx = findHastNode(tree, "mdxJsxFlowElement");
    expect(jsx).not.toBeNull();
    if (!jsx || jsx.type !== "mdxJsxFlowElement") throw new Error("expected mdxJsxFlowElement");
    expect(jsx.name).toBe("Component");
    expect(jsx.attributes).toEqual([]);
  });

  test("element with string literal attribute", () => {
    const buf = serializeHandle(createMdxHastHandle('<Component foo="bar" />\n'));
    const reader = new HastReader(buf);
    const tree = materializeHastTree(reader);
    const jsx = findHastNode(tree, "mdxJsxFlowElement");
    if (jsx?.type !== "mdxJsxFlowElement") throw new Error("expected mdxJsxFlowElement");
    expect(jsx.name).toBe("Component");
    expect(jsx.attributes).toEqual([{ type: "mdxJsxAttribute", name: "foo", value: "bar" }]);
  });

  test("element with boolean attribute", () => {
    const buf = serializeHandle(createMdxHastHandle("<Component disabled />\n"));
    const reader = new HastReader(buf);
    const tree = materializeHastTree(reader);
    const jsx = findHastNode(tree, "mdxJsxFlowElement");
    if (jsx?.type !== "mdxJsxFlowElement") throw new Error("expected mdxJsxFlowElement");
    expect(jsx.attributes).toEqual([{ type: "mdxJsxAttribute", name: "disabled", value: null }]);
  });

  test("element with expression attribute", () => {
    const buf = serializeHandle(createMdxHastHandle("<Component count={42} />\n"));
    const reader = new HastReader(buf);
    const tree = materializeHastTree(reader);
    const jsx = findHastNode(tree, "mdxJsxFlowElement");
    if (jsx?.type !== "mdxJsxFlowElement") throw new Error("expected mdxJsxFlowElement");
    expect(jsx.attributes).toEqual([
      {
        type: "mdxJsxAttribute",
        name: "count",
        value: { type: "mdxJsxAttributeValueExpression", value: "42" },
      },
    ]);
  });

  test("element with spread attribute", () => {
    const buf = serializeHandle(createMdxHastHandle("<Component {...props} />\n"));
    const reader = new HastReader(buf);
    const tree = materializeHastTree(reader);
    const jsx = findHastNode(tree, "mdxJsxFlowElement");
    if (jsx?.type !== "mdxJsxFlowElement") throw new Error("expected mdxJsxFlowElement");
    expect(jsx.attributes).toEqual([{ type: "mdxJsxExpressionAttribute", value: "...props" }]);
  });

  test("element with multiple mixed attributes", () => {
    const buf = serializeHandle(createMdxHastHandle('<Component a="1" b={2} c {...d} />\n'));
    const reader = new HastReader(buf);
    const tree = materializeHastTree(reader);
    const jsx = findHastNode(tree, "mdxJsxFlowElement");
    if (jsx?.type !== "mdxJsxFlowElement") throw new Error("expected mdxJsxFlowElement");
    expect(jsx.attributes).toHaveLength(4);
    expect(jsx.attributes[0]).toEqual({
      type: "mdxJsxAttribute",
      name: "a",
      value: "1",
    });
    expect(jsx.attributes[1]).toEqual({
      type: "mdxJsxAttribute",
      name: "b",
      value: { type: "mdxJsxAttributeValueExpression", value: "2" },
    });
    expect(jsx.attributes[2]).toEqual({
      type: "mdxJsxAttribute",
      name: "c",
      value: null,
    });
    expect(jsx.attributes[3]).toEqual({
      type: "mdxJsxExpressionAttribute",
      value: "...d",
    });
  });
});

describe("mdxjsEsm visitor", () => {
  test("mdxjsEsm callback receives import statement value", () => {
    const handle = createMdxHastHandle("import Foo from './foo'\n\n# Hello\n");
    const source = getHandleSource(handle);
    const values: string[] = [];
    const plugin = {
      mdxjsEsm(node: HastNode) {
        if ("value" in node && typeof node.value === "string") {
          values.push(node.value);
        }
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    expect(values.length).toBe(1);
    expect(values[0]).toContain("import Foo from");
  });

  test("mdxjsEsm callback receives export statement value", () => {
    const handle = createMdxHastHandle("export const x = 1\n\n# Hello\n");
    const source = getHandleSource(handle);
    const values: string[] = [];
    const plugin = {
      mdxjsEsm(node: HastNode) {
        if ("value" in node && typeof node.value === "string") {
          values.push(node.value);
        }
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    expect(values.length).toBe(1);
    expect(values[0]).toContain("export const x");
  });
});

// Lazy-children lifecycle: matched nodes resolve `.children` from a snapshot
// taken during the pass; after the pass the arena may be rebuilt with new ids,
// so a first-time read must fail loudly instead of mapping stale ids.

describe("visitHastHandle - lazy children lifecycle", () => {
  test("async visitor reads `.children` in a deferred callback", async () => {
    const { handle, source } = setup();
    let firstChild: ElementContent | undefined;
    const plugin = defineHastPlugin({
      name: "async-children-read",
      element: {
        filter: ["h1"],
        async visit(node) {
          await new Promise((r) => setTimeout(r, 1));
          firstChild = node.children[0];
        },
      },
    });
    const result = visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    expect(result).toBeInstanceOf(Promise);
    await result;
    expect(firstChild).toMatchObject({ type: "text", value: "Hello" });
  });

  test("a node retained past its visitor pass throws on its first `.children` read", () => {
    const { handle, source } = setup();
    let retained: Readonly<Element> | undefined;
    const plugin = defineHastPlugin({
      name: "retain-h1",
      element: {
        filter: ["h1"],
        visit(node) {
          retained = node;
        },
      },
    });
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    expect(retained).toBeDefined();
    expect(() => retained!.children).toThrow(/retained past its visitor pass/);
  });

  test("a retained node throws on `.children` after an async pass settles", async () => {
    const { handle, source } = setup();
    let retained: Readonly<Element> | undefined;
    const plugin = defineHastPlugin({
      name: "retain-h1-async",
      element: {
        filter: ["h1"],
        async visit(node) {
          await new Promise((r) => setTimeout(r, 1));
          retained = node;
        },
      },
    });
    await visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    expect(() => retained!.children).toThrow(/retained past its visitor pass/);
  });

  test("children materialized in-pass stay usable after the pass", () => {
    const { handle, source } = setup();
    let retained: Readonly<Element> | undefined;
    const plugin = defineHastPlugin({
      name: "read-then-retain",
      element: {
        filter: ["h1"],
        visit(node) {
          void node.children; // materialize (and cache) during the pass
          retained = node;
        },
      },
    });
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    expect(retained!.children[0]).toMatchObject({ type: "text", value: "Hello" });
  });
});

// Ref-stub children: `.children` of a matched node returns id+type stubs that
// defer the arena snapshot until a real field is read, so passthrough children
// compile to one-word refs without ever materializing.

describe("visitHastHandle - child stubs", () => {
  test("passthrough replaceNode keeps children rendering correctly", () => {
    const { handle, source } = setup("[hello **bold**](/x)");
    const plugin = defineHastPlugin({
      name: "swap-links",
      element: {
        filter: ["a"],
        visit(node, ctx) {
          ctx.replaceNode(node, {
            type: "element",
            tagName: "span",
            properties: {},
            children: node.children,
          });
        },
      },
    });
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    expect(renderHandle(handle)).toContain("<span>hello <strong>bold</strong></span>");
  });

  test("reordering and filtering stub children works", () => {
    const { handle, source } = setup("- one\n- two");
    const plugin = defineHastPlugin({
      name: "reverse-list",
      element: {
        filter: ["ul"],
        visit(node, ctx) {
          // `type` is eager on stubs: this filter needs no materialization.
          const items = node.children.filter((c) => c.type === "element");
          ctx.setProperty(node, "children", items.reverse());
        },
      },
    });
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<li>two</li>");
    expect(html).toContain("<li>one</li>");
    expect(html.indexOf("two")).toBeLessThan(html.indexOf("one"));
  });

  test("stub `.type` stays readable after the pass; first materialization throws", () => {
    const { handle, source } = setup();
    let retained: ElementContent[] = [];
    const plugin = defineHastPlugin({
      name: "retain-children",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          retained = node.children;
          // A mutation: the arena rebuilds after the pass, so stale ids must
          // refuse to materialize.
          ctx.setProperty(node, "id", "x");
        },
      },
    });
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    expect(retained).toHaveLength(1);
    const stub = retained[0]!;
    expect(stub.type).toBe("text");
    expect(() => stub.type === "text" && stub.value).toThrow(/retained past its visitor pass/);
  });

  test("a stub materialized after dropHandle throws the retention error", () => {
    const { handle, source } = setup();
    let retained: ElementContent[] = [];
    const plugin = defineHastPlugin({
      name: "retain-children-then-drop",
      element: {
        filter: ["h1"],
        visit(node) {
          retained = node.children;
        },
      },
    });
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    // The wrapped dropHandle bumps the handle epoch, so a deferred snapshot of
    // the dropped arena fails with the retention error, not a RangeError.
    dropHandle(handle);
    const stub = retained[0]!;
    expect(stub.type).toBe("text");
    expect(() => stub.type === "text" && stub.value).toThrow(/retained past its visitor pass/);
  });

  test("a spread copy of a child stub is new content, not a reused ref", () => {
    const { handle, source } = setup("# Hello");
    const plugin = defineHastPlugin({
      name: "edit-spread-stub",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          const first = node.children[0]!;
          if (first.type !== "text") return;
          // A ref here would splice the original text and drop the edit.
          const copy = { ...first, value: "Edited" };
          ctx.replaceNode(node, {
            type: "element",
            tagName: "h2",
            properties: {},
            children: [copy],
          });
        },
      },
    });
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    expect(renderHandle(handle)).toContain("<h2>Edited</h2>");
  });
});

test("a spread copy of a matched node is new content, not a reused ref", () => {
  const { handle, source } = setup("# Hello");
  const plugin = defineHastPlugin({
    name: "replace-with-edited-spread-copy",
    element: {
      filter: ["h1"],
      visit(node, ctx) {
        // Spread copies must not inherit arena identity: an inherited id would
        // splice the original node and silently drop the copy's edits.
        const copy = { ...node, tagName: "h2", properties: {}, children: [...node.children] };
        ctx.replaceNode(node, {
          type: "element",
          tagName: "section",
          properties: {},
          children: [copy],
        });
      },
    },
  });
  const subs = resolveSubscriptions(plugin);
  visitHastHandle(handle, plugin, subs, source, undefined);
  const html = renderHandle(handle);
  expect(html).toContain("<section><h2>Hello</h2></section>");
});

test("a bare spread replacement keeps properties and children without re-specifying them", () => {
  const { handle, source } = setup('[hello **bold**](/x "T")');
  let copyKeys: string[] = [];
  const plugin = defineHastPlugin({
    name: "replace-with-bare-spread-copy",
    element: {
      filter: ["a"],
      visit(node, ctx) {
        // `properties`/`children` are own enumerable getters on matched nodes,
        // so a plain spread must carry them — and none of the internals.
        const copy = { ...node, tagName: "h2" };
        copyKeys = Object.keys(copy);
        ctx.replaceNode(node, copy);
      },
    },
  });
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
  expect(copyKeys).toEqual(expect.arrayContaining(["properties", "children"]));
  expect(copyKeys.filter((k) => k.startsWith("_"))).toEqual([]);
  const tree = materializeHastTree(new HastReader(serializeHandle(handle)));
  const h2 = collect(
    tree,
    (n): n is Element => n.type === "element" && (n as Element).tagName === "h2",
  )[0]!;
  expect(h2.properties).toEqual({ href: "/x", title: "T" });
  expect(h2.children).toMatchObject([
    { type: "text", value: "hello " },
    { type: "element", tagName: "strong" },
  ]);
});

test("a bare doctype as replacement content fails loudly", () => {
  const { handle, source } = setup();
  const plugin = defineHastPlugin({
    name: "doctype-as-content",
    element: {
      filter: ["p"],
      visit() {
        return { type: "doctype" } satisfies HastNode;
      },
    },
  });
  expect(() =>
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined),
  ).toThrow(/cannot encode replacement content of type "doctype"/);
});

test("a bare root as replacement content fails loudly", () => {
  const { handle, source } = setup();
  const plugin = defineHastPlugin({
    name: "root-as-content",
    element: {
      filter: ["p"],
      visit() {
        return { type: "root", children: [] } satisfies HastNode;
      },
    },
  });
  expect(() =>
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined),
  ).toThrow(/cannot encode replacement content of type "root"/);
});

// ctx.parent()/indexOf(): same contract as the MDAST side (visitor.test.ts).

test("parent climbs from a nested element to the root", () => {
  const { handle, source } = setup("> quoted *text*\n");
  const chain: string[] = [];
  const plugin = defineHastPlugin({
    name: "climb-ancestors",
    element: {
      filter: ["em"],
      visit(node, ctx) {
        // Climbing reassigns from a possibly-root parent, so the loop var widens.
        let p: HastNode | undefined = ctx.parent(node);
        while (p) {
          chain.push(p.type === "element" ? p.tagName : p.type);
          p = ctx.parent(p);
        }
      },
    },
  });
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
  expect(chain).toEqual(["p", "blockquote", "root"]);
});

test("parent is canonical per id and usable as a structural anchor", () => {
  const { handle, source } = setup("alpha\n\nbeta\n");
  const parents = new Set<unknown>();
  const plugin = defineHastPlugin({
    name: "append-via-parent",
    element: {
      filter: ["p"],
      visit(node, ctx) {
        const parent = ctx.parent(node);
        if (parent === undefined || parents.has(parent)) return;
        parents.add(parent);
        ctx.appendChild(parent, {
          type: "element",
          tagName: "footer",
          properties: {},
          children: [{ type: "text", value: "once" }],
        });
      },
    },
  });
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
  const html = renderHandle(handle);
  expect(parents.size).toBe(1);
  expect(html.match(/<footer>once<\/footer>/g)).toHaveLength(1);
  expect(html.indexOf("<footer>")).toBeGreaterThan(html.indexOf("beta"));
});

test("indexOf counts the whitespace text nodes between blocks", () => {
  const { handle, source } = setup("alpha\n\nbeta\n");
  const indexes: (number | undefined)[] = [];
  const plugin = defineHastPlugin({
    name: "index-of",
    element: {
      filter: ["p"],
      visit(node, ctx) {
        indexes.push(ctx.indexOf(node));
        const root = ctx.parent(node)!;
        indexes.push(ctx.indexOf(root));
      },
    },
  });
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
  // Root children are [p, "\n", p]: the second paragraph sits at index 2.
  expect(indexes).toEqual([0, undefined, 2, undefined]);
});

test("parent and indexOf work on child stubs, not just visited nodes", () => {
  const { handle, source } = setup("hello\n");
  let stubParentTag: string | undefined;
  let stubIndex: number | undefined;
  const plugin = defineHastPlugin({
    name: "stub-parent",
    element: {
      filter: ["p"],
      visit(node, ctx) {
        const firstChild: HastNode = node.children[0]!;
        const parent = ctx.parent(firstChild);
        stubParentTag = parent?.type === "element" ? parent.tagName : parent?.type;
        stubIndex = ctx.indexOf(firstChild);
      },
    },
  });
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
  expect(stubParentTag).toBe("p");
  expect(stubIndex).toBe(0);
});

test("parent throws on plugin-built nodes (no arena id)", () => {
  const { handle, source } = setup("hello\n");
  let error: Error | undefined;
  const plugin = defineHastPlugin({
    name: "parent-of-new-node",
    element: {
      filter: ["p"],
      visit(_node, ctx) {
        try {
          ctx.parent({ type: "element", tagName: "div", properties: {}, children: [] });
        } catch (e) {
          error = e as Error;
        }
      },
    },
  });
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
  expect(error?.message).toMatch(/no arena id/);
});
