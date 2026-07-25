import { test, expect } from "vitest";
import { markdownToHtml } from "../src/index.js";
import { defineMdastPlugin } from "../src/plugin.js";
import type { MdastContent } from "../src/mdast/mdast-visitor.js";

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
        children: node.children,
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
        children: node.children,
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
        children: node.children,
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
        children: node.children,
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
      // Wrapping only adds a level: a later pass still reaches the descendants.
      seenStrong = true;
    },
  });

  markdownToHtml("Hello **bold**", { mdastPlugins: [create, inspect] });
  expect(seenType).toBe("section");
  expect(seenStrong).toBe(true);
});

test("GFM content survives inside a custom node (the #125 repro, fixed)", () => {
  // Replace a blockquote with a section wrapping its children — a GFM table
  // among them. A directive wrapper drops all of it without an `hName`.
  const wrap = defineMdastPlugin({
    name: "wrap-block",
    blockquote(node, ctx) {
      ctx.replaceNode(node, {
        type: "section",
        data: { hName: "section" },
        children: node.children,
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
        children: node.children,
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
        children: node.children,
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

test("children win over value when a custom node declares both", () => {
  const wrap = defineMdastPlugin({
    name: "both",
    paragraph(node, ctx) {
      ctx.replaceNode(node, { type: "weird", value: "IGNORED", children: node.children });
    },
  });
  const { html } = markdownToHtml("Hello **bold**", { mdastPlugins: [wrap] });
  expect(html).toContain("<div>Hello <strong>bold</strong></div>");
  expect(html).not.toContain("IGNORED");
});

test("a custom node with an empty value is a parent, not a text node", () => {
  const wrap = defineMdastPlugin({
    name: "empty",
    paragraph(node, ctx) {
      ctx.replaceNode(node, { type: "token", value: "" });
    },
  });
  const { html } = markdownToHtml("placeholder", { mdastPlugins: [wrap] });
  expect(html.trim()).toBe("<div></div>");
});

test("fields outside the node shape are dropped; data carries metadata", () => {
  const create = defineMdastPlugin({
    name: "create",
    paragraph(node, ctx) {
      ctx.replaceNode(node, {
        type: "section",
        depth: 2,
        data: { hName: "section", depth: 2 },
        children: node.children,
      } as MdastContent);
    },
  });
  let seen: Record<string, unknown> = {};
  const inspect = defineMdastPlugin({
    name: "inspect",
    custom(node) {
      seen = { depth: (node as { depth?: number }).depth, data: node.data };
    },
  });
  markdownToHtml("hi", { mdastPlugins: [create, inspect] });
  expect(seen.depth).toBeUndefined();
  expect(seen.data).toEqual({ hName: "section", depth: 2 });
});

test("a custom node can be mutated from the custom visitor", () => {
  // The node handed to `custom` goes straight back into the mutation API,
  // without a cast — the type-level half of this test is the compile.
  const create = defineMdastPlugin({
    name: "create",
    paragraph(node, ctx) {
      ctx.replaceNode(node, { type: "sec", data: { hName: "sec" }, children: node.children });
    },
  });
  const mutate = defineMdastPlugin({
    name: "mutate",
    custom(node, ctx) {
      if (node.type !== "sec") return;
      ctx.setProperty(node, "data", { hName: "aside" });
      ctx.appendChild(node, { type: "text", value: "!" });
    },
  });
  const { html } = markdownToHtml("hey", { mdastPlugins: [create, mutate] });
  expect(html).toContain("<aside>hey!</aside>");
});

test("a custom node can wrap the node it was reached from", () => {
  const create = defineMdastPlugin({
    name: "create",
    paragraph(node, ctx) {
      ctx.replaceNode(node, { type: "sec", data: { hName: "sec" }, children: node.children });
    },
  });
  const rewrap = defineMdastPlugin({
    name: "rewrap",
    custom(node, ctx) {
      if (node.type !== "sec") return;
      ctx.replaceNode(node, { type: "outer", data: { hName: "outer" }, children: [node] });
    },
  });
  const { html } = markdownToHtml("Hello **b**", { mdastPlugins: [create, rewrap] });
  expect(html).toContain("<outer><sec>Hello <strong>b</strong></sec></outer>");
});

test("nested custom nodes render and nest", () => {
  const nest = defineMdastPlugin({
    name: "nest",
    paragraph(node, ctx) {
      ctx.replaceNode(node, {
        type: "outer",
        data: { hName: "outer" },
        children: [{ type: "inner", data: { hName: "inner" }, children: node.children }],
      });
    },
  });
  const { html } = markdownToHtml("Hello **bold**", { mdastPlugins: [nest] });
  expect(html).toContain("<outer><inner>Hello <strong>bold</strong></inner></outer>");
});

test("a custom child's type is readable from a parent's children", () => {
  const create = defineMdastPlugin({
    name: "create",
    paragraph(node, ctx) {
      ctx.replaceNode(node, { type: "sec", data: { hName: "sec" }, children: node.children });
    },
  });
  let childTypes: string[] = [];
  const inspect = defineMdastPlugin({
    name: "inspect",
    blockquote(node) {
      childTypes = node.children.map((child) => child.type);
    },
  });
  markdownToHtml("> hello\n", { mdastPlugins: [create, inspect] });
  expect(childTypes).toEqual(["sec"]);
});
