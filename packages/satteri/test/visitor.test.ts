import { test, expect } from "vitest";
import {
  visitMdastHandle,
  resolveMdastSubscriptions,
  type MdastVisitorContext,
} from "../src/mdast/mdast-visitor.js";
import {
  createMdastHandle,
  getHandleSource,
  applyCommandsAndConvertToHastHandle,
  renderHandle,
} from "../index.js";
import type { MdastNode } from "../src/types.js";
import { defineMdastPlugin } from "../src/plugin.js";

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
