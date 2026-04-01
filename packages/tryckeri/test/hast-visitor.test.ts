import { describe, test, expect } from "vitest";
import { HastReader } from "../src/hast/hast-reader.js";
import { DataMap } from "../src/data-map.js";
import { visitHast } from "../src/hast/hast-visitor.js";
import { materializeHastTree } from "../src/hast/hast-materializer.js";
import {
  parseToHastBuffer,
  hastBufferToHtmlStr,
  applyMutations,
  parseMdxToHastBuffer,
} from "../index.js";
import type { HastNode } from "../src/hast/hast-materializer.js";
import type { HastVisitorContext } from "../src/hast/hast-visitor.js";

// Parse a simple markdown document to a HAST binary buffer for testing.
// "# Hello\n\nWorld" produces: root > [h1 > text("Hello"), p > text("World")]
function setup(source = "# Hello\n\nWorld") {
  const uint8 = parseToHastBuffer(source);
  const reader = new HastReader(uint8);
  const dataMap = new DataMap();
  return { reader, dataMap, uint8 };
}

/** Apply visitor mutations and return HTML */
function applyAndSerialize(uint8: Uint8Array, commandBuffer: Uint8Array): string {
  const newBuf = applyMutations(uint8, commandBuffer);
  return hastBufferToHtmlStr(newBuf);
}

// ---------------------------------------------------------------------------
// Basic visitor behaviour
// ---------------------------------------------------------------------------

describe("visitHast — basic behaviour", () => {
  test("visitor with no subscriptions produces no mutations, no diagnostics", () => {
    const { reader, dataMap } = setup();
    const result = visitHast(reader, {}, dataMap);
    expect(result.commandBuffer.length).toBe(0);
    expect(result.diagnostics.length).toBe(0);
    expect(result.hasMutations).toBe(false);
  });

  test("element() callback fires for each element node", () => {
    const { reader, dataMap } = setup();
    const tags: string[] = [];
    visitHast(
      reader,
      {
        element: {
          filter: [],
          visit(node: HastNode) {
            tags.push(node.type === "element" ? node.tagName : "?");
          },
        },
      },
      dataMap,
    );
    expect(tags).toContain("h1");
    expect(tags).toContain("p");
  });

  test("text() callback fires for each text node", () => {
    const { reader, dataMap } = setup();
    const texts: string[] = [];
    visitHast(
      reader,
      {
        text(node: HastNode) {
          texts.push(node.type === "text" ? node.value : "");
        },
      },
      dataMap,
    );
    expect(texts).toContain("Hello");
    expect(texts).toContain("World");
  });
});

// ---------------------------------------------------------------------------
// Lifecycle hooks
// ---------------------------------------------------------------------------

describe("visitHast — lifecycle hooks", () => {
  test("before() fires before visitor methods", () => {
    const { reader, dataMap } = setup();
    const order: string[] = [];
    visitHast(
      reader,
      {
        before() {
          order.push("before");
        },
        element: {
          filter: [],
          visit() {
            order.push("element");
          },
        },
      },
      dataMap,
    );
    expect(order[0]).toBe("before");
    expect(order).toContain("element");
  });

  test("after() fires after visitor methods", () => {
    const { reader, dataMap } = setup();
    const order: string[] = [];
    visitHast(
      reader,
      {
        element: {
          filter: [],
          visit() {
            order.push("element");
          },
        },
        after() {
          order.push("after");
        },
      },
      dataMap,
    );
    expect(order[order.length - 1]).toBe("after");
  });

  test("transformRoot() receives the full materialized root", () => {
    const { reader, dataMap } = setup();
    let rootType = "";
    visitHast(
      reader,
      {
        transformRoot(root: HastNode) {
          rootType = root.type;
        },
      },
      dataMap,
    );
    expect(rootType).toBe("root");
  });
});

// ---------------------------------------------------------------------------
// Mutations (end-to-end: visit → applyMutations → HTML)
// ---------------------------------------------------------------------------

describe("visitHast — mutations", () => {
  test("returning a node from element() creates a replace mutation", () => {
    const { reader, dataMap, uint8 } = setup();
    const result = visitHast(
      reader,
      {
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
      },
      dataMap,
    );
    expect(result.hasMutations).toBe(true);
    const html = applyAndSerialize(uint8, result.commandBuffer);
    expect(html).toContain("<h2>");
    expect(html).not.toContain("<h1>");
  });

  test("context.removeNode() removes a node", () => {
    const { reader, dataMap, uint8 } = setup();
    const result = visitHast(
      reader,
      {
        element: {
          filter: ["h1"],
          visit(node: HastNode, ctx: HastVisitorContext) {
            if (node.type === "element" && node.tagName === "h1") {
              ctx.removeNode(node);
            }
          },
        },
      },
      dataMap,
    );
    expect(result.hasMutations).toBe(true);
    const html = applyAndSerialize(uint8, result.commandBuffer);
    expect(html).not.toContain("<h1>");
    expect(html).toContain("World");
  });

  test("context.setProperty() modifies element attributes", () => {
    const { reader, dataMap, uint8 } = setup();
    const result = visitHast(
      reader,
      {
        element: {
          filter: ["h1"],
          visit(node: HastNode, ctx: HastVisitorContext) {
            if (node.type === "element" && node.tagName === "h1") {
              ctx.setProperty(node, "id", "title");
            }
          },
        },
      },
      dataMap,
    );
    expect(result.hasMutations).toBe(true);
    const html = applyAndSerialize(uint8, result.commandBuffer);
    expect(html).toContain('id="title"');
  });
});

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

describe("visitHast — diagnostics", () => {
  test("context.report() collects diagnostics", () => {
    const { reader, dataMap } = setup();
    const result = visitHast(
      reader,
      {
        element: {
          filter: ["h1"],
          visit(node: HastNode, ctx: HastVisitorContext) {
            if (node.type === "element" && node.tagName === "h1") {
              ctx.report({ message: "heading found", node, severity: "info" });
            }
          },
        },
      },
      dataMap,
    );
    expect(result.diagnostics.length).toBe(1);
    expect(result.diagnostics[0]!.message).toBe("heading found");
    expect(result.diagnostics[0]!.severity).toBe("info");
  });
});

// ---------------------------------------------------------------------------
// Materialize
// ---------------------------------------------------------------------------

describe("materializeHastTree", () => {
  test("materializes a tree from a HAST buffer", () => {
    const { reader, dataMap } = setup();
    const tree = materializeHastTree(reader, dataMap);
    expect(tree.type).toBe("root");
    if (tree.type !== "root") throw new Error("expected root");
    expect(tree.children).toBeDefined();
    expect(tree.children.length).toBeGreaterThan(0);
  });

  test("element nodes have tagName and properties", () => {
    const { reader, dataMap } = setup();
    const tree = materializeHastTree(reader, dataMap);
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
    const { reader, dataMap } = setup();
    const tree = materializeHastTree(reader, dataMap);
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

// ---------------------------------------------------------------------------
// MDX JSX attributes on HAST nodes
// ---------------------------------------------------------------------------

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
    const buf = parseMdxToHastBuffer("<Component />\n");
    const reader = new HastReader(buf);
    const tree = materializeHastTree(reader, new DataMap());
    const jsx = findHastNode(tree, "mdxJsxFlowElement");
    expect(jsx).not.toBeNull();
    if (!jsx || jsx.type !== "mdxJsxFlowElement") throw new Error("expected mdxJsxFlowElement");
    expect(jsx.name).toBe("Component");
    expect(jsx.attributes).toEqual([]);
  });

  test("element with string literal attribute", () => {
    const buf = parseMdxToHastBuffer('<Component foo="bar" />\n');
    const reader = new HastReader(buf);
    const tree = materializeHastTree(reader, new DataMap());
    const jsx = findHastNode(tree, "mdxJsxFlowElement");
    if (jsx?.type !== "mdxJsxFlowElement") throw new Error("expected mdxJsxFlowElement");
    expect(jsx.name).toBe("Component");
    expect(jsx.attributes).toEqual([{ type: "mdxJsxAttribute", name: "foo", value: "bar" }]);
  });

  test("element with boolean attribute", () => {
    const buf = parseMdxToHastBuffer("<Component disabled />\n");
    const reader = new HastReader(buf);
    const tree = materializeHastTree(reader, new DataMap());
    const jsx = findHastNode(tree, "mdxJsxFlowElement");
    if (jsx?.type !== "mdxJsxFlowElement") throw new Error("expected mdxJsxFlowElement");
    expect(jsx.attributes).toEqual([{ type: "mdxJsxAttribute", name: "disabled", value: null }]);
  });

  test("element with expression attribute", () => {
    const buf = parseMdxToHastBuffer("<Component count={42} />\n");
    const reader = new HastReader(buf);
    const tree = materializeHastTree(reader, new DataMap());
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
    const buf = parseMdxToHastBuffer("<Component {...props} />\n");
    const reader = new HastReader(buf);
    const tree = materializeHastTree(reader, new DataMap());
    const jsx = findHastNode(tree, "mdxJsxFlowElement");
    if (jsx?.type !== "mdxJsxFlowElement") throw new Error("expected mdxJsxFlowElement");
    expect(jsx.attributes).toEqual([{ type: "mdxJsxExpressionAttribute", value: "...props" }]);
  });

  test("element with multiple mixed attributes", () => {
    const buf = parseMdxToHastBuffer('<Component a="1" b={2} c {...d} />\n');
    const reader = new HastReader(buf);
    const tree = materializeHastTree(reader, new DataMap());
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
