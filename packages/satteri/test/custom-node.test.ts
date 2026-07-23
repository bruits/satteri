import { test, expect } from "vitest";
import { markdownToHtml } from "../src/index.js";
import { defineMdastPlugin } from "../src/plugin.js";
import type { MdastNode } from "../src/types.js";

// Issue #125: user-defined mdast node types. A plugin creates a node with an
// arbitrary `type` string; it round-trips, renders via `data.hName` (default
// `<div>`), recurses its children, and is visible to other plugins.

test("custom node renders via data.hName with children recursed", () => {
  const wrap = defineMdastPlugin({
    name: "wrap",
    paragraph(node, ctx) {
      ctx.replaceNode(node, {
        type: "section",
        data: { hName: "section" },
        children: node.children as unknown as MdastNode[],
      });
    },
  });
  const { html } = markdownToHtml("Hello **bold** world", { mdastPlugins: [wrap] });
  expect(html).toContain("<section>");
  expect(html).toContain("<strong>bold</strong>");
  expect(html).toContain("</section>");
});

test("custom node with hProperties merges attributes", () => {
  const wrap = defineMdastPlugin({
    name: "wrap",
    paragraph(node, ctx) {
      ctx.replaceNode(node, {
        type: "section",
        data: { hName: "section", hProperties: { className: ["note"], id: "s1" } },
        children: node.children as unknown as MdastNode[],
      });
    },
  });
  const { html } = markdownToHtml("hi", { mdastPlugins: [wrap] });
  expect(html).toMatch(/<section[^>]*class="note"[^>]*>/);
  expect(html).toMatch(/<section[^>]*id="s1"[^>]*>/);
});

test("custom node without hName defaults to <div>", () => {
  const wrap = defineMdastPlugin({
    name: "wrap",
    paragraph(node, ctx) {
      ctx.replaceNode(node, {
        type: "callout",
        children: node.children as unknown as MdastNode[],
      });
    },
  });
  const { html } = markdownToHtml("Hello **bold**", { mdastPlugins: [wrap] });
  expect(html).toContain("<div>");
  expect(html).toContain("<strong>bold</strong>");
});

test("custom type round-trips as node.type and content stays visible to other plugins", () => {
  const create = defineMdastPlugin({
    name: "create",
    paragraph(node, ctx) {
      ctx.replaceNode(node, {
        type: "section",
        data: { hName: "section" },
        children: node.children as unknown as MdastNode[],
      });
    },
  });

  let seenType: string | undefined;
  let seenStrong = false;
  const inspect = defineMdastPlugin({
    name: "inspect",
    custom(node) {
      seenType = node.type;
    },
    strong() {
      // A later pass still visits descendants of the custom node — content is
      // not skipped the way directive content is.
      seenStrong = true;
    },
  });

  markdownToHtml("Hello **bold**", { mdastPlugins: [create, inspect] });
  expect(seenType).toBe("section");
  expect(seenStrong).toBe(true);
});

test("GFM content survives inside a custom node (the #125 repro, fixed)", () => {
  // Replace a blockquote with a section wrapping its children — a GFM table
  // among them. In the directive world this content would be dropped.
  const wrap = defineMdastPlugin({
    name: "wrap-block",
    blockquote(node, ctx) {
      ctx.replaceNode(node, {
        type: "section",
        data: { hName: "section" },
        children: node.children as unknown as MdastNode[],
      });
    },
  });
  const md = "> | a | b |\n> | - | - |\n> | 1 | 2 |\n";
  const { html } = markdownToHtml(md, { features: { gfm: true }, mdastPlugins: [wrap] });
  expect(html).toContain("<section>");
  expect(html).toContain("<table>");
  expect(html).toContain("<td>1</td>");
});

test("custom leaf node (value, no children) renders as an escaped text node", () => {
  const wrap = defineMdastPlugin({
    name: "leaf",
    paragraph(node, ctx) {
      // Replace the paragraph with a value-bearing leaf — no children, no hName.
      ctx.replaceNode(node, { type: "token", value: "a < b & c" });
    },
  });
  const { html } = markdownToHtml("placeholder", { mdastPlugins: [wrap] });
  // Rendered as text (not wrapped in an element) and HTML-escaped.
  expect(html).toContain("a &lt; b &amp; c");
  expect(html).not.toContain("<token>");
  expect(html).not.toContain("<div>");
});

test("custom leaf's value round-trips and is visible to the custom visitor", () => {
  const create = defineMdastPlugin({
    name: "create",
    paragraph(node, ctx) {
      ctx.replaceNode(node, { type: "token", value: "hi" });
    },
  });
  let seenType: string | undefined;
  let seenValue: unknown = "UNSET";
  let seenChildren: unknown = "UNSET";
  const inspect = defineMdastPlugin({
    name: "inspect",
    custom(node) {
      seenType = node.type;
      seenValue = (node as { value?: string }).value;
      seenChildren = (node as { children?: unknown }).children;
    },
  });
  markdownToHtml("placeholder", { mdastPlugins: [create, inspect] });
  expect(seenType).toBe("token");
  expect(seenValue).toBe("hi");
  expect(seenChildren).toBeUndefined();
});

test("custom parent node carries no spurious value field", () => {
  const create = defineMdastPlugin({
    name: "create",
    paragraph(node, ctx) {
      ctx.replaceNode(node, {
        type: "section",
        data: { hName: "section" },
        children: node.children as unknown as MdastNode[],
      });
    },
  });
  let hasValueKey = true;
  const inspect = defineMdastPlugin({
    name: "inspect",
    custom(node) {
      hasValueKey = "value" in node;
    },
  });
  markdownToHtml("hello", { mdastPlugins: [create, inspect] });
  expect(hasValueKey).toBe(false);
});

test("a node whose type is literally 'custom' round-trips its type", () => {
  // `"custom"` is the internal tag's own public name; a plugin may still pick
  // it as a user-defined type, and it must survive rather than emptying out.
  const create = defineMdastPlugin({
    name: "create",
    paragraph(node, ctx) {
      ctx.replaceNode(node, {
        type: "custom",
        data: { hName: "aside" },
        children: node.children as unknown as MdastNode[],
      });
    },
  });

  let seenType: string | undefined = "UNSET";
  const inspect = defineMdastPlugin({
    name: "inspect",
    custom(node) {
      seenType = node.type;
    },
  });

  const { html } = markdownToHtml("Hello **bold**", { mdastPlugins: [create, inspect] });
  expect(seenType).toBe("custom");
  expect(html).toContain("<aside>");
  expect(html).toContain("<strong>bold</strong>");
});
