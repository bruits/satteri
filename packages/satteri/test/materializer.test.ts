import { test, expect, describe } from "vitest";
import { MdastReader } from "../src/mdast/mdast-reader.js";
import { materializeMdastTree } from "../src/mdast/mdast-materializer.js";
import type { MdastNode, MdastNodeInternal } from "../src/types.js";
import { buildHelloWorldBuffer } from "./fixtures.js";
import { createMdxMdastHandle, serializeHandle } from "../index.js";

function setup() {
  const buf = buildHelloWorldBuffer();
  const reader = new MdastReader(buf);
  return { reader };
}

test('materializeMdastTree returns a root node with type === "root"', () => {
  const { reader } = setup();
  const root = materializeMdastTree(reader);
  expect(root.type).toBe("root");
});

test("root node children is a lazy getter initially", () => {
  const { reader } = setup();
  const root = materializeMdastTree(reader);
  const desc = Object.getOwnPropertyDescriptor(root, "children");
  expect(typeof desc?.get === "function").toBe(true);
  expect("value" in (desc ?? {})).toBe(false);
});

test("accessing root.children returns 2 children (heading, paragraph)", () => {
  const { reader } = setup();
  const root = materializeMdastTree(reader);
  if (root.type !== "root") throw new Error("expected root");
  const children = root.children;
  expect(children.length).toBe(2);
  expect(children[0]!.type).toBe("heading");
  expect(children[1]!.type).toBe("paragraph");
});

test("heading has depth === 1", () => {
  const { reader } = setup();
  const root = materializeMdastTree(reader);
  if (root.type !== "root") throw new Error("expected root");
  const heading = root.children[0]!;
  if (heading.type !== "heading") throw new Error("expected heading");
  expect(heading.depth).toBe(1);
});

test('text child of heading has value === "Hello"', () => {
  const { reader } = setup();
  const root = materializeMdastTree(reader);
  if (root.type !== "root") throw new Error("expected root");
  const heading = root.children[0]!;
  if (heading.type !== "heading") throw new Error("expected heading");
  const textNode = heading.children[0]!;
  expect(textNode.type).toBe("text");
  if (textNode.type === "text") expect(textNode.value).toBe("Hello");
});

test('text child of paragraph has value === "World"', () => {
  const { reader } = setup();
  const root = materializeMdastTree(reader);
  if (root.type !== "root") throw new Error("expected root");
  const para = root.children[1]!;
  if (para.type !== "paragraph") throw new Error("expected paragraph");
  const textNode = para.children[0]!;
  expect(textNode.type).toBe("text");
  if (textNode.type === "text") expect(textNode.value).toBe("World");
});

test("position data is correct: root.position.start.line === 1", () => {
  const { reader } = setup();
  const root = materializeMdastTree(reader);
  expect(root.position!.start.line).toBe(1);
});

test("_nodeId is non-enumerable", () => {
  const { reader } = setup();
  const root = materializeMdastTree(reader);
  expect(Object.keys(root)).not.toContain("_nodeId");
  expect((root as MdastNodeInternal)._nodeId).toBe(0);
});

test("data is undefined when no data is set", () => {
  const { reader } = setup();
  const root = materializeMdastTree(reader);
  expect(root.data).toBeUndefined();
});

test("children are lazily evaluated (getter replaced by plain array after access)", () => {
  const { reader } = setup();
  const root = materializeMdastTree(reader);
  if (root.type !== "root") throw new Error("expected root");

  const beforeDesc = Object.getOwnPropertyDescriptor(root, "children");
  expect(typeof beforeDesc?.get === "function").toBe(true);

  const children = root.children;
  expect(Array.isArray(children)).toBe(true);

  const afterDesc = Object.getOwnPropertyDescriptor(root, "children");
  expect("get" in (afterDesc ?? {})).toBe(false);
  expect("value" in (afterDesc ?? {})).toBe(true);
});

// MDX JSX attribute tests

function mdxSetup(source: string) {
  const buf = serializeHandle(createMdxMdastHandle(source)) as Uint8Array;
  const reader = new MdastReader(buf);
  return { reader, tree: materializeMdastTree(reader) };
}

function findNode(node: MdastNode, type: string): any {
  if (node.type === type) return node;
  if ("children" in node && node.children) {
    for (const child of node.children) {
      const found = findNode(child, type);
      if (found) return found;
    }
  }
  return null;
}

describe("MDX JSX attributes on MDAST nodes", () => {
  test("self-closing element with no attributes", () => {
    const { tree } = mdxSetup("<Component />\n");
    const jsx = findNode(tree, "mdxJsxFlowElement");
    expect(jsx).not.toBeNull();
    expect(jsx.name).toBe("Component");
    expect(jsx.attributes).toEqual([]);
  });

  test("element with string literal attribute", () => {
    const { tree } = mdxSetup('<Component foo="bar" />\n');
    const jsx = findNode(tree, "mdxJsxFlowElement");
    expect(jsx.name).toBe("Component");
    expect(jsx.attributes).toEqual([{ type: "mdxJsxAttribute", name: "foo", value: "bar" }]);
  });

  test("element with boolean attribute", () => {
    const { tree } = mdxSetup("<Component disabled />\n");
    const jsx = findNode(tree, "mdxJsxFlowElement");
    expect(jsx.attributes).toEqual([{ type: "mdxJsxAttribute", name: "disabled", value: null }]);
  });

  test("element with expression attribute", () => {
    const { tree } = mdxSetup("<Component count={42} />\n");
    const jsx = findNode(tree, "mdxJsxFlowElement");
    expect(jsx.attributes).toEqual([
      {
        type: "mdxJsxAttribute",
        name: "count",
        value: { type: "mdxJsxAttributeValueExpression", value: "42" },
      },
    ]);
  });

  test("element with spread attribute", () => {
    const { tree } = mdxSetup("<Component {...props} />\n");
    const jsx = findNode(tree, "mdxJsxFlowElement");
    expect(jsx.attributes).toEqual([{ type: "mdxJsxExpressionAttribute", value: "...props" }]);
  });

  test("element with multiple mixed attributes", () => {
    const { tree } = mdxSetup('<Component a="1" b={2} c {...d} />\n');
    const jsx = findNode(tree, "mdxJsxFlowElement");
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

  test("inline JSX text element with attributes", () => {
    const { tree } = mdxSetup('a <Comp x="y" /> b\n');
    const jsx = findNode(tree, "mdxJsxTextElement");
    expect(jsx).not.toBeNull();
    expect(jsx.name).toBe("Comp");
    expect(jsx.attributes).toEqual([{ type: "mdxJsxAttribute", name: "x", value: "y" }]);
  });

  test("fragment has null name and no attributes", () => {
    const { tree } = mdxSetup("a <>hello</> b\n");
    const jsx = findNode(tree, "mdxJsxTextElement");
    expect(jsx).not.toBeNull();
    expect(jsx.name).toBeNull();
    expect(jsx.attributes).toEqual([]);
  });
});
