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

test("a node replaced by several nodes is reused as all of them", () => {
  let done = false;
  const plugin = defineMdastPlugin({
    name: "multi-replace-then-reuse",
    blockquote(node, ctx) {
      if (done) return;
      done = true;
      const siblings = ctx.parent(node).children;
      ctx.replaceNode(node, [
        { type: "paragraph", children: [{ type: "text", value: "X" }] },
        { type: "paragraph", children: [{ type: "text", value: "Y" }] },
      ]);
      ctx.insertAfter(siblings[siblings.length - 1]!, {
        type: "blockquote",
        children: [node],
      });
    },
  });
  const { html } = markdownToHtml(twoQuotes, { mdastPlugins: [plugin] });
  expect(html).toContain("<blockquote>\n<p>X</p>\n<p>Y</p>\n</blockquote>");
});

test("hast: an element replaced by several elements is reused as all of them", () => {
  let done = false;
  const plugin = defineHastPlugin({
    name: "hast-multi-replace-then-reuse",
    element: {
      filter: ["h1"],
      visit(node, ctx) {
        if (done) return;
        done = true;
        const siblings = ctx.parent(node).children;
        ctx.replaceNode(node, [
          { type: "element", tagName: "b", properties: {}, children: [] },
          { type: "element", tagName: "i", properties: {}, children: [] },
        ]);
        ctx.insertAfter(siblings[siblings.length - 1]!, {
          type: "element",
          tagName: "section",
          properties: {},
          children: [node],
        });
      },
    },
  });
  const { html } = markdownToHtml("# one\n\nbody\n", { hastPlugins: [plugin] });
  expect(html).toContain("<section><b></b><i></i></section>");
});

test("a node cannot be made its own child", () => {
  const asChild = (op: "appendChild" | "prependChild") =>
    defineMdastPlugin({
      name: `self-${op}`,
      blockquote(node, ctx) {
        ctx[op](node, node);
      },
    });
  for (const op of ["appendChild", "prependChild"] as const) {
    expect(() => markdownToHtml("> x\n", { mdastPlugins: [asChild(op)] })).toThrow(
      /content that contains the target node/,
    );
  }
});

test("the root cannot be made its own child", () => {
  const plugin = defineMdastPlugin({
    name: "self-root",
    paragraph(node, ctx) {
      const root = ctx.parent(node);
      ctx.appendChild(root, root);
    },
  });
  expect(() => markdownToHtml("x\n", { mdastPlugins: [plugin] })).toThrow(
    /content that contains the target node/,
  );
});

test("hast: an element cannot be made its own child", () => {
  const plugin = defineHastPlugin({
    name: "hast-self-child",
    element: {
      filter: ["em"],
      visit(node, ctx) {
        ctx.appendChild(node, node);
      },
    },
  });
  expect(() => markdownToHtml("a *b* c\n", { hastPlugins: [plugin] })).toThrow(
    /content that contains the target node/,
  );
});

test("a long chain of reuses is not mistaken for a cycle", () => {
  let previous: Readonly<{ type: string }> | undefined;
  const plugin = defineMdastPlugin({
    name: "chain",
    paragraph(node, ctx) {
      if (previous !== undefined) ctx.insertAfter(node, previous as never);
      previous = node;
    },
  });
  const source = Array.from({ length: 400 }, (_, i) => `p${i}`).join("\n\n") + "\n";
  const { html } = markdownToHtml(source, { mdastPlugins: [plugin] });
  expect((html.match(/<p>/g) ?? []).length).toBe(799);
});
