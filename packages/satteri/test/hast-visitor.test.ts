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
import type { HastNode } from "../src/hast/hast-materializer.js";
import type { HastVisitorContext } from "../src/hast/hast-visitor.js";
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
          } as unknown as HastNode);
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
          } as unknown as HastNode);
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<div><h1>Hello</h1></div>");
  });

  test("context.appendChild() adds a child to an element", () => {
    const { handle, source } = setup("# Hello");
    const plugin = {
      element: {
        filter: ["h1"],
        visit(node: HastNode, ctx: HastVisitorContext) {
          ctx.appendChild(node, { type: "text", value: "!" } as unknown as HastNode);
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
          } as unknown as HastNode);
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
          ctx.prependChild(node, { type: "text", value: ">> " } as unknown as HastNode);
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
          } as unknown as HastNode);
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
            ctx.insertAfter(textChild, { type: "text", value: " World" } as unknown as HastNode);
          }
        },
      },
    };
    const subs = resolveSubscriptions(plugin);
    visitHastHandle(handle, plugin, subs, source, undefined);
    const html = renderHandle(handle);
    expect(html).toContain("<h1>Hello World</h1>");
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
