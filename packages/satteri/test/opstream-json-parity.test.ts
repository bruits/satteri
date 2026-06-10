// Plugin tree mutations have two encodings that must stay interchangeable: the
// binary op-stream fast path (compileMdastToOpstream → replay) and the JSON
// fallback (CommandBuffer.replace → serde JsNode). op-stream.ts and
// mdast-visitor.ts pin them as producing *identical arenas*; these tests are
// that claim's regression guard. Each case replaces the same node in two
// handles built from the same source — once through the visitor return value
// (op-stream, asserted via the payload-type byte) and once through a
// hand-built CommandBuffer JSON command — then asserts the serialized arenas
// are byte-identical. Byte equality held for every covered shape (the replay
// drives the same arena encoders in the same tree order, so even string-pool
// layout matches); the semantic tree comparison runs first only to give a
// readable diff if the stronger byte pin ever breaks.

import { test, expect, vi } from "vitest";
import {
  createMdastHandle,
  createMdxMdastHandle,
  createHastHandle,
  getHandleSource,
  serializeHandle,
  applyCommandsToMdastHandle,
  applyCommandsToHandle,
  type JsFeatures,
} from "../index.js";
import { visitMdastHandle, resolveMdastSubscriptions } from "../src/mdast/mdast-visitor.js";
import { visitHastHandle, resolveSubscriptions } from "../src/hast/hast-visitor.js";
import { CommandBuffer } from "../src/command-buffer.js";
import { MdastReader } from "../src/mdast/mdast-reader.js";
import { materializeMdastTree } from "../src/mdast/mdast-materializer.js";
import { HastReader } from "../src/hast/hast-reader.js";
import { materializeHastTree } from "../src/hast/hast-materializer.js";
import { defineMdastPlugin, defineHastPlugin } from "../src/plugin.js";
import type { MdxJsxFlowElement, MdxJsxFlowElementData } from "../src/mdx-types.js";
import type { MdastNode, MdastNodeInternal, HastNode, HastNodeInternal } from "../src/types.js";
import { findByType } from "./fixtures.js";

// Wire constants, must match command-buffer.ts. The payload-type byte is the
// guard that each side really took its intended path — without it a silent
// fallback would make the comparison vacuous (same encoder on both sides).
const CMD_REPLACE = 0x0b;
const PAYLOAD_SERDE_JSON = 0x12;
const PAYLOAD_OPSTREAM = 0x14;

function expectIdenticalArenas(viaOpstream: Uint8Array, viaJson: Uint8Array): void {
  expect(viaOpstream).toEqual(viaJson);
}

// MDAST: each doc has exactly one paragraph, which both sides replace.

interface MdastCaseOpts {
  mdx?: boolean;
  features?: JsFeatures;
}

function makeMdastHandle(md: string, opts: MdastCaseOpts) {
  return opts.mdx ? createMdxMdastHandle(md) : createMdastHandle(md, opts.features);
}

/** Op-stream side: visitor return value → compileMdastToOpstream → replaceOpstream. */
function arenaViaOpstream(md: string, replacement: MdastNode, opts: MdastCaseOpts): Uint8Array {
  const handle = makeMdastHandle(md, opts);
  const plugin = defineMdastPlugin({
    name: "parity-opstream",
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
  return serializeHandle(handle);
}

/** JSON side: hand-built CommandBuffer.replace (serde JsNode payload). */
function arenaViaJson(md: string, replacement: MdastNode, opts: MdastCaseOpts): Uint8Array {
  const handle = makeMdastHandle(md, opts);
  const tree = materializeMdastTree(new MdastReader(serializeHandle(handle)));
  const target = findByType(tree, "paragraph");
  expect(target).toBeDefined();
  const buf = new CommandBuffer();
  buf.replace((target as MdastNodeInternal)._nodeId, replacement);
  const bytes = buf.getBuffer();
  expect(bytes[0]).toBe(CMD_REPLACE);
  expect(bytes[5]).toBe(PAYLOAD_SERDE_JSON);
  applyCommandsToMdastHandle(handle, bytes);
  return serializeHandle(handle);
}

function expectMdastParity(replacement: MdastNode, opts: MdastCaseOpts = {}): void {
  const md = "Hello *world*.\n";
  const viaOpstream = arenaViaOpstream(md, replacement, opts);
  const viaJson = arenaViaJson(md, replacement, opts);
  const treeA = materializeMdastTree(new MdastReader(viaOpstream));
  const treeB = materializeMdastTree(new MdastReader(viaJson));
  expect(treeA).toEqual(treeB);
  expectIdenticalArenas(viaOpstream, viaJson);
}

test("mdast: heading (depth) with text child", () => {
  expectMdastParity({
    type: "heading",
    depth: 3,
    children: [{ type: "text", value: "replaced heading" }],
  } satisfies MdastNode);
});

test("mdast: link (url + title) nested under a paragraph", () => {
  expectMdastParity({
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

test("mdast: code (lang + meta + value)", () => {
  expectMdastParity({
    type: "code",
    lang: "rust",
    meta: 'file="main.rs" showLineNumbers',
    value: "fn main() {\n    println!();\n}",
  } satisfies MdastNode);
});

test("mdast: list (ordered + start + spread) with checked listItems", () => {
  expectMdastParity({
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

test("mdast: table with align (including none)", () => {
  expectMdastParity({
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

test("mdast: imageReference (alt + identifier + referenceType)", () => {
  expectMdastParity({
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

test("mdast: containerDirective with attributes", () => {
  expectMdastParity(
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

test("mdast: mdxJsxFlowElement with literal/expression/spread attributes and _mdxExplicitJsx", () => {
  const explicitJsx: ExplicitJsxData = { _mdxExplicitJsx: true };
  expectMdastParity(
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

test("mdast: bare text with a value", () => {
  expectMdastParity({ type: "text", value: "plain text replacement" } satisfies MdastNode);
});

test("mdast: non-ASCII strings ride encodeInto's bulk path with a correct length backpatch", () => {
  expectMdastParity({
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

test("mdast: a large replacement tree grows the op-stream writer past its initial buffer", () => {
  // Well past OpWriter's 512-byte initial size (and any growth left over from
  // earlier tests reusing the module-level writer), forcing ByteWriter.ensure.
  const children = Array.from({ length: 64 }, (_, i) => ({
    type: "paragraph" as const,
    children: [
      {
        type: "text" as const,
        value: `paragraph body ${i} with enough text to push the op-stream well past its initial buffer`,
      },
    ],
  }));
  expectMdastParity({ type: "blockquote", children } satisfies MdastNode);
});

// HAST: visitor return value (op-stream) vs hand-built replaceRawJson carrying
// the same markHast-shaped JSON the visitor's fallback emits (`_hast: true` on
// every node, only the fields the node actually has).

test("hast: element replacement with properties and nested children", () => {
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

  const handleA = createHastHandle(md);
  const plugin = defineHastPlugin({
    name: "parity-opstream",
    element: {
      filter: ["p"],
      visit() {
        return replacement;
      },
    },
  });
  // The hast visitor applies its commands internally, so the payload-type byte
  // is out of reach; spy on the CommandBuffer methods instead to guard that
  // this side really took the op-stream path (no silent JSON fallback).
  const opstreamSpy = vi.spyOn(CommandBuffer.prototype, "replaceOpstream");
  const jsonFallbackSpy = vi.spyOn(CommandBuffer.prototype, "replaceRawJson");
  visitHastHandle(
    handleA,
    plugin,
    resolveSubscriptions(plugin),
    getHandleSource(handleA),
    undefined,
  );
  expect(opstreamSpy).toHaveBeenCalledTimes(1);
  expect(jsonFallbackSpy).not.toHaveBeenCalled();
  opstreamSpy.mockRestore();
  jsonFallbackSpy.mockRestore();
  const viaOpstream = serializeHandle(handleA);

  const handleB = createHastHandle(md);
  const tree = materializeHastTree(new HastReader(serializeHandle(handleB)));
  const target = findByType(tree, "element");
  expect(target).toBeDefined();
  const buf = new CommandBuffer();
  buf.replaceRawJson(
    (target as HastNodeInternal)._nodeId,
    JSON.stringify({
      _hast: true,
      type: "element",
      tagName: "section",
      properties: replacement.properties,
      children: [
        {
          _hast: true,
          type: "element",
          tagName: "span",
          properties: {},
          children: [{ _hast: true, type: "text", value: "inner" }],
        },
      ],
    }),
  );
  applyCommandsToHandle(handleB, buf.getBuffer());
  const viaJson = serializeHandle(handleB);

  const treeA = materializeHastTree(new HastReader(viaOpstream));
  const treeB = materializeHastTree(new HastReader(viaJson));
  expect(treeA).toEqual(treeB);
  expectIdenticalArenas(viaOpstream, viaJson);
});
