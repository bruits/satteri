import { test, expect } from "vitest";
import {
  createMdastHandle,
  createMdxMdastHandle,
  createHastHandle,
  createMdxHastHandle,
  getHandleSource,
  serializeHandle,
  applyCommandsToMdastHandle,
  dropHandle,
} from "../src/index.js";
import { visitMdastHandle, resolveMdastSubscriptions } from "../src/mdast/mdast-visitor.js";
import { visitHastHandle, resolveSubscriptions } from "../src/hast/hast-visitor.js";
import { MdastReader } from "../src/mdast/mdast-reader.js";
import { materializeMdastTree } from "../src/mdast/mdast-materializer.js";
import { HastReader } from "../src/hast/hast-reader.js";
import { materializeHastTree } from "../src/hast/hast-materializer.js";
import { markdownToHtml, defineMdastPlugin, defineHastPlugin } from "../src/index.js";
import type { MdastPluginDefinition } from "../src/plugin.js";
import type { Custom, MdastNode } from "../src/types.js";
import type { MdxJsxFlowElementHast } from "../src/mdx-types.js";
import type { Link, Paragraph, Strong, Text as MdastText } from "mdast";
import type { Element, ElementContent, Text as HastText } from "hast";
import type { Position } from "unist";
import { collect, type TreeNode } from "./fixtures.js";

const isEl = (n: TreeNode): n is Element => n.type === "element";

const isLink = (c: MdastNode): c is Link => c.type === "link";

const isStrong = (c: MdastNode): c is Strong => c.type === "strong";

const isMdastText = (c: MdastNode): c is MdastText => c.type === "text";

const isHastText = (c: ElementContent): c is HastText => c.type === "text";

// Phantom-space sentinel; mirrors the unexported PHANTOM_SPACE in src/phantom.ts.
const PHANTOM = "\uF002";

test.each(["mdast", "hast"] as const)(
  "%s reader preserves Unicode and node fields in a misaligned buffer slice",
  (kind) => {
    const source = "# Héllo 🌍\n\nA [link](https://example.com).";
    const handle = kind === "mdast" ? createMdastHandle(source) : createHastHandle(source);
    const Reader = kind === "mdast" ? MdastReader : HastReader;
    try {
      const bytes = serializeHandle(handle);
      const shifted = new Uint8Array(bytes.length + 1);
      shifted.set(bytes, 1);
      const reference = new Reader(bytes);
      const reader = new Reader(shifted.subarray(1));

      expect(reader.getStringPool()).toBe(reference.getStringPool());
      expect(reader.getStringPool()).toContain("Héllo 🌍");
      expect(reader.header).toEqual(reference.header);
      reader.header.nodeCount = 0;
      expect(reader.nodeCount).toBe(reference.nodeCount);
      for (let id = 0; id < reader.nodeCount; id++) {
        expect(reader.getNodeType(id)).toBe(reference.getNodeType(id));
        expect(reader.getPosition(id)).toEqual(reference.getPosition(id));
        expect(reader.getChildIds(id)).toEqual(reference.getChildIds(id));
        expect(reader.getNodeData(id)).toBe(reference.getNodeData(id));
      }
    } finally {
      dropHandle(handle);
    }
  },
);

type MdastNodeOf<T extends MdastNode["type"]> = Extract<MdastNode, { type: T }>;

function walkAndReader<T extends MdastNode["type"]>(md: string, type: T, mdx = false) {
  const handle = mdx ? createMdxMdastHandle(md) : createMdastHandle(md);
  const source = getHandleSource(handle);
  const walked: MdastNodeOf<T>[] = [];
  // Explicit type argument: the computed key infers as an index signature,
  // which the weak-type check would otherwise reject at the visit call sites.
  const plugin = defineMdastPlugin<MdastPluginDefinition>({
    name: "walk-reader-capture",
    [type](node: MdastNodeOf<T>) {
      walked.push(node);
    },
  });
  visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  const tree = materializeMdastTree(new MdastReader(serializeHandle(handle)));
  const materialized = collect(tree, (n): n is MdastNodeOf<T> => n.type === type);
  return { walked, materialized };
}

test("imageReference exposes `alt`/`referenceType` on the walk path, matching the reader", () => {
  const { walked, materialized } = walkAndReader(
    "![my alt][ref]\n\n[ref]: /img.png",
    "imageReference",
  );
  expect(walked).toHaveLength(1);
  expect(materialized).toHaveLength(1);
  expect(walked[0]!.alt).toBe("my alt");
  expect(walked[0]!.alt).toBe(materialized[0]!.alt);
  expect(walked[0]!.referenceType).toBe(materialized[0]!.referenceType);
  expect(walked[0]!.identifier).toBe(materialized[0]!.identifier);
  expect(walked[0]!.label).toBe(materialized[0]!.label);
});

test("MDX expression value strips phantom spaces on the walk path, matching the reader", () => {
  const { walked, materialized } = walkAndReader(
    "<div>\n\t{`a\n\tb`}\n</div>",
    "mdxFlowExpression",
    true,
  );
  expect(walked).toHaveLength(1);
  expect(materialized).toHaveLength(1);
  expect(walked[0]!.value).not.toContain(PHANTOM);
  expect(walked[0]!.value).toBe(materialized[0]!.value);
});

test("walk-path position matches the reader for every matched node", () => {
  // Lone-CR inputs check reader parity; line_index.rs separately verifies line-ending boundaries.
  const docs = [
    "# Heading\n\nA paragraph with **bold** and a [link](/x).",
    "# ❤️你好\n\n😀 A paragraph with **bold** and a [link](/x).",
    "# ❤️Heading\r\r😀 A paragraph with **bold** and a [link](/x).\rTail 你好.",
  ];
  for (const md of docs) {
    for (const type of ["heading", "paragraph", "text", "strong", "link"] as const) {
      const { walked, materialized } = walkAndReader(md, type);
      expect(walked.length).toBe(materialized.length);
      expect(walked.length).toBeGreaterThan(0);
      for (let i = 0; i < walked.length; i++) {
        expect(walked[i]!.position).toEqual(materialized[i]!.position);
      }
    }
  }
});

test("GFM autolink positions match the reader, present or absent", () => {
  const docs = ["[[x]](https://x.y)\n\n[x]: /", "a [b(https://x.y), c"];
  for (const md of docs) {
    for (const type of ["link", "text"] as const) {
      const { walked, materialized } = walkAndReader(md, type);
      expect(walked.length).toBe(materialized.length);
      expect(walked.length).toBeGreaterThan(0);
      for (let i = 0; i < walked.length; i++) {
        expect(walked[i]!.position).toEqual(materialized[i]!.position);
      }
    }
  }
});

test("ctx.replaceNode preserves a passed-through child's identity (nested transforms, one pass)", () => {
  const variants = new Set(["note", "tip"]);
  const plugin = defineMdastPlugin({
    name: "aside-ctx",
    containerDirective(node, ctx) {
      if (!variants.has(node.name)) return;
      ctx.replaceNode(node, {
        type: "paragraph",
        data: { hName: "aside", hProperties: { "data-v": node.name } },
        // The hName trick deliberately re-parents directive flow children into
        // a paragraph (same cast as test/conformance/asides.test.ts).
        children: [...node.children] as Paragraph["children"],
      });
    },
  });
  const { html } = markdownToHtml("::::note\nouter\n\n:::tip\ninner\n:::\n::::", {
    features: { directive: true, gfm: false },
    mdastPlugins: [plugin],
  }) as { html: string };
  expect((html.match(/<aside/g) ?? []).length).toBe(2);
  expect(html).toContain('data-v="note"');
  expect(html).toContain('data-v="tip"');
});

const isJsxFlow = (n: TreeNode): n is MdxJsxFlowElementHast => n.type === "mdxJsxFlowElement";

function setJsxAttr(md: string, component: string, key: string, value: unknown) {
  const handle = createMdxHastHandle(md);
  const source = getHandleSource(handle);
  const plugin = defineHastPlugin({
    name: "set-jsx-attr",
    mdxJsxFlowElement: {
      filter: [component],
      visit(node, ctx) {
        ctx.setProperty(node, key, value);
      },
    },
  });
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
  const tree = materializeHastTree(new HastReader(serializeHandle(handle)));
  return collect(tree, isJsxFlow)[0]!;
}

test("setProperty adds a string JSX attribute and preserves existing ones + children", () => {
  const jsx = setJsxAttr("<Box foo='bar'>\n  hi\n</Box>", "Box", "id", "x");
  expect(jsx.attributes).toContainEqual({ type: "mdxJsxAttribute", name: "id", value: "x" });
  expect(jsx.attributes).toContainEqual({ type: "mdxJsxAttribute", name: "foo", value: "bar" });
  const texts = collect(jsx, (n): n is HastText => n.type === "text");
  expect(texts.some((t) => t.value.includes("hi"))).toBe(true);
});

test("setProperty updates an existing JSX attribute without duplicating it", () => {
  const jsx = setJsxAttr("<Box foo='bar' />", "Box", "foo", "baz");
  const foos = jsx.attributes.filter((a) => a.type === "mdxJsxAttribute" && a.name === "foo");
  expect(foos).toEqual([{ type: "mdxJsxAttribute", name: "foo", value: "baz" }]);
});

test("setProperty(true) yields a boolean JSX attribute (value null)", () => {
  const jsx = setJsxAttr("<Box />", "Box", "disabled", true);
  expect(jsx.attributes).toContainEqual({ type: "mdxJsxAttribute", name: "disabled", value: null });
});

test("setProperty replaces an expression-valued JSX attribute instead of duplicating it", () => {
  const jsx = setJsxAttr("<Box foo={1+1} />", "Box", "foo", "x");
  const foos = jsx.attributes.filter((a) => a.type === "mdxJsxAttribute" && a.name === "foo");
  expect(foos).toEqual([{ type: "mdxJsxAttribute", name: "foo", value: "x" }]);
});

test("setProperty over a spread re-appends the attribute after it, so the write wins", () => {
  const jsx = setJsxAttr('<Box foo="a" {...rest} />', "Box", "foo", "b");
  const kinds = jsx.attributes.map((a) => (a.type === "mdxJsxAttribute" ? a.name : "{...}"));
  expect(kinds).toEqual(["{...}", "foo"]);
  expect(jsx.attributes[1]).toMatchObject({ name: "foo", value: "b" });
});

test("setProperty space-joins array values (binary path)", () => {
  const jsx = setJsxAttr("<Box />", "Box", "className", ["a", "b"]);
  expect(jsx.attributes).toContainEqual({
    type: "mdxJsxAttribute",
    name: "className",
    value: "a b",
  });
});

test("setProperty after replaceNode (fold path) space-joins arrays the same way", () => {
  const handle = createMdxHastHandle("<Box />");
  const source = getHandleSource(handle);
  const plugin = defineHastPlugin({
    name: "replace-then-set",
    mdxJsxFlowElement: {
      filter: ["Box"],
      visit(node, ctx) {
        ctx.replaceNode(node, { ...node, attributes: [] });
        ctx.setProperty(node, "className", ["a", "b"]);
      },
    },
  });
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
  const tree = materializeHastTree(new HastReader(serializeHandle(handle)));
  const jsx = collect(tree, isJsxFlow)[0]!;
  expect(jsx.attributes).toContainEqual({
    type: "mdxJsxAttribute",
    name: "className",
    value: "a b",
  });
});

test("an array replaceNode clears the queued replacement so a later setProperty can't resurrect it", () => {
  const handle = createMdxHastHandle("<Box />");
  const source = getHandleSource(handle);
  const plugin = defineHastPlugin({
    name: "single-then-array-then-set",
    mdxJsxFlowElement: {
      filter: ["Box"],
      visit(node, ctx) {
        ctx.replaceNode(node, { ...node, name: "Stale" });
        ctx.replaceNode(node, [
          { type: "element", tagName: "a-el", properties: {}, children: [] },
          { type: "element", tagName: "b-el", properties: {}, children: [] },
        ]);
        ctx.setProperty(node, "id", "x");
      },
    },
  });
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
  const tree = materializeHastTree(new HastReader(serializeHandle(handle)));
  expect(collect(tree, isJsxFlow)).toHaveLength(0);
  const tags = collect(tree, isEl).map((e) => e.tagName);
  expect(tags).toContain("a-el");
  expect(tags).toContain("b-el");
});

test("mdast stub children read the same as reader-materialized children", () => {
  const handle = createMdastHandle('A paragraph with **bold** and a [link](/x "T").');
  const source = getHandleSource(handle);
  let stubs: Paragraph["children"] = [];
  const plugin = defineMdastPlugin({
    name: "capture-paragraph-children",
    paragraph(node) {
      stubs = node.children;
    },
  });
  visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  const tree = materializeMdastTree(new MdastReader(serializeHandle(handle)));
  const real = collect(tree, (n): n is Paragraph => n.type === "paragraph")[0]!.children;
  expect(stubs.length).toBe(real.length);
  expect(stubs.map((c) => c.type)).toEqual(real.map((c) => c.type));
  for (let i = 0; i < stubs.length; i++) {
    expect(stubs[i]!.position).toEqual(real[i]!.position);
  }
  expect(stubs.find(isLink)!.url).toBe(real.find(isLink)!.url);
  expect(stubs.find(isLink)!.title).toBe(real.find(isLink)!.title);
  expect(stubs.find(isMdastText)!.value).toBe(real.find(isMdastText)!.value);
  expect(stubs.find(isStrong)!.children).toEqual(real.find(isStrong)!.children);
});

test("hast stub children read the same as reader-materialized children", () => {
  const handle = createHastHandle("# Hi [link](/x)");
  const source = getHandleSource(handle);
  let stubs: ElementContent[] = [];
  const plugin = defineHastPlugin({
    name: "capture-h1-children",
    element: {
      filter: ["h1"],
      visit(node) {
        stubs = node.children;
      },
    },
  });
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
  const tree = materializeHastTree(new HastReader(serializeHandle(handle)));
  const h1 = collect(
    tree,
    (n): n is Element => n.type === "element" && (n as Element).tagName === "h1",
  )[0]!;
  const real = h1.children;
  expect(stubs.length).toBe(real.length);
  expect(stubs.map((c) => c.type)).toEqual(real.map((c) => c.type));
  const posOf = (n: object): Position | undefined => (n as { position?: Position }).position;
  for (let i = 0; i < stubs.length; i++) {
    expect(posOf(stubs[i]!)).toEqual(posOf(real[i]!));
  }
  const isAnchor = (c: ElementContent): c is Element =>
    c.type === "element" && (c as Element).tagName === "a";
  expect(stubs.find(isHastText)!.value).toBe(real.find(isHastText)!.value);
  expect(stubs.find(isAnchor)!.tagName).toBe(real.find(isAnchor)!.tagName);
  expect(stubs.find(isAnchor)!.properties).toEqual(real.find(isAnchor)!.properties);
  expect(stubs.find(isAnchor)!.children).toEqual(real.find(isAnchor)!.children);
});

test("a false-valued element property reads the same from walk and reader paths", () => {
  const handle = createHastHandle("- [ ] todo");
  const source = getHandleSource(handle);
  let walkChecked: unknown = "unset";
  const plugin = defineHastPlugin({
    name: "read-checked",
    element: {
      filter: ["input"],
      visit(node) {
        walkChecked = node.properties.checked;
      },
    },
  });
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
  const tree = materializeHastTree(new HastReader(serializeHandle(handle)));
  const input = collect(
    tree,
    (n): n is Element => n.type === "element" && (n as Element).tagName === "input",
  )[0]!;
  expect(input.properties.checked).toBe(false);
  expect(walkChecked).toBe(false);
});

test("empty parent exposes children on the walk path, matching the reader", () => {
  const { walked, materialized } = walkAndReader("<Component />", "mdxJsxFlowElement", true);
  expect(walked).toHaveLength(1);
  expect(materialized).toHaveLength(1);
  expect(walked[0]!.children).toEqual([]);
  expect(walked[0]!.children).toEqual(materialized[0]!.children);
});

test("leaf node omits children on the walk path, matching the reader", () => {
  const { walked, materialized } = walkAndReader("test", "text");
  expect(walked).toHaveLength(1);
  expect(materialized).toHaveLength(1);
  expect("children" in walked[0]!).toBe(false);
  expect("children" in materialized[0]!).toBe(false);
});

function customWalkAndReader(replacement: Custom) {
  const handle = createMdastHandle("> placeholder\n");
  const source = getHandleSource(handle);
  const create = defineMdastPlugin({
    name: "create-custom",
    paragraph(node, ctx) {
      ctx.replaceNode(node, replacement);
    },
  });
  const result = visitMdastHandle(
    handle,
    create,
    resolveMdastSubscriptions(create),
    source,
    undefined,
  ) as { commandBuffer: Uint8Array };
  applyCommandsToMdastHandle(handle, result.commandBuffer);

  let walked: Custom | undefined;
  let stub: TreeNode | undefined;
  const capture = defineMdastPlugin({
    name: "capture-custom",
    custom(node) {
      walked = node;
    },
    blockquote(node) {
      stub = node.children[0];
    },
  });
  visitMdastHandle(handle, capture, resolveMdastSubscriptions(capture), source, undefined);

  const tree = materializeMdastTree(new MdastReader(serializeHandle(handle)));
  const materialized = collect(tree, (n): n is TreeNode => n.type === replacement.type);
  return { walked, stub, materialized };
}

test("a custom leaf omits children on the walk, stub and reader paths alike", () => {
  const { walked, stub, materialized } = customWalkAndReader({ type: "kbd", value: "Ctrl" });
  expect(materialized).toHaveLength(1);
  expect(walked?.value).toBe("Ctrl");
  expect("children" in walked!).toBe(false);
  expect("children" in materialized[0]!).toBe(false);
  expect(stub!.children).toBeUndefined();
  expect("children" in stub!).toBe(false);
});

test("a custom parent keeps its children on the walk, stub and reader paths alike", () => {
  const { walked, stub, materialized } = customWalkAndReader({
    type: "section",
    children: [{ type: "text", value: "hi" }],
  });
  expect(materialized).toHaveLength(1);
  expect(walked?.children?.map((c) => c.type)).toEqual(["text"]);
  expect(materialized[0]!.children?.map((c) => c.type)).toEqual(["text"]);
  expect(stub!.children?.map((c) => c.type)).toEqual(["text"]);
});
