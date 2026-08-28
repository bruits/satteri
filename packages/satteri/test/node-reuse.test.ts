import { test, expect } from "vitest";
import { markdownToHtml, defineMdastPlugin, defineHastPlugin } from "../src/index.js";

// Structural ops splice a reused tree node by id, so it resolves to whatever that node ends up as.

const twoQuotes = "> a\n\n> b\n";

test("a node moved with insertAfter + removeNode keeps its content", () => {
  let moved = false;
  const plugin = defineMdastPlugin({
    name: "move-first-quote-last",
    blockquote(node, ctx) {
      if (moved) return;
      moved = true;
      const siblings = ctx.parent(node).children;
      ctx.insertAfter(siblings[siblings.length - 1]!, node);
      ctx.removeNode(node);
    },
  });
  const { html } = markdownToHtml(twoQuotes, { mdastPlugins: [plugin] });
  expect(html).toBe(
    "<blockquote>\n<p>b</p>\n</blockquote>\n<blockquote>\n<p>a</p>\n</blockquote>\n",
  );
});

test("a node moved with insertAfter + removeNode keeps transforms made in the same pass", () => {
  let moved = false;
  const plugin = defineMdastPlugin({
    name: "move-and-transform",
    text(node) {
      if (node.value === "a") return { type: "text", value: "TRANSFORMED" };
    },
    blockquote(node, ctx) {
      if (moved) return;
      moved = true;
      const siblings = ctx.parent(node).children;
      ctx.insertAfter(siblings[siblings.length - 1]!, node);
      ctx.removeNode(node);
    },
  });
  const { html } = markdownToHtml(twoQuotes, { mdastPlugins: [plugin] });
  expect(html).toContain("TRANSFORMED");
});

test("insertAfter with the node itself duplicates it exactly once", () => {
  const plugin = defineMdastPlugin({
    name: "dup-quote",
    blockquote(node, ctx) {
      ctx.insertAfter(node, node);
    },
  });
  const { html } = markdownToHtml("> a\n", { mdastPlugins: [plugin] });
  expect((html.match(/<blockquote>/g) ?? []).length).toBe(2);
});

test("inserting a node inside itself is rejected at the call site", () => {
  const plugin = defineMdastPlugin({
    name: "insert-parent-after-child",
    paragraph(node, ctx) {
      const parent = ctx.parent(node);
      if (parent.type !== "blockquote") return;
      ctx.insertAfter(node, parent);
    },
  });
  expect(() => markdownToHtml("> a\n", { mdastPlugins: [plugin] })).toThrow(
    /content that contains the target node/,
  );
});

test("two inserts that each reuse the other's node are rejected at the call site", () => {
  let paired = false;
  const plugin = defineMdastPlugin({
    name: "mutual-insert",
    blockquote(node, ctx) {
      if (paired) return;
      paired = true;
      const siblings = ctx.parent(node).children;
      const other = siblings[siblings.length - 1]!;
      ctx.insertAfter(other, node);
      ctx.insertAfter(node, other);
    },
  });
  expect(() => markdownToHtml(twoQuotes, { mdastPlugins: [plugin] })).toThrow(
    /closes a cycle of inserts/,
  );
});

test("an inserted tree node carries transforms made to it in the same pass", () => {
  const plugin = defineMdastPlugin({
    name: "mixed-vintage",
    emphasis(node, ctx) {
      ctx.setProperty(node, "data", { hName: "mark" });
    },
    text(node) {
      if (node.value === "label") return { type: "text", value: "CHANGED" };
    },
    code(node, ctx) {
      const index = ctx.indexOf(node);
      if (index === undefined) return;
      const previous = ctx.parent(node).children[index - 1];
      if (previous?.type !== "paragraph") return;
      ctx.insertAfter(node, [previous.children[1]!]);
    },
  });
  const { html } = markdownToHtml("Some *label* text\n\n```js\nx\n```\n", {
    mdastPlugins: [plugin],
  });
  expect((html.match(/<mark>/g) ?? []).length).toBe(2);
});

test("hast: an element moved with insertAfter + removeNode keeps its content", () => {
  let moved = false;
  const plugin = defineHastPlugin({
    name: "move-first-heading-last",
    element: {
      filter: ["h1"],
      visit(node, ctx) {
        if (moved) return;
        moved = true;
        const siblings = ctx.parent(node).children;
        ctx.insertAfter(siblings[siblings.length - 1]!, node);
        ctx.removeNode(node);
      },
    },
  });
  const { html } = markdownToHtml("# one\n\nbody\n", { hastPlugins: [plugin] });
  expect(html).toContain("<h1>one</h1>");
  expect(html.indexOf("<h1>")).toBeGreaterThan(html.indexOf("<p>"));
});

test("hast: insertAfter with the element itself duplicates it exactly once", () => {
  const plugin = defineHastPlugin({
    name: "dup-heading",
    element: {
      filter: ["h1"],
      visit(node, ctx) {
        ctx.insertAfter(node, node);
      },
    },
  });
  const { html } = markdownToHtml("# one\n", { hastPlugins: [plugin] });
  expect((html.match(/<h1>one<\/h1>/g) ?? []).length).toBe(2);
});

test("hast: inserting an element inside itself is rejected at the call site", () => {
  const plugin = defineHastPlugin({
    name: "insert-parent-after-child",
    element: {
      filter: ["em"],
      visit(node, ctx) {
        const parent = ctx.parent(node);
        if (parent.type !== "element") return;
        ctx.insertAfter(node, parent);
      },
    },
  });
  expect(() => markdownToHtml("a *b* c\n", { hastPlugins: [plugin] })).toThrow(
    /content that contains the target node/,
  );
});

test("hast: an inserted element carries transforms made to it in the same pass", () => {
  const plugin = defineHastPlugin({
    name: "hast-mixed-vintage",
    element: {
      filter: ["em"],
      visit(node, ctx) {
        ctx.setProperty(node, "className", ["marked"]);
        const siblings = ctx.parent(node).children;
        const last = siblings[siblings.length - 1]!;
        if (last !== node) ctx.insertAfter(last, node);
      },
    },
  });
  const { html } = markdownToHtml("a *b* c\n", { hastPlugins: [plugin] });
  expect((html.match(/class="marked"/g) ?? []).length).toBe(2);
});

test("structuredClone works on nodes read from the tree, in hooks and visitors", () => {
  const cloned: string[] = [];
  const plugin = defineMdastPlugin({
    name: "clone-nodes",
    before(root) {
      cloned.push(structuredClone(root).type, structuredClone(root.children[0]!).type);
    },
    paragraph(node, ctx) {
      cloned.push(structuredClone(node).type, structuredClone(ctx.parent(node)).type);
    },
  });
  markdownToHtml("one\n\ntwo\n", { mdastPlugins: [plugin] });
  expect(cloned).toEqual(["root", "paragraph", "paragraph", "root", "paragraph", "root"]);
});

test("a cloned node carries no internal fields", () => {
  const keys: string[][] = [];
  const plugin = defineMdastPlugin({
    name: "clone-keys",
    before(root) {
      keys.push(Object.keys(structuredClone(root.children[0]!)));
      keys.push(Object.keys({ ...root.children[0]! }));
    },
  });
  markdownToHtml("one\n", { mdastPlugins: [plugin] });
  for (const k of keys) expect(k.filter((n) => n.startsWith("_"))).toEqual([]);
});

test("hast: structuredClone works on elements read from the tree", () => {
  const cloned: string[] = [];
  const plugin = defineHastPlugin({
    name: "clone-elements",
    element: {
      filter: ["em"],
      visit(node, ctx) {
        cloned.push(structuredClone(node).tagName, structuredClone(ctx.parent(node)).type);
      },
    },
  });
  markdownToHtml("a *b* c\n", { hastPlugins: [plugin] });
  expect(cloned).toEqual(["em", "element"]);
});
