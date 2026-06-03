// Regression guard: a node must read the same whether a plugin reaches it as a
// direct walk match (the Rust `walk.rs` inline path) or as a materialized child
// (the `*Reader` path). Divergences between these two paths were the root of a
// batch of "behaves differently depending on how you got the node" bugs — see
// C2 (phantom spaces), C3 (position), C4 (imageReference `alt`). These tests
// pin the two paths together. C1 and P1 cover the mutation-side fixes.

import { test, expect } from "vitest";
import {
  createMdastHandle,
  createMdxMdastHandle,
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
import { markdownToHtml, defineMdastPlugin } from "../src/index.js";

const PHANTOM = "\uF002";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyNode = any;

function collect(node: AnyNode, pred: (n: AnyNode) => boolean, out: AnyNode[] = []): AnyNode[] {
  if (pred(node)) out.push(node);
  const children = node.children;
  if (Array.isArray(children)) for (const c of children) collect(c, pred, out);
  return out;
}

/** Capture the nodes a walk visitor receives for `type`, plus the same-type
 *  nodes from the reader-materialized tree, in document order. */
function walkAndReader(md: string, type: string, mdx = false) {
  const handle = mdx ? createMdxMdastHandle(md) : createMdastHandle(md);
  const source = getHandleSource(handle);
  const walked: AnyNode[] = [];
  const plugin: AnyNode = {
    [type](node: AnyNode) {
      walked.push(node);
    },
  };
  visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  const tree = materializeMdastTree(new MdastReader(serializeHandle(handle)));
  const materialized = collect(tree, (n) => n.type === type);
  return { walked, materialized };
}

// C4 — imageReference `alt`

test("C4: imageReference exposes `alt`/`referenceType` on the walk path, matching the reader", () => {
  const { walked, materialized } = walkAndReader("![my alt][ref]\n\n[ref]: /img.png", "imageReference");
  expect(walked).toHaveLength(1);
  expect(materialized).toHaveLength(1);
  expect(walked[0].alt).toBe("my alt");
  expect(walked[0].alt).toBe(materialized[0].alt);
  expect(walked[0].referenceType).toBe(materialized[0].referenceType);
  expect(walked[0].identifier).toBe(materialized[0].identifier);
  expect(walked[0].label).toBe(materialized[0].label);
});

// C2 — phantom-space sentinels

test("C2: MDX expression value strips phantom spaces on the walk path, matching the reader", () => {
  // The tab on the continuation line is partially consumed by the dedent and
  // re-emitted as phantom-space sentinels; both paths must restore real spaces.
  const { walked, materialized } = walkAndReader("<div>\n\t{`a\n\tb`}\n</div>", "mdxFlowExpression", true);
  expect(walked).toHaveLength(1);
  expect(materialized).toHaveLength(1);
  expect(walked[0].value).not.toContain(PHANTOM);
  expect(walked[0].value).toBe(materialized[0].value);
});

// C3 — position parity (the synthesized → undefined branch is defensive; this
// pins the decode offsets and guards the walk path from fabricating positions)

test("C3: walk-path position matches the reader for every matched node", () => {
  const md = "# Heading\n\nA paragraph with **bold** and a [link](/x).";
  for (const type of ["heading", "paragraph", "text", "strong", "link"]) {
    const { walked, materialized } = walkAndReader(md, type);
    expect(walked.length).toBe(materialized.length);
    for (let i = 0; i < walked.length; i++) {
      expect(walked[i].position).toEqual(materialized[i].position);
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
        children: [...node.children],
      } as unknown as Parameters<typeof ctx.replaceNode>[1]);
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

function setJsxAttr(md: string, component: string, key: string, value: unknown) {
  const handle = createMdxHastHandle(md);
  const source = getHandleSource(handle);
  const plugin = {
    mdxJsxFlowElement: {
      filter: [component],
      visit(node: AnyNode, ctx: AnyNode) {
        ctx.setProperty(node, key, value);
      },
    },
  };
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
  const tree = materializeHastTree(new HastReader(serializeHandle(handle)));
  return collect(tree, (n) => n.type === "mdxJsxFlowElement")[0];
}

test("P1: setProperty adds a string JSX attribute and preserves existing ones + children", () => {
  const jsx = setJsxAttr("<Box foo='bar'>\n  hi\n</Box>", "Box", "id", "x");
  expect(jsx.attributes).toContainEqual({ type: "mdxJsxAttribute", name: "id", value: "x" });
  expect(jsx.attributes).toContainEqual({ type: "mdxJsxAttribute", name: "foo", value: "bar" });
  // Children survive the attribute write (the whole point of the binary path).
  expect(collect(jsx, (n) => n.type === "text").some((t) => t.value.includes("hi"))).toBe(true);
});

test("P1: setProperty updates an existing JSX attribute in place", () => {
  const jsx = setJsxAttr("<Box foo='bar' />", "Box", "foo", "baz");
  const foos = jsx.attributes.filter((a: AnyNode) => a.name === "foo");
  expect(foos).toEqual([{ type: "mdxJsxAttribute", name: "foo", value: "baz" }]);
});

test("P1: setProperty(true) yields a boolean JSX attribute (value null)", () => {
  const jsx = setJsxAttr("<Box />", "Box", "disabled", true);
  expect(jsx.attributes).toContainEqual({ type: "mdxJsxAttribute", name: "disabled", value: null });
});
