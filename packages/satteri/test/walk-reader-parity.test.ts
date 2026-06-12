// Regression guard: a node must read the same whether a plugin reaches it as a
// direct walk match (the Rust `walk.rs` inline path) or as a materialized child
// (the `*Reader` path). Divergences between these two paths were the root of a
// batch of "behaves differently depending on how you got the node" bugs — see
// C2 (phantom spaces), C3 (position), C4 (imageReference `alt`). These tests
// pin the two paths together. C1, P1, and P2 cover the mutation-side fixes.

import { test, expect } from "vitest";
import {
  createMdastHandle,
  createMdxMdastHandle,
  createHastHandle,
  createMdxHastHandle,
  getHandleSource,
  serializeHandle,
} from "../index.js";
import { visitMdastHandle, resolveMdastSubscriptions } from "../src/mdast/mdast-visitor.js";
import { visitHastHandle, resolveSubscriptions } from "../src/hast/hast-visitor.js";
import { MdastReader } from "../src/mdast/mdast-reader.js";
import { materializeMdastTree } from "../src/mdast/mdast-materializer.js";
import { HastReader } from "../src/hast/hast-reader.js";
import { materializeHastTree } from "../src/hast/hast-materializer.js";
import { markdownToHtml, defineMdastPlugin, defineHastPlugin } from "../src/index.js";
import type { MdastPluginDefinition } from "../src/plugin.js";
import type { MdastNode } from "../src/types.js";
import type { MdxJsxFlowElementHast } from "../src/mdx-types.js";
import type { Link, Paragraph, Strong, Text as MdastText } from "mdast";
import type { Element, ElementContent, Text as HastText } from "hast";
import type { Position } from "unist";
import { collect, type TreeNode } from "./fixtures.js";

// Phantom-space sentinel; mirrors the unexported PHANTOM_SPACE in src/phantom.ts.
const PHANTOM = "\uF002";

type MdastNodeOf<T extends MdastNode["type"]> = Extract<MdastNode, { type: T }>;

/** Capture the nodes a walk visitor receives for `type`, plus the same-type
 *  nodes from the reader-materialized tree, in document order. */
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

// C4 — imageReference `alt`

test("C4: imageReference exposes `alt`/`referenceType` on the walk path, matching the reader", () => {
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

// C2 — phantom-space sentinels

test("C2: MDX expression value strips phantom spaces on the walk path, matching the reader", () => {
  // The tab on the continuation line is partially consumed by the dedent and
  // re-emitted as phantom-space sentinels; both paths must restore real spaces.
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

// C3 — position parity (the synthesized → undefined branch is defensive; this
// pins the decode offsets and guards the walk path from fabricating positions)

test("C3: walk-path position matches the reader for every matched node", () => {
  const md = "# Heading\n\nA paragraph with **bold** and a [link](/x).";
  for (const type of ["heading", "paragraph", "text", "strong", "link"] as const) {
    const { walked, materialized } = walkAndReader(md, type);
    expect(walked.length).toBe(materialized.length);
    for (let i = 0; i < walked.length; i++) {
      expect(walked[i]!.position).toEqual(materialized[i]!.position);
    }
  }
});

// C1 — mdast context structural methods preserve passed-through node identity

test("C1: ctx.replaceNode preserves a passed-through child's identity (nested transforms, one pass)", () => {
  const variants = new Set(["note", "tip"]);
  const plugin = defineMdastPlugin({
    name: "aside-ctx",
    containerDirective(node, ctx) {
      if (!variants.has(node.name)) return;
      // Replace via the context method (not the return value), passing children
      // through. The inner `:::tip` rides along and its own replacement, queued
      // the same pass, must still land — which requires the child to keep its
      // arena id (a `_ref` placeholder), exactly like the return-value path.
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

// P1 — HAST setProperty on MDX JSX elements (binary attribute upsert)

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

test("P1: setProperty adds a string JSX attribute and preserves existing ones + children", () => {
  const jsx = setJsxAttr("<Box foo='bar'>\n  hi\n</Box>", "Box", "id", "x");
  expect(jsx.attributes).toContainEqual({ type: "mdxJsxAttribute", name: "id", value: "x" });
  expect(jsx.attributes).toContainEqual({ type: "mdxJsxAttribute", name: "foo", value: "bar" });
  // Children survive the attribute write (the whole point of the binary path).
  const texts = collect(jsx, (n): n is HastText => n.type === "text");
  expect(texts.some((t) => t.value.includes("hi"))).toBe(true);
});

test("P1: setProperty updates an existing JSX attribute without duplicating it", () => {
  const jsx = setJsxAttr("<Box foo='bar' />", "Box", "foo", "baz");
  const foos = jsx.attributes.filter((a) => a.type === "mdxJsxAttribute" && a.name === "foo");
  expect(foos).toEqual([{ type: "mdxJsxAttribute", name: "foo", value: "baz" }]);
});

test("P1: setProperty(true) yields a boolean JSX attribute (value null)", () => {
  const jsx = setJsxAttr("<Box />", "Box", "disabled", true);
  expect(jsx.attributes).toContainEqual({ type: "mdxJsxAttribute", name: "disabled", value: null });
});

test("P1: setProperty replaces an expression-valued JSX attribute instead of duplicating it", () => {
  const jsx = setJsxAttr("<Box foo={1+1} />", "Box", "foo", "x");
  const foos = jsx.attributes.filter((a) => a.type === "mdxJsxAttribute" && a.name === "foo");
  expect(foos).toEqual([{ type: "mdxJsxAttribute", name: "foo", value: "x" }]);
});

test("P1: setProperty over a spread re-appends the attribute after it, so the write wins", () => {
  const jsx = setJsxAttr('<Box foo="a" {...rest} />', "Box", "foo", "b");
  const kinds = jsx.attributes.map((a) => (a.type === "mdxJsxAttribute" ? a.name : "{...}"));
  expect(kinds).toEqual(["{...}", "foo"]);
  expect(jsx.attributes[1]).toMatchObject({ name: "foo", value: "b" });
});

test("P1: setProperty space-joins array values (binary path)", () => {
  const jsx = setJsxAttr("<Box />", "Box", "className", ["a", "b"]);
  expect(jsx.attributes).toContainEqual({
    type: "mdxJsxAttribute",
    name: "className",
    value: "a b",
  });
});

test("P1: setProperty after replaceNode (fold path) space-joins arrays the same way", () => {
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

// P2 — walk-vs-reader parity for element properties

// S1 — ref-stub children: a matched node's `.children` are id+type stubs whose
// lazily-forwarded fields must read exactly like the reader-materialized node.

test("S1: mdast stub children read the same as reader-materialized children", () => {
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
  // `type` is eager on stubs; the rest forwards through the snapshot.
  expect(stubs.map((c) => c.type)).toEqual(real.map((c) => c.type));
  for (let i = 0; i < stubs.length; i++) {
    expect(stubs[i]!.position).toEqual(real[i]!.position);
  }
  const isLink = (c: MdastNode): c is Link => c.type === "link";
  const isStrong = (c: MdastNode): c is Strong => c.type === "strong";
  const isText = (c: MdastNode): c is MdastText => c.type === "text";
  expect(stubs.find(isLink)!.url).toBe(real.find(isLink)!.url);
  expect(stubs.find(isLink)!.title).toBe(real.find(isLink)!.title);
  expect(stubs.find(isText)!.value).toBe(real.find(isText)!.value);
  // A stub's `children` forwards to the reader path (real materialized nodes).
  expect(stubs.find(isStrong)!.children).toEqual(real.find(isStrong)!.children);
});

test("S1: hast stub children read the same as reader-materialized children", () => {
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
  const isText = (c: ElementContent): c is HastText => c.type === "text";
  expect(stubs.find(isText)!.value).toBe(real.find(isText)!.value);
  expect(stubs.find(isAnchor)!.tagName).toBe(real.find(isAnchor)!.tagName);
  expect(stubs.find(isAnchor)!.properties).toEqual(real.find(isAnchor)!.properties);
  expect(stubs.find(isAnchor)!.children).toEqual(real.find(isAnchor)!.children);
});

test("P2: a false-valued element property reads the same from walk and reader paths", () => {
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
