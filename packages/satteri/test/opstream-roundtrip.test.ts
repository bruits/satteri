// Round-trip oracle for the structural op-stream — the correctness guard now
// that the op-stream is the only declarative encoding (the JSON/JsNode path is
// gone). Each case replaces the document's single paragraph (or `p` element)
// with a declarative tree, compiles it to the op-stream, applies it, then
// materializes the result and asserts it deep-equals the *input* tree. This
// pins the real contract end to end — what a plugin declares is what the arena
// reads back — rather than only that two encoders agreed. Synthesized
// replacement nodes carry no source position, so the readback matches the
// input literal exactly. The payload-type byte is asserted so a regression that
// silently failed to compile would surface as an error, not a vacuous pass.

import { test, expect } from "vitest";
import {
  createMdastHandle,
  createMdxMdastHandle,
  createHastHandle,
  createMdxHastHandle,
  getHandleSource,
  serializeHandle,
  applyCommandsToMdastHandle,
  type JsFeatures,
} from "../index.js";
import { visitMdastHandle, resolveMdastSubscriptions } from "../src/mdast/mdast-visitor.js";
import { visitHastHandle, resolveSubscriptions } from "../src/hast/hast-visitor.js";
import { MdastReader } from "../src/mdast/mdast-reader.js";
import { materializeMdastTree } from "../src/mdast/mdast-materializer.js";
import { HastReader } from "../src/hast/hast-reader.js";
import { materializeHastTree } from "../src/hast/hast-materializer.js";
import { defineMdastPlugin, defineHastPlugin } from "../src/plugin.js";
import { MDAST_CUSTOM_TYPES } from "../src/mdast/generated/node-types.js";
import { HAST_CUSTOM_TYPES } from "../src/hast/generated/node-types.js";
import type { MdxJsxFlowElement, MdxJsxFlowElementData } from "../src/mdx-types.js";
import type { MdastNode, HastNode } from "../src/types.js";

const CMD_REPLACE = 0x0b;
const PAYLOAD_OPSTREAM = 0x14;

interface MdastCaseOpts {
  mdx?: boolean;
  features?: JsFeatures;
}

/** Replace the doc's single paragraph with `replacement` through the op-stream,
 *  then materialize and return the replaced subtree (root's only child). */
function roundTripMdast(replacement: MdastNode, opts: MdastCaseOpts): MdastNode {
  const md = "Hello *world*.\n";
  const handle = opts.mdx ? createMdxMdastHandle(md) : createMdastHandle(md, opts.features);
  const plugin = defineMdastPlugin({
    name: "roundtrip",
    paragraph() {
      return replacement;
    },
  });
  const result = visitMdastHandle(
    handle,
    plugin,
    resolveMdastSubscriptions(plugin),
    getHandleSource(handle),
    undefined,
  ) as { commandBuffer: Uint8Array };
  expect(result.commandBuffer[0]).toBe(CMD_REPLACE);
  expect(result.commandBuffer[5]).toBe(PAYLOAD_OPSTREAM);
  applyCommandsToMdastHandle(handle, result.commandBuffer);
  const tree = materializeMdastTree(new MdastReader(serializeHandle(handle)));
  expect(tree.children).toHaveLength(1);
  return tree.children[0] as MdastNode;
}

function expectMdastRoundTrip(replacement: MdastNode, opts: MdastCaseOpts = {}): void {
  expect(roundTripMdast(replacement, opts)).toEqual(replacement);
}

test("mdast round-trip: heading (depth) with text child", () => {
  expectMdastRoundTrip({
    type: "heading",
    depth: 3,
    children: [{ type: "text", value: "replaced heading" }],
  } satisfies MdastNode);
});

test("mdast round-trip: link (url + title)", () => {
  expectMdastRoundTrip({
    type: "paragraph",
    children: [
      {
        type: "link",
        url: "https://example.com/a?b=c&d=e",
        title: "Example title",
        children: [{ type: "text", value: "a link" }],
      },
    ],
  } satisfies MdastNode);
});

test("mdast round-trip: code (lang + meta + value)", () => {
  expectMdastRoundTrip({
    type: "code",
    lang: "rust",
    meta: 'file="main.rs" showLineNumbers',
    value: "fn main() {\n    println!();\n}",
  } satisfies MdastNode);
});

test("mdast round-trip: list (ordered + start + spread) with checked listItems", () => {
  expectMdastRoundTrip({
    type: "list",
    ordered: true,
    start: 7,
    spread: true,
    children: [
      {
        type: "listItem",
        checked: true,
        spread: false,
        children: [{ type: "paragraph", children: [{ type: "text", value: "done" }] }],
      },
      {
        type: "listItem",
        checked: false,
        spread: true,
        children: [{ type: "paragraph", children: [{ type: "text", value: "todo" }] }],
      },
    ],
  } satisfies MdastNode);
});

test("mdast round-trip: table with align (including none)", () => {
  expectMdastRoundTrip({
    type: "table",
    align: ["left", "center", null, "right"],
    children: [
      {
        type: "tableRow",
        children: [
          { type: "tableCell", children: [{ type: "text", value: "a" }] },
          { type: "tableCell", children: [{ type: "text", value: "b" }] },
          { type: "tableCell", children: [{ type: "text", value: "c" }] },
          { type: "tableCell", children: [{ type: "text", value: "d" }] },
        ],
      },
    ],
  } satisfies MdastNode);
});

test("mdast round-trip: imageReference (alt + identifier + referenceType)", () => {
  expectMdastRoundTrip({
    type: "paragraph",
    children: [
      {
        type: "imageReference",
        alt: "an image",
        identifier: "img-1",
        label: "Img-1",
        referenceType: "full",
      },
    ],
  } satisfies MdastNode);
});

test("mdast round-trip: containerDirective with attributes", () => {
  expectMdastRoundTrip(
    {
      type: "containerDirective",
      name: "note",
      attributes: { class: "callout wide", id: "n1" },
      children: [{ type: "paragraph", children: [{ type: "text", value: "directive body" }] }],
    } satisfies MdastNode,
    { features: { directive: true } },
  );
});

// `_mdxExplicitJsx` is a private marker not declared on the Data interfaces;
// declare it locally so the node literal stays fully typed (no casts).
interface ExplicitJsxData extends MdxJsxFlowElementData {
  _mdxExplicitJsx: true;
}

test("mdast round-trip: mdxJsxFlowElement with literal/expression/spread attributes", () => {
  const explicitJsx: ExplicitJsxData = { _mdxExplicitJsx: true };
  expectMdastRoundTrip(
    {
      type: "mdxJsxFlowElement",
      name: "Callout",
      attributes: [
        { type: "mdxJsxAttribute", name: "title", value: "Hi" },
        { type: "mdxJsxAttribute", name: "bare", value: null },
        {
          type: "mdxJsxAttribute",
          name: "count",
          value: { type: "mdxJsxAttributeValueExpression", value: "1 + 2" },
        },
        { type: "mdxJsxExpressionAttribute", value: "...rest" },
      ],
      data: explicitJsx,
      children: [{ type: "paragraph", children: [{ type: "text", value: "inside" }] }],
    } satisfies MdxJsxFlowElement,
    { mdx: true },
  );
});

test("mdast round-trip: bare text with a value", () => {
  expectMdastRoundTrip({ type: "text", value: "plain text replacement" } satisfies MdastNode);
});

test("mdast round-trip: image (url + alt + title, multi-slot fixed layout)", () => {
  expectMdastRoundTrip({
    type: "paragraph",
    children: [{ type: "image", url: "/img/banner.png", alt: "a banner", title: "Banner title" }],
  } satisfies MdastNode);
});

test("mdast round-trip: definition (all four string slots)", () => {
  expectMdastRoundTrip({
    type: "definition",
    identifier: "def-1",
    label: "Def-1",
    url: "https://example.com/def",
    title: "Definition title",
  } satisfies MdastNode);
});

test("mdast round-trip: empty strings ride OP_STR", () => {
  expectMdastRoundTrip({
    type: "paragraph",
    children: [
      { type: "text", value: "" },
      { type: "inlineCode", value: "" },
      // A null title is skipped on encode and reads back null, so the
      // empty-url link round-trips exactly.
      { type: "link", url: "", title: null, children: [{ type: "text", value: "x" }] },
    ],
  } satisfies MdastNode);
});

test("mdast round-trip: depth and start at their stored maxima", () => {
  // Deliberately outside `Heading["depth"]`'s 1-6 literal union: the wire
  // stores a u8 and the 255 boundary is exactly what's pinned here.
  expectMdastRoundTrip(rawMdast({ type: "heading", depth: 255, children: [] }));
  expectMdastRoundTrip({
    type: "list",
    ordered: true,
    start: 4294967295,
    spread: false,
    children: [{ type: "listItem", checked: null, spread: false, children: [] }],
  } satisfies MdastNode);
});

test("mdast round-trip: non-ASCII strings ride encodeInto's bulk path", () => {
  expectMdastRoundTrip({
    type: "paragraph",
    children: [
      { type: "text", value: "emoji 🎉🚀 mixed with CJK 日本語のテキスト" },
      {
        type: "link",
        url: "https://example.com/路径/ページ?q=🎯",
        title: "タイトル 🌟 标题",
        children: [{ type: "text", value: "リンク 🔗" }],
      },
    ],
  } satisfies MdastNode);
});

test("mdast round-trip: a large tree grows the op-stream writer past its initial buffer", () => {
  const children = Array.from({ length: 64 }, (_, i) => ({
    type: "paragraph" as const,
    children: [
      {
        type: "text" as const,
        value: `paragraph body ${i} with enough text to push the op-stream well past its initial buffer`,
      },
    ],
  }));
  expectMdastRoundTrip({ type: "blockquote", children } satisfies MdastNode);
});

test("hast round-trip: element with properties and nested children", () => {
  const md = "Hello world.\n";
  const replacement = {
    type: "element",
    tagName: "section",
    properties: { className: ["a", "b"], id: "s1", hidden: true, tabIndex: 0 },
    children: [
      {
        type: "element",
        tagName: "span",
        properties: {},
        children: [{ type: "text", value: "inner" }],
      },
    ],
  } satisfies HastNode;

  const handle = createHastHandle(md);
  const plugin = defineHastPlugin({
    name: "roundtrip",
    element: {
      filter: ["p"],
      visit() {
        return replacement;
      },
    },
  });
  // The hast visitor applies its commands internally.
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), getHandleSource(handle), undefined);
  const tree = materializeHastTree(new HastReader(serializeHandle(handle)));
  expect(tree.children).toHaveLength(1);
  expect(tree.children[0]).toEqual(replacement);
});

test("hast round-trip: non-ASCII property and MDX JSX attribute values", () => {
  const el = {
    type: "element",
    tagName: "div",
    properties: { title: "日本語タイトル 🎌", id: "café" },
    children: [],
  } satisfies HastNode;
  expect(roundTripHast(el)).toEqual(el);

  const jsx = {
    type: "mdxJsxFlowElement",
    name: "Note",
    attributes: [{ type: "mdxJsxAttribute", name: "label", value: "値 🎯 ünïcode" }],
    children: [],
  } satisfies HastNode;
  expect(roundTripHast(jsx, true)).toEqual(jsx);
});

/** Replace the doc's single `p` element with `replacement` through the
 *  op-stream, then materialize and return the replaced subtree. */
function roundTripHast(replacement: HastNode, mdx = false): HastNode {
  const md = "Hello world.\n";
  const handle = mdx ? createMdxHastHandle(md) : createHastHandle(md);
  const plugin = defineHastPlugin({
    name: "roundtrip",
    element: {
      filter: ["p"],
      visit() {
        return replacement;
      },
    },
  });
  visitHastHandle(handle, plugin, resolveSubscriptions(plugin), getHandleSource(handle), undefined);
  const tree = materializeHastTree(new HastReader(serializeHandle(handle)));
  expect(tree.children).toHaveLength(1);
  return tree.children[0] as HastNode;
}

// Footgun backstop: every variable-length `custom` node type must survive an
// op-stream round-trip. A forgotten or drifted encode/decode arm yields empty
// type_data, so the type's distinguishing field goes missing and the matching
// case fails here. The coverage loop fails if a new custom type is added to the
// registry without a sample, forcing a test that would catch the same bug.

type Check = (n: Record<string, unknown>) => void;

/** Widen a node to its raw field map for the sample checks; the unions'
 *  interfaces have no index signature, so the direct cast is rejected. */
function fields(n: object): Record<string, unknown> {
  return n as Record<string, unknown>;
}

/** Build a deliberately out-of-spec node for wire-boundary fixtures (e.g. a
 *  depth past Heading's 1-6, or text directly under a listItem); the encoder
 *  must round-trip the bytes verbatim regardless of mdast validity. */
function rawMdast(node: object): MdastNode {
  return node as MdastNode;
}

const MDAST_CUSTOM_SAMPLES: Record<string, { node: MdastNode; opts: MdastCaseOpts; check: Check }> =
  {
    list: {
      // Deliberately type-loose: text directly under listItem is not valid
      // flow content, but the encoder must still round-trip it verbatim.
      node: rawMdast({
        type: "list",
        ordered: true,
        start: 2,
        spread: false,
        children: [{ type: "listItem", spread: false, children: [{ type: "text", value: "x" }] }],
      }),
      opts: {},
      check: (n) => expect(n.start).toBe(2),
    },
    listItem: {
      node: {
        type: "listItem",
        spread: true,
        checked: true,
        children: [{ type: "paragraph", children: [{ type: "text", value: "x" }] }],
      } satisfies MdastNode,
      opts: {},
      check: (n) => expect(n.checked).toBe(true),
    },
    table: {
      node: {
        type: "table",
        align: ["center"],
        children: [{ type: "tableRow", children: [{ type: "tableCell", children: [] }] }],
      } satisfies MdastNode,
      opts: {},
      check: (n) => expect(n.align).toEqual(["center"]),
    },
    containerDirective: {
      node: { type: "containerDirective", name: "note", attributes: { id: "x" }, children: [] },
      opts: { features: { directive: true } },
      check: (n) => expect((n.attributes as Record<string, string>).id).toBe("x"),
    },
    leafDirective: {
      node: { type: "leafDirective", name: "note", attributes: { id: "x" }, children: [] },
      opts: { features: { directive: true } },
      check: (n) => expect(n.name).toBe("note"),
    },
    textDirective: {
      node: { type: "textDirective", name: "note", attributes: { id: "x" }, children: [] },
      opts: { features: { directive: true } },
      check: (n) => expect(n.name).toBe("note"),
    },
    mdxJsxFlowElement: {
      node: {
        type: "mdxJsxFlowElement",
        name: "Foo",
        attributes: [{ type: "mdxJsxAttribute", name: "a", value: "b" }],
        children: [],
      } satisfies MdastNode,
      opts: { mdx: true },
      check: (n) => expect(n.name).toBe("Foo"),
    },
    mdxJsxTextElement: {
      node: {
        type: "mdxJsxTextElement",
        name: "Foo",
        attributes: [{ type: "mdxJsxAttribute", name: "a", value: "b" }],
        children: [],
      } satisfies MdastNode,
      opts: { mdx: true },
      check: (n) => expect(n.name).toBe("Foo"),
    },
  };

const HAST_CUSTOM_SAMPLES: Record<string, { node: HastNode; mdx: boolean; check: Check }> = {
  element: {
    node: { type: "element", tagName: "section", properties: { id: "x" }, children: [] },
    mdx: false,
    check: (n) => expect(n.tagName).toBe("section"),
  },
  mdxJsxFlowElement: {
    node: {
      type: "mdxJsxFlowElement",
      name: "Foo",
      attributes: [{ type: "mdxJsxAttribute", name: "a", value: "b" }],
      children: [],
    } satisfies HastNode,
    mdx: true,
    check: (n) => expect(n.name).toBe("Foo"),
  },
  mdxJsxTextElement: {
    node: {
      type: "mdxJsxTextElement",
      name: "Foo",
      attributes: [{ type: "mdxJsxAttribute", name: "a", value: "b" }],
      children: [],
    } satisfies HastNode,
    mdx: true,
    check: (n) => expect(n.name).toBe("Foo"),
  },
};

test("op-stream round-trip covers every MDAST custom node type", () => {
  for (const name of MDAST_CUSTOM_TYPES) {
    const sample = MDAST_CUSTOM_SAMPLES[name];
    expect(sample, `missing round-trip sample for MDAST custom type "${name}"`).toBeDefined();
    const result = fields(roundTripMdast(sample!.node, sample!.opts));
    expect(result.type).toBe(name);
    sample!.check(result);
  }
});

test("op-stream round-trip covers every HAST custom node type", () => {
  for (const name of HAST_CUSTOM_TYPES) {
    const sample = HAST_CUSTOM_SAMPLES[name];
    expect(sample, `missing round-trip sample for HAST custom type "${name}"`).toBeDefined();
    const result = fields(roundTripHast(sample!.node, sample!.mdx));
    expect(result.type).toBe(name);
    sample!.check(result);
  }
});
