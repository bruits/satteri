import { test, expect } from "vitest";
import {
  visitMdastHandle,
  resolveMdastSubscriptions,
  type MdastHandle,
  type MdastVisitorContext,
} from "../src/mdast/mdast-visitor.js";
import type { HastHandle } from "../src/hast/hast-visitor.js";
import {
  createMdastHandle,
  getHandleSource,
  applyCommandsAndConvertToHastHandle,
  renderHandle,
} from "../index.js";
import type { MdastNode } from "../src/types.js";
import type { Heading, Text } from "mdast";
import { defineMdastPlugin } from "../src/plugin.js";
import { markdownToHtml, applyCommandsToMdastHandle } from "../src/index.js";

/** Helper: run a visitor on markdown, apply mutations, convert to HAST, render HTML. */
function visitAndRender(
  md: string,
  plugin: Parameters<typeof resolveMdastSubscriptions>[0],
): string {
  const handle = createMdastHandle(md);
  const source = getHandleSource(handle);
  const subs = resolveMdastSubscriptions(plugin);
  const result = visitMdastHandle(handle, plugin, subs, source, undefined) as {
    commandBuffer: Uint8Array;
  };
  const hastHandle = applyCommandsAndConvertToHastHandle(handle, result.commandBuffer);
  return renderHandle(hastHandle);
}

function setup() {
  const handle = createMdastHandle("# Hello\n\nWorld");
  const source = getHandleSource(handle);
  return { handle, source };
}

test("visitor with no subscriptions produces no mutations, no diagnostics", () => {
  const { handle, source } = setup();
  const plugin = {};
  const subs = resolveMdastSubscriptions(plugin);
  const result = visitMdastHandle(handle, plugin, subs, source, undefined);
  expect((result as { commandBuffer: Uint8Array }).commandBuffer.length).toBe(0);
  expect((result as { hasMutations: boolean }).hasMutations).toBe(false);
});

test("visiting heading nodes - callback fires once for the test doc", () => {
  const { handle, source } = setup();
  let callCount = 0;
  const plugin = {
    heading(_node: MdastNode) {
      callCount++;
    },
  };
  const subs = resolveMdastSubscriptions(plugin);
  visitMdastHandle(handle, plugin, subs, source, undefined);
  expect(callCount).toBe(1);
});

test('visitor callback receives correct MDAST node (type="heading", depth=1)', () => {
  const { handle, source } = setup();
  let capturedNode: MdastNode | null = null;
  const plugin = {
    heading(node: MdastNode) {
      capturedNode = node;
    },
  };
  const subs = resolveMdastSubscriptions(plugin);
  visitMdastHandle(handle, plugin, subs, source, undefined);
  expect(capturedNode).not.toBeNull();
  expect(capturedNode!.type).toBe("heading");
  if (capturedNode!.type === "heading") {
    expect((capturedNode! as { depth: number }).depth).toBe(1);
  }
});

test("return value from visitor creates a Replace command in the buffer", () => {
  const { handle, source } = setup();
  const newNode = { type: "paragraph", children: [] } satisfies MdastNode;
  const plugin = defineMdastPlugin({
    name: "replace-heading-via-return",
    heading(_node) {
      return newNode;
    },
  });
  const subs = resolveMdastSubscriptions(plugin);
  const result = visitMdastHandle(handle, plugin, subs, source, undefined) as {
    commandBuffer: Uint8Array;
    hasMutations: boolean;
  };
  expect(result.commandBuffer.length).toBeGreaterThan(0);
  expect(result.commandBuffer[0]).toBe(0x0b); // CMD_REPLACE
  expect(result.hasMutations).toBe(true);
});

test("context.removeNode creates a Remove command in the buffer", () => {
  const { handle, source } = setup();
  const plugin = {
    heading(node: MdastNode, context: MdastVisitorContext) {
      context.removeNode(node);
    },
  };
  const subs = resolveMdastSubscriptions(plugin);
  const result = visitMdastHandle(handle, plugin, subs, source, undefined) as {
    commandBuffer: Uint8Array;
    hasMutations: boolean;
  };
  expect(result.commandBuffer.length).toBe(5); // 1 byte cmd + 4 bytes nodeId
  expect(result.commandBuffer[0]).toBe(0x01); // CMD_REMOVE
  expect(result.hasMutations).toBe(true);
});

test("context.report creates a diagnostic entry", () => {
  const { handle, source } = setup();
  const plugin = {
    heading(node: MdastNode, context: MdastVisitorContext) {
      context.report({ message: "test diagnostic", node, severity: "warning" });
    },
  };
  const subs = resolveMdastSubscriptions(plugin);
  const result = visitMdastHandle(handle, plugin, subs, source, undefined) as {
    diagnostics: { message: string; severity: string; nodeId?: number }[];
  };
  expect(result.diagnostics.length).toBe(1);
  expect(result.diagnostics[0]!.message).toBe("test diagnostic");
  expect(result.diagnostics[0]!.severity).toBe("warning");
});

test("multiple subscribed types - all fire", () => {
  const { handle, source } = setup();
  const fired: string[] = [];
  const plugin = {
    heading(_node: MdastNode) {
      fired.push("heading");
    },
    text(_node: MdastNode) {
      fired.push("text");
    },
    paragraph(_node: MdastNode) {
      fired.push("paragraph");
    },
  };
  const subs = resolveMdastSubscriptions(plugin);
  visitMdastHandle(handle, plugin, subs, source, undefined);
  expect(fired).toContain("heading");
  expect(fired).toContain("paragraph");
  expect(fired.filter((x) => x === "text").length).toBe(2);
});

test("context.source returns the source text", () => {
  const { handle, source } = setup();
  let capturedSource: string | null = null;
  const plugin = {
    heading(_node: MdastNode, ctx: MdastVisitorContext) {
      capturedSource = ctx.source;
    },
  };
  const subs = resolveMdastSubscriptions(plugin);
  visitMdastHandle(handle, plugin, subs, source, undefined);
  expect(capturedSource).toBe("# Hello\n\nWorld");
});

test("context.textContent includes an inlineMath node's value", () => {
  // inlineMath shares Math's (meta, value) layout, so its value sits at the
  // second slot — mdast-util-to-string must read that, not the empty `meta`.
  const handle = createMdastHandle("# Energy $E=mc^2$", { math: true });
  const source = getHandleSource(handle);
  let captured: string | null = null;
  const plugin = defineMdastPlugin({
    name: "capture-heading-text",
    heading(node, ctx) {
      captured = ctx.textContent(node);
    },
  });
  const subs = resolveMdastSubscriptions(plugin);
  visitMdastHandle(handle, plugin, subs, source, undefined);
  expect(captured).toBe("Energy E=mc^2");
});

test("context.fileURL exposes a file URL passed as a URL", () => {
  const { handle, source } = setup();
  let captured: URL | undefined;
  const fileURL = new URL("file:///project/test.md");
  const plugin = {
    heading(_node: MdastNode, ctx: MdastVisitorContext) {
      captured = ctx.fileURL;
    },
  };
  const subs = resolveMdastSubscriptions(plugin);
  visitMdastHandle(handle, plugin, subs, source, fileURL);
  expect(captured).toBeInstanceOf(URL);
  expect(captured?.href).toBe("file:///project/test.md");
});

test("context.fileURL preserves a URL with a percent-encoded path", () => {
  const { handle, source } = setup();
  let captured: URL | undefined;
  const fileURL = new URL("file:///home/My Docs/test.md");
  const plugin = {
    heading(_node: MdastNode, ctx: MdastVisitorContext) {
      captured = ctx.fileURL;
    },
  };
  const subs = resolveMdastSubscriptions(plugin);
  visitMdastHandle(handle, plugin, subs, source, fileURL);
  // The URL keeps the space percent-encoded; `fileURLToPath` would decode it.
  expect(captured?.pathname).toBe("/home/My%20Docs/test.md");
});

test("hasMutations is false when no mutations, true when there are mutations", () => {
  const { handle, source } = setup();
  const noMutPlugin = { heading(_node: MdastNode) {} };
  const noMutSubs = resolveMdastSubscriptions(noMutPlugin);
  const noMutResult = visitMdastHandle(handle, noMutPlugin, noMutSubs, source, undefined) as {
    hasMutations: boolean;
  };
  expect(noMutResult.hasMutations).toBe(false);

  const handle2 = createMdastHandle("# Hello\n\nWorld");
  const mutPlugin = {
    heading(node: MdastNode, context: MdastVisitorContext) {
      context.removeNode(node);
    },
  };
  const mutSubs = resolveMdastSubscriptions(mutPlugin);
  const mutResult = visitMdastHandle(handle2, mutPlugin, mutSubs, source, undefined) as {
    hasMutations: boolean;
  };
  expect(mutResult.hasMutations).toBe(true);
});

test("setProperty + returning the same node does not drop the mutation", () => {
  const { handle, source } = setup();
  const plugin = defineMdastPlugin({
    name: "set-depth-keep-mutation",
    heading(node, context) {
      context.setProperty(node, "depth", 3);
      return node; // returning same object should NOT clobber the setProperty
    },
  });
  const subs = resolveMdastSubscriptions(plugin);
  const result = visitMdastHandle(handle, plugin, subs, source, undefined) as {
    hasMutations: boolean;
  };
  // The setProperty should still be present in the command buffer
  expect(result.hasMutations).toBe(true);
});

// End-to-end structural mutation tests

test("context.insertBefore() inserts a node before the target", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node: MdastNode, ctx: MdastVisitorContext) {
      ctx.insertBefore(node, { type: "thematicBreak" });
    },
  });
  expect(html).toContain("<hr>");
  expect(html).toContain("<h1>");
  expect(html.indexOf("<hr>")).toBeLessThan(html.indexOf("<h1>"));
});

test("context.insertAfter() inserts a node after the target", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node: MdastNode, ctx: MdastVisitorContext) {
      ctx.insertAfter(node, { type: "thematicBreak" });
    },
  });
  expect(html).toContain("<hr>");
  expect(html).toContain("<h1>");
  expect(html.indexOf("<h1>")).toBeLessThan(html.indexOf("<hr>"));
});

test("context.prependChild() adds a child at the start", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node: MdastNode, ctx: MdastVisitorContext) {
      ctx.prependChild(node, { type: "text", value: ">> " });
    },
  });
  expect(html).toContain("<h1>");
  expect(html).toMatch(/<h1>.*&gt;&gt;.*Hello/);
});

test("context.appendChild() adds a child at the end", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node: MdastNode, ctx: MdastVisitorContext) {
      ctx.appendChild(node, { type: "text", value: "!" });
    },
  });
  expect(html).toContain("<h1>");
  expect(html).toMatch(/<h1>Hello!<\/h1>/);
});

test("context.wrapNode() wraps a node in a parent", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node: MdastNode, ctx: MdastVisitorContext) {
      ctx.wrapNode(node, { type: "blockquote", children: [] });
    },
  });
  expect(html).toContain("<blockquote>");
  expect(html).toContain("<h1>");
  expect(html).toMatch(/<blockquote>.*<h1>/s);
});

test("context.replaceNode() replaces a node via context method", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node: MdastNode, ctx: MdastVisitorContext) {
      ctx.replaceNode(node, {
        type: "paragraph",
        children: [{ type: "text", value: "Replaced" }],
      });
    },
  });
  expect(html).not.toContain("<h1>");
  expect(html).toContain("Replaced");
});

test("context.setProperty(node, 'children', ...) replaces a node's children", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node, ctx) {
      ctx.setProperty(node, "children", [{ type: "text", value: "New heading" }]);
    },
  });
  expect(html).toMatch(/<h1>New heading<\/h1>/);
});

test("context.setProperty 'children' composes with a scalar setProperty on the same node", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node, ctx) {
      ctx.setProperty(node, "depth", 3);
      ctx.setProperty(node, "children", [{ type: "text", value: "New heading" }]);
    },
  });
  expect(html).toMatch(/<h3>New heading<\/h3>/);
});

test("context.setProperty(node, 'children', ...) keeps reused children", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node, ctx) {
      const original = node.children[0]!;
      ctx.setProperty(node, "children", [{ type: "text", value: "> " }, original]);
    },
  });
  expect(html).toMatch(/<h1>&gt; Hello<\/h1>/);
});

test("context.insertChildAt() prepends at index 0", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node, ctx) {
      ctx.insertChildAt(node, 0, { type: "text", value: ">> " });
    },
  });
  expect(html).toMatch(/<h1>&gt;&gt; Hello<\/h1>/);
});

test("context.insertChildAt() appends past the end", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node, ctx) {
      ctx.insertChildAt(node, 99, { type: "text", value: "!" });
    },
  });
  expect(html).toMatch(/<h1>Hello!<\/h1>/);
});

test("context.insertChildAt() inserts before the index-th child", () => {
  const html = visitAndRender("a *b*", {
    paragraph(node, ctx) {
      ctx.insertChildAt(node, 1, { type: "text", value: "Z" });
    },
  });
  expect(html).toMatch(/<p>a Z<em>b<\/em><\/p>/);
});

test("context.removeChildAt() removes the index-th child", () => {
  const html = visitAndRender("a *b*", {
    paragraph(node, ctx) {
      ctx.removeChildAt(node, 1);
    },
  });
  expect(html).toContain("<p>a ");
  expect(html).not.toContain("<em>");
});

test("context.appendChild() accepts an array of nodes, in order", () => {
  const html = visitAndRender("# Hello", {
    heading(node, ctx) {
      ctx.appendChild(node, [
        { type: "text", value: " A" },
        { type: "text", value: " B" },
      ]);
    },
  });
  expect(html).toMatch(/<h1>Hello A B<\/h1>/);
});

test("context.insertChildAt() accepts an array, keeping order at the index", () => {
  const html = visitAndRender("a *b*", {
    paragraph(node, ctx) {
      ctx.insertChildAt(node, 1, [
        { type: "text", value: "X" },
        { type: "text", value: "Y" },
      ]);
    },
  });
  expect(html).toMatch(/<p>a XY<em>b<\/em><\/p>/);
});

test("context.insertBefore() accepts an array of siblings", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node, ctx) {
      ctx.insertBefore(node, [{ type: "thematicBreak" }, { type: "thematicBreak" }]);
    },
  });
  expect((html.match(/<hr>/g) ?? []).length).toBe(2);
  expect(html.lastIndexOf("<hr>")).toBeLessThan(html.indexOf("<h1>"));
});

test("context.insertAfter() accepts an array of siblings", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node, ctx) {
      ctx.insertAfter(node, [{ type: "thematicBreak" }, { type: "thematicBreak" }]);
    },
  });
  expect((html.match(/<hr>/g) ?? []).length).toBe(2);
  expect(html.indexOf("<hr>")).toBeGreaterThan(html.indexOf("<h1>"));
});

test("context.prependChild() accepts an array of nodes, in order", () => {
  const html = visitAndRender("# Hello", {
    heading(node, ctx) {
      ctx.prependChild(node, [
        { type: "text", value: "A " },
        { type: "text", value: "B " },
      ]);
    },
  });
  expect(html).toMatch(/<h1>A B Hello<\/h1>/);
});

test("context.setProperty(node, 'children', []) clears the children", () => {
  const html = visitAndRender("# Hello\n\nWorld", {
    heading(node, ctx) {
      ctx.setProperty(node, "children", []);
    },
  });
  expect(html).toMatch(/<h1><\/h1>/);
});

test("context.removeChildAt() is a no-op for an out-of-range or negative index", () => {
  const html = visitAndRender("a *b*", {
    paragraph(node, ctx) {
      ctx.removeChildAt(node, 99);
      ctx.removeChildAt(node, -1);
    },
  });
  expect(html).toMatch(/<p>a <em>b<\/em><\/p>/);
});

test("context.insertChildAt() treats a negative index as a prepend", () => {
  const html = visitAndRender("# Hello", {
    heading(node, ctx) {
      ctx.insertChildAt(node, -5, { type: "text", value: ">> " });
    },
  });
  expect(html).toMatch(/<h1>&gt;&gt; Hello<\/h1>/);
});

test("setProperty on an invalid field throws an error naming the property and node type", () => {
  const run = () =>
    visitAndRender("# Hello\n\nWorld", {
      heading(node, ctx) {
        // @ts-expect-error "value" is not a field on a heading node
        ctx.setProperty(node, "value", "x");
      },
    });
  expect(run).toThrow(/cannot set property 'value' on a 'heading' node/);
});

// Directive visitors

function setupDirective(md: string) {
  const handle = createMdastHandle(md, { directive: true });
  const source = getHandleSource(handle);
  return { handle, source };
}

test("containerDirective visitor fires and exposes name + attributes", () => {
  const { handle, source } = setupDirective(":::tip{.note #id}\nbody\n:::\n");
  const seen: { name: string; attributes: Record<string, string> }[] = [];
  const plugin = defineMdastPlugin({
    name: "collect-container-directive",
    containerDirective(node) {
      seen.push({ name: node.name, attributes: { ...(node.attributes ?? {}) } });
    },
  });
  const subs = resolveMdastSubscriptions(plugin);
  expect(subs.length).toBe(1);
  visitMdastHandle(handle, plugin, subs, source, undefined);
  expect(seen.length).toBe(1);
  expect(seen[0]!.name).toBe("tip");
  expect(seen[0]!.attributes.id).toBe("id");
  expect(seen[0]!.attributes.class).toBe("note");
});

test("containerDirective with [label] exposes directiveLabel marker on first child", () => {
  const { handle, source } = setupDirective(":::warning[Heads up]\ncontent\n:::\n");
  let labelChildHadMarker = false;
  const plugin = defineMdastPlugin({
    name: "read-container-directive-label",
    containerDirective(node) {
      const first = node.children[0] as { data?: { directiveLabel?: boolean } } | undefined;
      labelChildHadMarker = first?.data?.directiveLabel === true;
    },
  });
  const subs = resolveMdastSubscriptions(plugin);
  visitMdastHandle(handle, plugin, subs, source, undefined);
  expect(labelChildHadMarker).toBe(true);
});

test("leafDirective visitor fires and exposes name", () => {
  const { handle, source } = setupDirective("::break{aria-label=section}\n");
  const seen: { name: string; attributes: Record<string, string> }[] = [];
  const plugin = defineMdastPlugin({
    name: "collect-leaf-directive",
    leafDirective(node) {
      seen.push({ name: node.name, attributes: { ...(node.attributes ?? {}) } });
    },
  });
  const subs = resolveMdastSubscriptions(plugin);
  visitMdastHandle(handle, plugin, subs, source, undefined);
  expect(seen.length).toBe(1);
  expect(seen[0]!.name).toBe("break");
  expect(seen[0]!.attributes["aria-label"]).toBe("section");
});

test("textDirective visitor fires inside a paragraph", () => {
  const { handle, source } = setupDirective("Hello :emoji[smile]{.big} world\n");
  const seen: string[] = [];
  const plugin = defineMdastPlugin({
    name: "collect-text-directive",
    textDirective(node) {
      seen.push(node.name);
    },
  });
  const subs = resolveMdastSubscriptions(plugin);
  visitMdastHandle(handle, plugin, subs, source, undefined);
  expect(seen).toEqual(["emoji"]);
});

test("containerDirective replaceNode rewrites to an aside-style block", () => {
  const handle = createMdastHandle(":::tip\nbody\n:::\n", { directive: true });
  const source = getHandleSource(handle);
  const plugin = defineMdastPlugin({
    name: "container-directive-to-aside",
    containerDirective(node, ctx) {
      ctx.replaceNode(node, { rawHtml: `<aside class="${node.name}">body</aside>` } as never);
    },
  });
  const subs = resolveMdastSubscriptions(plugin);
  const result = visitMdastHandle(handle, plugin, subs, source, undefined) as {
    commandBuffer: Uint8Array;
    hasMutations: boolean;
  };
  expect(result.hasMutations).toBe(true);
  const hastHandle = applyCommandsAndConvertToHastHandle(handle, result.commandBuffer);
  const html = renderHandle(hastHandle);
  expect(html).toContain('<aside class="tip">body</aside>');
});

test("returning a replacement with _keepChildren keeps the original children", () => {
  const heading = { type: "heading", depth: 2, children: [] } satisfies MdastNode;
  const replacement = { ...heading, _keepChildren: true };
  const plugin = defineMdastPlugin({
    name: "promote-heading-keep-children",
    heading() {
      return replacement;
    },
  });
  const html = visitAndRender("# Hello\n\nWorld", plugin);
  expect(html).toContain("<h2>Hello</h2>");
});

test("insertAfter ignores _keepChildren", () => {
  const heading = { type: "heading", depth: 3, children: [] } satisfies MdastNode;
  const inserted = { ...heading, _keepChildren: true };
  const plugin = defineMdastPlugin({
    name: "insert-after-keep-children",
    heading(node, ctx) {
      ctx.insertAfter(node, inserted);
    },
  });
  const html = visitAndRender("# Hello", plugin);
  expect(html).toContain("<h1>Hello</h1>");
  expect(html).toContain("<h3></h3>");
});

test("an out-of-range list start fails loudly instead of being silently masked", () => {
  const plugin = defineMdastPlugin({
    name: "bad-list-start",
    list(node) {
      return { ...node, start: -1 };
    },
  });
  expect(() => visitAndRender("1. one\n2. two", plugin)).toThrow(/out-of-range/);
});

test("a list start one past the u32 boundary fails loudly", () => {
  const plugin = defineMdastPlugin({
    name: "list-start-past-u32",
    list(node) {
      return { ...node, start: 4294967296 };
    },
  });
  expect(() => visitAndRender("1. one\n2. two", plugin)).toThrow(/out-of-range/);
});

test("a non-integer list start fails loudly", () => {
  const plugin = defineMdastPlugin({
    name: "fractional-list-start",
    list(node) {
      return { ...node, start: 1.5 };
    },
  });
  expect(() => visitAndRender("1. one\n2. two", plugin)).toThrow(/out-of-range/);
});

test("a heading depth past the u8 boundary fails loudly", () => {
  const plugin = defineMdastPlugin({
    name: "bad-heading-depth",
    heading(node) {
      // Deliberately outside Heading["depth"]'s 1-6 union: the wire boundary
      // (a stored u8) is what's pinned here.
      return { ...node, depth: 256 as Heading["depth"], children: [] };
    },
  });
  expect(() => visitAndRender("# Hello", plugin)).toThrow(/out-of-range/);
});

test("a bare root as replacement content fails loudly", () => {
  const plugin = defineMdastPlugin({
    name: "root-as-content",
    heading() {
      return { type: "root", children: [] } satisfies MdastNode;
    },
  });
  expect(() => visitAndRender("# Hello", plugin)).toThrow(/cannot encode replacement content/);
});

test("setProperty with an out-of-range number fails at apply instead of masking bits", () => {
  const plugin = defineMdastPlugin({
    name: "set-depth-out-of-range",
    heading(node, ctx) {
      // setProperty rides CMD_SET_PROPERTY, not the op-stream; Rust enforces
      // the slot range. 9999 is deliberately outside the depth union.
      ctx.setProperty(node, "depth", 9999 as Heading["depth"]);
    },
  });
  expect(() => visitAndRender("# Hello", plugin)).toThrow(/between 0 and 255/);
});

test("a replacement nested past the replay depth cap fails loudly", () => {
  let node: MdastNode = { type: "paragraph", children: [{ type: "text", value: "leaf" }] };
  for (let i = 0; i < 200; i++) node = { type: "blockquote", children: [node] };
  const deep = node;
  const plugin = defineMdastPlugin({
    name: "too-deep",
    paragraph() {
      return deep;
    },
  });
  expect(() => visitAndRender("Hello", plugin)).toThrow(/nests deeper/);
});

test("context mutations reject plugin-built nodes with no arena id", () => {
  const plugin = defineMdastPlugin({
    name: "remove-fresh-node",
    heading(_node, ctx) {
      ctx.removeNode({ type: "text", value: "x" });
    },
  });
  expect(() => visitAndRender("# Hello", plugin)).toThrow(/no arena id/);
});

test("setProperty(node, 'children', ...) rides the op-stream", () => {
  const { handle, source } = setup();
  const plugin = defineMdastPlugin({
    name: "swap-children-opstream",
    heading(node, ctx) {
      ctx.setProperty(node, "children", [{ type: "text", value: "swapped" }]);
    },
  });
  const subs = resolveMdastSubscriptions(plugin);
  const result = visitMdastHandle(handle, plugin, subs, source, undefined) as {
    commandBuffer: Uint8Array;
  };
  expect(result.commandBuffer[0]).toBe(0x0d); // CMD_SET_CHILDREN
  expect(result.commandBuffer[5]).toBe(0x14); // PAYLOAD_OPSTREAM
});

// Lazy-children lifecycle: matched nodes resolve `.children` from a snapshot
// taken during the pass; after the pass the arena may be rebuilt with new ids,
// so a first-time read must fail loudly instead of mapping stale ids.

test("async visitor reads `.children` in a deferred callback", async () => {
  const { handle, source } = setup();
  let firstChild: Heading["children"][number] | undefined;
  const plugin = defineMdastPlugin({
    name: "async-children-read",
    async heading(node) {
      await new Promise((r) => setTimeout(r, 1));
      firstChild = node.children[0];
    },
  });
  const result = visitMdastHandle(
    handle,
    plugin,
    resolveMdastSubscriptions(plugin),
    source,
    undefined,
  );
  expect(result).toBeInstanceOf(Promise);
  await result;
  expect(firstChild).toMatchObject({ type: "text", value: "Hello" });
});

test("a node retained past its visitor pass throws on its first `.children` read", () => {
  const { handle, source } = setup();
  let retained: Readonly<Heading> | undefined;
  const plugin = defineMdastPlugin({
    name: "retain-heading",
    heading(node) {
      retained = node;
    },
  });
  visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  expect(retained).toBeDefined();
  expect(() => retained!.children).toThrow(/retained past its visitor pass/);
});

test("a retained node throws on `.children` after an async pass settles", async () => {
  const { handle, source } = setup();
  let retained: Readonly<Heading> | undefined;
  const plugin = defineMdastPlugin({
    name: "retain-heading-async",
    async heading(node) {
      await new Promise((r) => setTimeout(r, 1));
      retained = node;
    },
  });
  await visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  expect(() => retained!.children).toThrow(/retained past its visitor pass/);
});

test("handles are kind-branded: cross-kind use is a compile error", () => {
  const { handle, source } = setup();
  const intoMdast = (h: MdastHandle): MdastHandle => h;
  const intoHast = (h: HastHandle): HastHandle => h;
  expect(intoMdast(handle)).toBe(handle);
  // @ts-expect-error a mdast handle must not flow into a hast-typed slot
  intoHast(handle);
  expect(getHandleSource(handle)).toBe(source);
});

// Ref-stub children: `.children` of a matched node returns id+type stubs that
// defer the arena snapshot until a real field is read, so passthrough children
// compile to one-word refs without ever materializing.

test("passthrough replacement keeps stub children rendering correctly", () => {
  const plugin = defineMdastPlugin({
    name: "heading-to-paragraph",
    heading(node) {
      return { type: "paragraph", children: node.children };
    },
  });
  const html = visitAndRender("# Hello **bold**", plugin);
  expect(html).toContain("<p>Hello <strong>bold</strong></p>");
});

test("reordering and filtering stub children works", () => {
  const plugin = defineMdastPlugin({
    name: "reverse-paragraph",
    paragraph(node, ctx) {
      // `type` is eager on stubs: this filter needs no materialization.
      const kept = node.children.filter((c) => c.type !== "emphasis");
      ctx.setProperty(node, "children", kept.reverse());
    },
  });
  const html = visitAndRender("*a* x **b**", plugin);
  expect(html).toContain("<strong>b</strong> x");
  expect(html).not.toContain("<em>");
});

test("stub `.type` stays readable after the pass; first materialization throws", () => {
  let retained: Heading["children"] = [];
  const plugin = defineMdastPlugin({
    name: "retain-heading-children",
    heading(node, ctx) {
      retained = node.children;
      // A mutation: the arena rebuilds after the pass, so stale ids must
      // refuse to materialize.
      ctx.setProperty(node, "depth", 2);
    },
  });
  markdownToHtml("# Hello\n\nWorld", { mdastPlugins: [plugin] });
  expect(retained).toHaveLength(1);
  const stub = retained[0]!;
  expect(stub.type).toBe("text");
  expect(() => stub.type === "text" && stub.value).toThrow(/retained past its visitor pass/);
});

test("a stub materialized after a manual applyCommandsToMdastHandle throws the retention error", () => {
  const { handle, source } = setup();
  let retained: Heading["children"] = [];
  const plugin = defineMdastPlugin({
    name: "retain-children-manual-apply",
    heading(node) {
      retained = node.children;
      return {
        type: "heading",
        depth: 2,
        children: [{ type: "text", value: "x" }],
      } satisfies MdastNode;
    },
  });
  const result = visitMdastHandle(
    handle,
    plugin,
    resolveMdastSubscriptions(plugin),
    source,
    undefined,
  ) as { commandBuffer: Uint8Array };
  // The wrapped mutator bumps the handle epoch: the rebuilt arena renumbered
  // the stub's id, so it must hit the retention error, not a RangeError.
  applyCommandsToMdastHandle(handle, result.commandBuffer);
  const stub = retained[0]!;
  expect(stub.type).toBe("text");
  expect(() => stub.type === "text" && stub.value).toThrow(/retained past its visitor pass/);
});

test("a spread copy of a child stub is new content, not a reused ref", () => {
  const plugin = defineMdastPlugin({
    name: "edit-spread-stub",
    heading(node) {
      const first = node.children[0]!;
      if (first.type !== "text") return;
      // A ref here would splice the original text and drop the edit.
      const copy = { ...first, value: "Edited" };
      return { type: "heading", depth: 2, children: [copy] };
    },
  });
  const html = visitAndRender("# Hello", plugin);
  expect(html).toContain("<h2>Edited</h2>");
});

test("parent of a top-level node is the root; the root has no parent", () => {
  const { handle, source } = setup();
  let parentType: string | undefined;
  let rootParent: unknown = "untouched";
  const plugin = defineMdastPlugin({
    name: "climb-to-root",
    heading(node, ctx) {
      const parent = ctx.parent(node);
      parentType = parent?.type;
      rootParent = parent ? ctx.parent(parent) : parent;
    },
  });
  visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  expect(parentType).toBe("root");
  expect(rootParent).toBeUndefined();
});

test("parent of a nested node is its container, and ancestors are climbable", () => {
  const handle = createMdastHandle("> nested *here*\n");
  const source = getHandleSource(handle);
  const chain: string[] = [];
  const plugin = defineMdastPlugin({
    name: "climb-ancestors",
    emphasis(node, ctx) {
      // Climbing reassigns from a possibly-root parent, so the loop var widens.
      let p: MdastNode | undefined = ctx.parent(node);
      while (p) {
        chain.push(p.type);
        p = ctx.parent(p);
      }
    },
  });
  visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  expect(chain).toEqual(["paragraph", "blockquote", "root"]);
});

test("parent returns the same object for the same parent across visits", () => {
  const handle = createMdastHandle("one\n\ntwo\n\nthree\n");
  const source = getHandleSource(handle);
  const parents = new Set<unknown>();
  let visits = 0;
  const plugin = defineMdastPlugin({
    name: "dedupe-parent",
    paragraph(node, ctx) {
      visits++;
      parents.add(ctx.parent(node));
    },
  });
  visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  expect(visits).toBe(3);
  expect(parents.size).toBe(1);
});

test("parent of a concrete non-root node narrows to non-null", () => {
  const { handle, source } = setup();
  let childCount = -1;
  const plugin = defineMdastPlugin({
    name: "narrowed-parent",
    heading(node, ctx) {
      // No `?.` or null check: a heading can't be the root, so the type is
      // non-null. A `?.` here would be a compile-time hint the narrowing broke.
      const parent = ctx.parent(node);
      childCount = "children" in parent ? parent.children.length : 0;
    },
  });
  visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  expect(childCount).toBe(2);
});

test("parent works on child stubs, not just visited nodes", () => {
  const { handle, source } = setup();
  let stubParentType: string | undefined;
  const plugin = defineMdastPlugin({
    name: "stub-parent",
    heading(node, ctx) {
      const firstChild: MdastNode = node.children[0]!;
      stubParentType = ctx.parent(firstChild)?.type;
    },
  });
  visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  expect(stubParentType).toBe("heading");
});

test("parent throws on plugin-built nodes (no arena id)", () => {
  const { handle, source } = setup();
  let error: Error | undefined;
  const plugin = defineMdastPlugin({
    name: "parent-of-new-node",
    heading(_node, ctx) {
      try {
        ctx.parent({ type: "paragraph", children: [] });
      } catch (e) {
        error = e as Error;
      }
    },
  });
  visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  expect(error?.message).toMatch(/no arena id/);
});

test("parent called after the pass throws the retention error", () => {
  const { handle, source } = setup();
  let retained: Readonly<Heading> | undefined;
  let retainedCtx: MdastVisitorContext | undefined;
  const plugin = defineMdastPlugin({
    name: "retain-for-parent",
    heading(node, ctx) {
      retained = node;
      retainedCtx = ctx;
    },
  });
  visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  expect(() => retainedCtx!.parent(retained!)).toThrow(/retained past its visitor pass/);
});

test("a parent is a valid anchor for structural mutations", () => {
  const plugin = defineMdastPlugin({
    name: "append-via-parent",
    heading(node, ctx) {
      const parent = ctx.parent(node);
      if (parent === undefined) return;
      ctx.appendChild(parent, {
        type: "paragraph",
        children: [{ type: "text", value: "appended at end" }],
      });
    },
  });
  const html = visitAndRender("# Title\n\nbody\n", plugin);
  expect(html.indexOf("appended at end")).toBeGreaterThan(html.indexOf("body"));
});

test("indexOf gives the node's position in its parent; root has none", () => {
  const { handle, source } = setup();
  const indexes: (number | undefined)[] = [];
  const plugin = defineMdastPlugin({
    name: "index-of",
    heading(node, ctx) {
      indexes.push(ctx.indexOf(node), ctx.indexOf(node.children[0]!));
      const root = ctx.parent(node)!;
      indexes.push(ctx.indexOf(root));
    },
  });
  visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  // heading is root's first child; its text stub is the heading's first child.
  expect(indexes).toEqual([0, 0, undefined]);
});

test("indexOf works where identity comparison against parent.children misses", () => {
  const handle = createMdastHandle("alpha\n\nbeta\n\ngamma\n");
  const source = getHandleSource(handle);
  let identityIndex: number | undefined;
  let idIndex: number | undefined;
  const plugin = defineMdastPlugin({
    name: "identity-vs-id",
    heading() {},
    paragraph(node, ctx) {
      if (ctx.textContent(node) !== "beta") return;
      const parent = ctx.parent(node)!;
      if (!("children" in parent)) return;
      const siblings: readonly MdastNode[] = parent.children;
      identityIndex = siblings.indexOf(node);
      idIndex = ctx.indexOf(node);
    },
  });
  visitMdastHandle(handle, plugin, resolveMdastSubscriptions(plugin), source, undefined);
  expect(identityIndex).toBe(-1);
  expect(idIndex).toBe(1);
});

test("indexOf-based insertion places content relative to the visited node", () => {
  const plugin = defineMdastPlugin({
    name: "insert-after-self",
    heading(node, ctx) {
      const parent = ctx.parent(node);
      if (parent === undefined) return;
      ctx.insertChildAt(parent, ctx.indexOf(node)! + 1, {
        type: "paragraph",
        children: [{ type: "text", value: "inserted" }],
      });
    },
  });
  const html = visitAndRender("# Title\n\nbody\n", plugin);
  expect(html.replaceAll("\n", "")).toBe("<h1>Title</h1><p>inserted</p><p>body</p>");
});

test("child edits land inside a parent-level restructure from the same pass", () => {
  const plugin = defineMdastPlugin({
    name: "reverse-and-uppercase",
    heading(node, ctx) {
      const parent = ctx.parent(node);
      if (parent === undefined || !("children" in parent)) return;
      ctx.setProperty(parent, "children", [...parent.children].reverse());
    },
    text(node, ctx) {
      ctx.setProperty(node, "value", node.value.toUpperCase());
    },
  });
  const html = visitAndRender("# Title\n\nbody\n", plugin);
  expect(html.replaceAll("\n", "")).toBe("<p>BODY</p><h1>TITLE</h1>");
});

test("parent and indexOf work inside an async visitor after the sync walk", async () => {
  const result = await markdownToHtml("# Title\n\nbody\n", {
    mdastPlugins: [
      defineMdastPlugin({
        name: "async-parent",
        async heading(node, ctx) {
          await new Promise((r) => setTimeout(r, 1));
          const parent = ctx.parent(node);
          if (parent === undefined) return;
          ctx.insertChildAt(parent, ctx.indexOf(node)! + 1, {
            type: "paragraph",
            children: [{ type: "text", value: "async insert" }],
          });
        },
      }),
    ],
  });
  expect(result.html.replaceAll("\n", "")).toBe("<h1>Title</h1><p>async insert</p><p>body</p>");
});

test("parent sees the rebuilt tree in a later plugin's pass", () => {
  const first = defineMdastPlugin({
    name: "first-pass-restructure",
    heading(node, ctx) {
      const parent = ctx.parent(node);
      if (parent === undefined || !("children" in parent)) return;
      ctx.setProperty(parent, "children", [...parent.children].reverse());
    },
  });
  const seen: (number | undefined)[] = [];
  const second = defineMdastPlugin({
    name: "second-pass-observe",
    heading(node, ctx) {
      seen.push(ctx.indexOf(node));
      const parent = ctx.parent(node);
      if (parent && "children" in parent) seen.push(parent.children.length);
    },
  });
  const { html } = markdownToHtml("# Title\n\nbody\n", { mdastPlugins: [first, second] });
  expect(html.replaceAll("\n", "")).toBe("<p>body</p><h1>Title</h1>");
  // After the first pass's reversal the heading is the root's second child.
  expect(seen).toEqual([1, 2]);
});

test("a node built by an earlier plugin is a real parent()-able node in the next pass", () => {
  const inserted = defineMdastPlugin({
    name: "insert-paragraph",
    heading(node, ctx) {
      ctx.insertAfter(node, {
        type: "paragraph",
        children: [{ type: "text", value: "from plugin one" }],
      });
    },
  });
  const seen: [string | undefined, number | undefined][] = [];
  const observe = defineMdastPlugin({
    name: "observe-inserted",
    paragraph(node, ctx) {
      if (ctx.textContent(node) !== "from plugin one") return;
      seen.push([ctx.parent(node)?.type, ctx.indexOf(node)]);
    },
  });
  markdownToHtml("# Title\n\nbody\n", { mdastPlugins: [inserted, observe] });
  // The inserted paragraph is a real node at index 1, after the heading.
  expect(seen).toEqual([["root", 1]]);
});

test("the plugin-built object itself stays id-less across passes", () => {
  const orphan = {
    type: "paragraph",
    children: [{ type: "text", value: "from plugin one" }],
  } satisfies MdastNode;
  const inserted = defineMdastPlugin({
    name: "insert-shared-object",
    heading(node, ctx) {
      ctx.insertAfter(node, orphan);
    },
  });
  let error: Error | undefined;
  const observe = defineMdastPlugin({
    name: "parent-of-shared-object",
    paragraph(_node, ctx) {
      try {
        ctx.parent(orphan);
      } catch (e) {
        error = e as Error;
      }
    },
  });
  markdownToHtml("# Title\n", { mdastPlugins: [inserted, observe] });
  // The built object never gets an arena id; the tree holds a node derived from it.
  expect(error?.message).toMatch(/no arena id/);
});

test("indexOf ignores buffered mutations within the same pass", () => {
  const pairs: [number | undefined, number | undefined][] = [];
  const plugin = defineMdastPlugin({
    name: "index-stable-under-mutation",
    paragraph(node, ctx) {
      const before = ctx.indexOf(node);
      ctx.insertBefore(node, { type: "paragraph", children: [{ type: "text", value: "x" }] });
      const after = ctx.indexOf(node);
      pairs.push([before, after]);
    },
  });
  const html = visitAndRender("alpha\n\nbeta\n", plugin);
  // Each insert is buffered, so it never shifts indexOf mid-pass.
  expect(pairs).toEqual([
    [0, 0],
    [1, 1],
  ]);
  // The inserts did apply: two originals plus two inserted paragraphs.
  expect(html.match(/<p>/g)).toHaveLength(4);
});
