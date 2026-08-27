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
    /contains its own insertion point/,
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
    /each reuse the other's node/,
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

test("hast: an element's own parent is accepted as an insert payload", () => {
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
  const { html } = markdownToHtml("a *b* c\n", { hastPlugins: [plugin] });
  expect((html.match(/<em>b<\/em>/g) ?? []).length).toBe(2);
});
