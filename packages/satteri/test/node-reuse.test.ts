import { test, expect, vi } from "vitest";
import type { Blockquote, List, Paragraph } from "mdast";
import type { Element } from "hast";
import {
  markdownToHtml,
  defineMdastPlugin,
  defineHastPlugin,
  type HastVisitorContext,
  type MdastVisitorContext,
} from "../src/index.js";

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

test("inserting a node inside itself is rejected when commands are applied", () => {
  const plugin = defineMdastPlugin({
    name: "insert-parent-after-child",
    paragraph(node, ctx) {
      const parent = ctx.parent(node);
      if (parent.type !== "blockquote") return;
      ctx.insertAfter(node, parent);
    },
  });
  expect(() => markdownToHtml("> a\n", { mdastPlugins: [plugin] })).toThrow(
    /payload ref targets an ancestor of its anchor/,
  );
});

test("two inserts that each reuse the other's node are rejected when commands are applied", () => {
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
    /ref-dependency cycle/,
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

test("hast: inserting an element inside itself is rejected when commands are applied", () => {
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
    /payload ref targets an ancestor of its anchor/,
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

test("foreign references must be detached before insertion", () => {
  let saved: Blockquote | undefined;
  let detached: Blockquote | undefined;
  markdownToHtml("> first\n", {
    mdastPlugins: [
      defineMdastPlugin({
        name: "save-reference",
        blockquote(node) {
          saved = node;
          detached = structuredClone(node);
        },
      }),
    ],
  });
  const foreign = saved!;
  const insert = (child: typeof foreign) =>
    defineMdastPlugin({
      name: "insert-reference",
      blockquote(node, ctx) {
        ctx.insertAfter(node, { type: "blockquote", children: [child] });
      },
    });
  expect(() => markdownToHtml("> second\n", { mdastPlugins: [insert(foreign)] })).toThrow(
    /another tree or pass/,
  );
  expect(markdownToHtml("> second\n", { mdastPlugins: [insert(detached!)] }).html).toContain(
    "<blockquote>\n<blockquote>\n<p>first</p>",
  );
});

test("hast: foreign references must be detached before insertion", () => {
  let saved: Element | undefined;
  markdownToHtml("a *b* c\n", {
    hastPlugins: [
      defineHastPlugin({
        name: "save-element",
        element: {
          filter: ["em"],
          visit(node) {
            saved = node;
          },
        },
      }),
    ],
  });
  const foreign = saved!;
  const plugin = defineHastPlugin({
    name: "insert-foreign-element",
    element: {
      filter: ["p"],
      visit(node, ctx) {
        ctx.appendChild(node, foreign);
      },
    },
  });
  expect(() => markdownToHtml("text\n", { hastPlugins: [plugin] })).toThrow(/another tree or pass/);
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
      /payload ref would contain its anchor/,
    );
  }
});

test("a nested self-child fails after the visitor returns", () => {
  let returned = false;
  const plugin = defineMdastPlugin({
    name: "nested-self-child",
    blockquote(node, ctx) {
      ctx.appendChild(node, { type: "blockquote", children: [node] });
      returned = true;
    },
  });
  expect(() => markdownToHtml("> x\n", { mdastPlugins: [plugin] })).toThrow(
    /payload ref would contain its anchor/,
  );
  expect(returned).toBe(true);
});

test("replacement content can wrap the node being replaced", () => {
  const plugin = defineMdastPlugin({
    name: "nested-self-replacement",
    blockquote(node, ctx) {
      ctx.replaceNode(node, { type: "blockquote", children: [node] });
    },
  });
  const { html } = markdownToHtml("> x\n", { mdastPlugins: [plugin] });
  expect((html.match(/<blockquote>/g) ?? []).length).toBe(2);
});

test("replacement root cannot contain a reference to itself", () => {
  const plugin = defineMdastPlugin({
    name: "root-cycle",
    before(root, ctx) {
      ctx.replaceNode(root, { type: "root", children: [root] });
    },
  });
  expect(() => markdownToHtml("text\n", { mdastPlugins: [plugin] })).toThrow(
    /payload ref would contain its anchor/,
  );
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
    /payload ref would contain its anchor/,
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
    /payload ref would contain its anchor/,
  );
});

test("hast: an element cannot be nested inside new content appended to itself", () => {
  const plugin = defineHastPlugin({
    name: "hast-nested-self-child",
    element: {
      filter: ["em"],
      visit(node, ctx) {
        ctx.appendChild(node, {
          type: "element",
          tagName: "span",
          properties: {},
          children: [node],
        });
      },
    },
  });
  expect(() => markdownToHtml("a *b* c\n", { hastPlugins: [plugin] })).toThrow(
    /payload ref would contain its anchor/,
  );
});

test("hast: replacement content can wrap the element being replaced", () => {
  const plugin = defineHastPlugin({
    name: "hast-nested-self-replacement",
    element: {
      filter: ["em"],
      visit(node, ctx) {
        ctx.replaceNode(node, {
          type: "element",
          tagName: "span",
          properties: {},
          children: [node],
        });
      },
    },
  });
  const { html } = markdownToHtml("a *b* c\n", { hastPlugins: [plugin] });
  expect(html).toContain("<span><em>b</em></span>");
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

test("keeping a node's children keeps a sibling inserted among them", () => {
  let done = false;
  const plugin = defineMdastPlugin({
    name: "keep-children-with-insert",
    blockquote(node, ctx) {
      if (done) return;
      done = true;
      ctx.insertAfter(node.children[1]!, { type: "thematicBreak" });
      return { ...node, _keepChildren: true } as never;
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n>\n> c\n", { mdastPlugins: [plugin] });
  expect(html).toContain("<p>b</p>\n<hr>\n<p>c</p>");
});

test("keeping a node's children keeps a removal among them", () => {
  const plugin = defineMdastPlugin({
    name: "keep-children-with-removal",
    blockquote(node) {
      return { ...node, data: { hName: "section" }, _keepChildren: true } as never;
    },
    paragraph(node, ctx) {
      if (ctx.indexOf(node) === 0) ctx.removeNode(node);
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<section>\n<p>b</p>\n</section>\n");
});

test("a node returned with its own children keeps the edits queued around them", () => {
  const plugin = defineMdastPlugin({
    name: "tabs",
    containerDirective(node, ctx) {
      ctx.insertBefore(node, { type: "html", value: `<${node.name}>` });
      ctx.insertAfter(node, { type: "html", value: `</${node.name}>` });
      return { ...node, data: { hName: "div" } };
    },
  });
  const source = ":::tabs\n::::tab\na\n::::\n::::tab\nb\n::::\n:::\n";
  const { html } = markdownToHtml(source, {
    features: { directive: true },
    mdastPlugins: [plugin],
  });
  expect(html).toBe(
    "<tabs>\n<div><tab><div><p>a</p></div></tab><tab><div><p>b</p></div></tab></div>\n</tabs>\n",
  );
});

test("a node returned with its own children keeps a removal among them", () => {
  const plugin = defineMdastPlugin({
    name: "drop-first-paragraph",
    blockquote(node) {
      return { ...node, data: { hName: "section" } };
    },
    paragraph(node, ctx) {
      if (ctx.indexOf(node) === 0) ctx.removeNode(node);
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<section>\n<p>b</p>\n</section>\n");
});

test("unwrapping a node keeps the edits queued around its children", () => {
  const plugin = defineMdastPlugin({
    name: "unwrap-quote",
    blockquote(node, ctx) {
      ctx.replaceNode(node, node.children);
    },
    paragraph(node, ctx) {
      if (ctx.indexOf(node) === 0) ctx.insertAfter(node, { type: "thematicBreak" });
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<p>a</p>\n<hr>\n<p>b</p>\n");
});

test("children handed back by a replacement belong to it in later plugins", () => {
  const replace = defineMdastPlugin({
    name: "quote-to-section",
    blockquote(node, ctx) {
      ctx.insertAfter(node.children[0]!, { type: "thematicBreak" });
      return { ...node, data: { hName: "section" } };
    },
  });
  const parents: unknown[] = [];
  const probe = defineMdastPlugin({
    name: "probe-parent",
    paragraph(node, ctx) {
      parents.push(ctx.parent(node).data);
    },
  });
  markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [replace, probe] });
  expect(parents).toEqual([{ hName: "section" }, { hName: "section" }]);
});

test("hast: an element returned with its own children keeps the edits queued around them", () => {
  const plugin = defineHastPlugin({
    name: "hast-handed-back-children",
    element: {
      filter: ["blockquote", "p"],
      visit(node, ctx) {
        if (node.tagName === "blockquote") return { ...node, tagName: "section" };
        const text = node.children[0];
        if (text?.type !== "text") return;
        if (text.value === "a") {
          ctx.insertAfter(node, { type: "element", tagName: "hr", properties: {}, children: [] });
        } else {
          ctx.removeNode(node);
        }
      },
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { hastPlugins: [plugin] });
  expect(html).toBe("<section>\n<p>a</p><hr>\n\n</section>\n");
});

test("a child its parent's replacement lists in a new order moves with its edits", () => {
  const plugin = defineMdastPlugin({
    name: "move-first-paragraph-last",
    blockquote(node) {
      const [first, ...rest] = node.children;
      if (!first) return;
      return { ...node, children: [...rest, first] };
    },
    paragraph(node, ctx) {
      ctx.appendChild(node, { type: "text", value: "!" });
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<blockquote>\n<p>b!</p>\n<p>a!</p>\n</blockquote>\n");
});

test("hast: a child its parent's replacement lists in a new order moves", () => {
  const plugin = defineHastPlugin({
    name: "hast-move-first-paragraph-last",
    element: {
      filter: ["blockquote"],
      visit(node) {
        const first = node.children.find((child) => child.type === "element");
        if (!first) return;
        return { ...node, children: [...node.children.filter((child) => child !== first), first] };
      },
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { hastPlugins: [plugin] });
  expect(html).toBe("<blockquote>\n\n<p>b</p>\n<p>a</p></blockquote>\n");
});

function moveFirstChildLast(node: Blockquote) {
  const [first, ...rest] = node.children;
  if (!first) return;
  return { ...node, children: [...rest, first] };
}

test("an async visitor's new order moves the children", async () => {
  const asyncVisit = defineMdastPlugin({
    name: "async-move-first-paragraph-last",
    async blockquote(node) {
      await Promise.resolve();
      return moveFirstChildLast(node);
    },
  });
  const promiseVisit = defineMdastPlugin({
    name: "promise-move-first-paragraph-last",
    blockquote(node) {
      return Promise.resolve().then(() => moveFirstChildLast(node));
    },
  });
  for (const plugin of [asyncVisit, promiseVisit]) {
    const { html } = await markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
    expect(html, plugin.name).toBe("<blockquote>\n<p>b</p>\n<p>a</p>\n</blockquote>\n");
  }
});

test("hast: an async visitor's new order moves the children", async () => {
  const plugin = defineHastPlugin({
    name: "hast-async-move-first-paragraph-last",
    element: {
      filter: ["blockquote"],
      async visit(node) {
        await Promise.resolve();
        const first = node.children.find((child) => child.type === "element");
        if (!first) return;
        return { ...node, children: [...node.children.filter((child) => child !== first), first] };
      },
    },
  });
  const { html } = await markdownToHtml("> a\n>\n> b\n", { hastPlugins: [plugin] });
  expect(html).toBe("<blockquote>\n\n<p>b</p>\n<p>a</p></blockquote>\n");
});

function asSection(node: Blockquote) {
  return { ...node, data: { hName: "section" } };
}

test("a removed child stays removed when its parent returns a spread copy", async () => {
  const removeFirstParagraph = defineMdastPlugin({
    name: "remove-first-paragraph",
    blockquote: asSection,
    paragraph(node, ctx) {
      if (ctx.indexOf(node) === 0) ctx.removeNode(node);
    },
  });
  const removeThenSpread = defineMdastPlugin({
    name: "remove-then-spread",
    blockquote(node, ctx) {
      const [first] = node.children;
      if (first) ctx.removeNode(first);
      return asSection(node);
    },
  });
  const asyncParent = defineMdastPlugin({
    name: "async-parent",
    async blockquote(node) {
      await Promise.resolve();
      return asSection(node);
    },
    paragraph(node, ctx) {
      if (ctx.indexOf(node) === 0) ctx.removeNode(node);
    },
  });
  for (const plugin of [removeFirstParagraph, removeThenSpread, asyncParent]) {
    const { html } = await markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
    expect(html, plugin.name).toBe("<section>\n<p>b</p>\n</section>\n");
  }

  const earlierVisit = defineMdastPlugin({
    name: "earlier-visit-removes",
    heading(node, ctx) {
      const quote = ctx.parent(node)?.children.find((child) => child.type === "blockquote");
      const [first] = quote?.type === "blockquote" ? quote.children : [];
      if (first) ctx.removeNode(first);
    },
    blockquote: asSection,
  });
  const { html } = markdownToHtml("# t\n\n> a\n>\n> b\n", { mdastPlugins: [earlierVisit] });
  expect(html).toBe("<h1>t</h1>\n<section>\n<p>b</p>\n</section>\n");
});

function removeParagraphA(node: Element, ctx: HastVisitorContext) {
  const [text] = node.children;
  if (text?.type === "text" && text.value === "a") ctx.removeNode(node);
}

test("hast: a removed child stays removed when its parent returns a spread copy", async () => {
  const syncParent = defineHastPlugin({
    name: "hast-sync-parent",
    element: {
      filter: ["blockquote", "p"],
      visit(node, ctx) {
        if (node.tagName === "blockquote") return { ...node, tagName: "section" };
        removeParagraphA(node, ctx);
      },
    },
  });
  const asyncParent = defineHastPlugin({
    name: "hast-async-parent",
    element: {
      filter: ["blockquote", "p"],
      async visit(node, ctx) {
        await Promise.resolve();
        if (node.tagName === "blockquote") return { ...node, tagName: "section" };
        removeParagraphA(node, ctx);
      },
    },
  });
  for (const plugin of [syncParent, asyncParent]) {
    const { html } = await markdownToHtml("> a\n>\n> b\n", { hastPlugins: [plugin] });
    expect(html, plugin.name).toBe("<section>\n\n<p>b</p>\n</section>\n");
  }
});

function isParagraphA(node: Paragraph) {
  const [text] = node.children;
  return text?.type === "text" && text.value === "a";
}

function removeMdastParagraphA(node: Paragraph, ctx: MdastVisitorContext) {
  if (isParagraphA(node)) ctx.removeNode(node);
}

test("an async parent's new list does not undo a removal another visitor makes meanwhile", async () => {
  const asyncReturn = defineMdastPlugin({
    name: "async-reverse",
    async blockquote(node) {
      await Promise.resolve();
      return { ...node, children: node.children.toReversed() };
    },
    paragraph: removeMdastParagraphA,
  });
  const promiseReturn = defineMdastPlugin({
    name: "promise-reverse",
    blockquote(node) {
      return Promise.resolve().then(() => ({ ...node, children: node.children.toReversed() }));
    },
    paragraph: removeMdastParagraphA,
  });
  const asyncSetProperty = defineMdastPlugin({
    name: "async-set-reversed-children",
    async blockquote(node, ctx) {
      await Promise.resolve();
      ctx.setProperty(node, "children", node.children.toReversed());
    },
    paragraph: removeMdastParagraphA,
  });
  for (const plugin of [asyncReturn, promiseReturn, asyncSetProperty]) {
    const { html } = await markdownToHtml("> a\n>\n> b\n>\n> c\n", { mdastPlugins: [plugin] });
    expect(html, plugin.name).toBe("<blockquote>\n<p>c</p>\n<p>b</p>\n</blockquote>\n");
  }
});

function elementsOnly(node: Element) {
  return { ...node, children: node.children.filter((child) => child.type === "element") };
}

test("hast: an async parent's new list does not undo a removal another visitor makes meanwhile", async () => {
  const asyncReturn = defineHastPlugin({
    name: "hast-async-elements-only",
    element: {
      filter: ["blockquote", "p"],
      async visit(node, ctx) {
        if (node.tagName === "p") return removeParagraphA(node, ctx);
        await Promise.resolve();
        return elementsOnly(node);
      },
    },
  });
  const promiseReturn = defineHastPlugin({
    name: "hast-promise-elements-only",
    element: {
      filter: ["blockquote", "p"],
      visit(node, ctx) {
        if (node.tagName === "p") return removeParagraphA(node, ctx);
        return Promise.resolve().then(() => elementsOnly(node));
      },
    },
  });
  const asyncSetProperty = defineHastPlugin({
    name: "hast-async-set-elements-only",
    element: {
      filter: ["blockquote", "p"],
      async visit(node, ctx) {
        if (node.tagName === "p") return removeParagraphA(node, ctx);
        await Promise.resolve();
        ctx.setProperty(node, "children", elementsOnly(node).children);
      },
    },
  });
  for (const plugin of [asyncReturn, promiseReturn, asyncSetProperty]) {
    const { html } = await markdownToHtml("> a\n>\n> b\n", { hastPlugins: [plugin] });
    expect(html, plugin.name).toBe("<blockquote><p>b</p></blockquote>\n");
  }
});

test("an async grandparent's copies of its children keep a grandchild removed meanwhile", async () => {
  const plugin = defineMdastPlugin({
    name: "async-copy-children",
    async blockquote(node, ctx) {
      if (ctx.parent(node)?.type !== "root") return;
      await Promise.resolve();
      return {
        ...node,
        children: node.children.map((child) => ({ ...child, data: { hName: "aside" } })),
      };
    },
    paragraph(node, ctx) {
      const [text] = node.children;
      if (text?.type === "text" && text.value === "a") ctx.removeNode(node);
    },
  });
  const { html } = await markdownToHtml("> > a\n> >\n> > b\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<blockquote>\n<aside>\n<p>b</p>\n</aside>\n</blockquote>\n");
});

test("a grandchild moved out of a removed parent takes the content queued beside it", () => {
  const plugin = defineMdastPlugin({
    name: "hoist-first-paragraph",
    blockquote(node, ctx) {
      const [inner] = node.children;
      if (ctx.parent(node)?.type !== "root" || inner?.type !== "blockquote") return;
      const [first] = inner.children;
      if (!first) return;
      ctx.insertAfter(node, first);
      ctx.removeNode(node);
    },
    paragraph(node, ctx) {
      ctx.insertAfter(node, { type: "thematicBreak" });
    },
  });
  const { html } = markdownToHtml("> > a\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<p>a</p>\n<hr>\n");
});

test("a child moved by editing the children array in place moves", () => {
  const plugin = defineMdastPlugin({
    name: "move-in-place",
    blockquote(node) {
      const first = node.children.shift();
      if (first) node.children.push(first);
      return asSection(node);
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<section>\n<p>b</p>\n<p>a</p>\n</section>\n");
});

function doneItemsLast(node: List) {
  const done = node.children.filter((item) => item.checked);
  return { ...node, children: [...node.children.filter((item) => !item.checked), ...done] };
}

test("moving done items to the bottom keeps them wherever they start", () => {
  const plugin = defineMdastPlugin({ name: "done-items-last", list: doneItemsLast });
  for (const source of ["- [ ] a\n- [x] b\n", "- [x] b\n- [ ] a\n"]) {
    const { html } = markdownToHtml(source, { mdastPlugins: [plugin] });
    expect(html, source).toContain("> a</li>");
    expect(html.indexOf("> a</li>"), source).toBeLessThan(html.indexOf("> b</li>"));
  }
});

test("a grandchild removed after its grandparent returns copies of its children stays removed", () => {
  const plugin = defineMdastPlugin({
    name: "copy-children-as-asides",
    blockquote(node, ctx) {
      if (ctx.parent(node)?.type !== "root") return;
      return {
        ...node,
        children: node.children.map((child) => ({ ...child, data: { hName: "aside" } })),
      };
    },
    paragraph(node, ctx) {
      const [text] = node.children;
      if (text?.type === "text" && text.value === "a") ctx.removeNode(node);
    },
  });
  const { html } = markdownToHtml("> > a\n> >\n> > b\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<blockquote>\n<aside>\n<p>b</p>\n</aside>\n</blockquote>\n");
});

test("a child removed while its parent is wrapped in raw items stays removed", () => {
  const plugin = defineMdastPlugin({
    name: "wrap-in-div",
    blockquote(node, ctx) {
      ctx.replaceNode(node, [{ raw: "<div>" }, ...node.children, { raw: "</div>" }]);
    },
    paragraph(node, ctx) {
      const [text] = node.children;
      if (text?.type === "text" && text.value === "a") ctx.removeNode(node);
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<div>\n<p>b</p>\n</div>\n");
});

test("content inserted after a node stays after the raw items that replace it", () => {
  const plugin = defineMdastPlugin({
    name: "insert-then-wrap",
    heading(node, ctx) {
      ctx.insertAfter(node, { type: "paragraph", children: [{ type: "text", value: "after" }] });
      ctx.replaceNode(node, [{ raw: "<div>" }, node, { raw: "</div>" }]);
    },
  });
  const { html } = markdownToHtml("# h\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<div>\n<h1>h</h1>\n</div>\n<p>after</p>\n");
});

test("a kept-children replacement after other items keeps the edits among the children", () => {
  const plugin = defineMdastPlugin({
    name: "rule-then-section",
    blockquote(node, ctx) {
      ctx.replaceNode(node, [
        { type: "thematicBreak" },
        { ...node, data: { hName: "section" }, _keepChildren: true } as never,
      ]);
    },
    paragraph(node, ctx) {
      if (ctx.indexOf(node) === 0) ctx.insertAfter(node, { type: "html", value: "<i>x</i>" });
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<hr>\n<section>\n<p>a</p>\n<i>x</i>\n<p>b</p>\n</section>\n");
});

test("a children list set on a node that is then replaced does not undo a removal", () => {
  const plugin = defineMdastPlugin({
    name: "set-children-then-spread",
    blockquote(node, ctx) {
      const [first, ...rest] = node.children;
      if (!first) return;
      ctx.removeNode(first);
      ctx.setProperty(node, "children", [...rest, first]);
      return asSection(node);
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<section>\n<p>b</p>\n</section>\n");
});

test("unwrapping a node with insertBefore and removeNode keeps the edits beside its children", () => {
  const plugin = defineMdastPlugin({
    name: "flatten-quotes",
    blockquote(node, ctx) {
      ctx.insertBefore(node, node.children);
      ctx.removeNode(node);
    },
    paragraph(node, ctx) {
      ctx.insertAfter(node, { type: "thematicBreak" });
    },
  });
  const { html } = markdownToHtml("> Body\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<p>Body</p>\n<hr>\n");
});

test("a node appended elsewhere moves whether it is removed before or after", () => {
  const removeFirst = defineMdastPlugin({
    name: "remove-then-append",
    heading(node, ctx) {
      const root = ctx.parent(node);
      ctx.removeNode(node);
      if (root) ctx.appendChild(root, node);
    },
  });
  const appendFirst = defineMdastPlugin({
    name: "append-then-remove",
    heading(node, ctx) {
      const root = ctx.parent(node);
      if (root) ctx.appendChild(root, node);
      ctx.removeNode(node);
    },
  });
  for (const plugin of [removeFirst, appendFirst]) {
    const { html } = markdownToHtml("# h\n\np\n", { mdastPlugins: [plugin] });
    expect(html, plugin.name).toBe("<p>p</p>\n<h1>h</h1>\n");
  }
});

test("a child moved into a new wrapper keeps its place there", () => {
  const plugin = defineMdastPlugin({
    name: "nest-first-paragraph",
    blockquote(node) {
      const [first, ...rest] = node.children;
      if (!first || node.data?.hName) return;
      return {
        ...node,
        children: [...rest, { type: "blockquote", data: { hName: "aside" }, children: [first] }],
      };
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<blockquote>\n<p>b</p>\n<aside>\n<p>a</p>\n</aside>\n</blockquote>\n");
});

test("hast: a child moved into a new wrapper keeps its place there", () => {
  const plugin = defineHastPlugin({
    name: "hast-nest-first-paragraph",
    element: {
      filter: ["blockquote"],
      visit(node) {
        const first = node.children.find((child) => child.type === "element");
        if (!first) return;
        const wrapper = {
          type: "element",
          tagName: "div",
          properties: {},
          children: [first],
        } satisfies Element;
        return { ...node, children: [wrapper] };
      },
    },
  });
  const { html } = markdownToHtml("> a\n", { hastPlugins: [plugin] });
  expect(html).toBe("<blockquote><div><p>a</p></div></blockquote>\n");
});

test("returning a child as the replacement unwraps the node", () => {
  const plugin = defineMdastPlugin({
    name: "unwrap-first-child",
    blockquote(node) {
      return node.children[0];
    },
  });
  const { html } = markdownToHtml("> a\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<p>a</p>\n");
});

test("a child listed in a new order through setProperty moves", () => {
  const plugin = defineMdastPlugin({
    name: "move-first-paragraph-last-in-place",
    blockquote(node, ctx) {
      const [first, ...rest] = node.children;
      if (!first) return;
      ctx.setProperty(node, "children", [...rest, first]);
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<blockquote>\n<p>b</p>\n<p>a</p>\n</blockquote>\n");
});

function relistAfterRemoving(
  name: string,
  relist: (node: Blockquote, ctx: MdastVisitorContext, first: Paragraph) => Blockquote | void,
) {
  return defineMdastPlugin({
    name,
    blockquote(node, ctx) {
      const [first] = node.children;
      if (first?.type !== "paragraph") return;
      ctx.removeNode(first);
      return relist(node, ctx, first);
    },
  });
}

test("a removed child listed again stays removed and the plugin is warned", () => {
  const plugins = [
    relistAfterRemoving("new-order", (node, _ctx, first) => ({
      ...node,
      children: [...node.children.slice(1), first],
    })),
    relistAfterRemoving("set-children", (node, ctx, first) => {
      ctx.setProperty(node, "children", [...node.children.slice(1), first]);
    }),
    relistAfterRemoving("replace-node", (node, ctx, first) => {
      ctx.replaceNode(node, { ...node, children: [first] });
    }),
    relistAfterRemoving("new-wrapper", (node, _ctx, first) => ({
      ...node,
      children: [{ type: "blockquote", data: { hName: "aside" }, children: [first] }],
    })),
    relistAfterRemoving("edited-in-place", (node, _ctx, first) => {
      node.children.shift();
      node.children.push(first);
      return asSection(node);
    }),
    relistAfterRemoving("beside-removed-parent", (node, ctx, first) => {
      ctx.removeNode(node);
      ctx.insertAfter(node, first);
    }),
    relistAfterRemoving("beside-parent-removed-later", (node, ctx, first) => {
      ctx.insertAfter(node, first);
      ctx.removeNode(node);
    }),
    relistAfterRemoving("listed-twice", (node, _ctx, first) => ({
      ...node,
      children: [first, ...node.children.slice(1), first],
    })),
    relistAfterRemoving("set-field-after-replace", (node, ctx, first) => {
      const children = [...node.children.slice(1), first];
      ctx.replaceNode(node, asSection(node));
      ctx.setField(node, "children", children);
    }),
  ];
  const doneItems = defineMdastPlugin({
    name: "done-items-removed",
    list(node, ctx) {
      for (const item of node.children) if (item.checked) ctx.removeNode(item);
      return doneItemsLast(node);
    },
  });
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  try {
    for (const plugin of plugins) {
      warn.mockClear();
      const { html } = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
      expect(html, plugin.name).not.toContain("<p>a</p>");
      expect(warn, plugin.name).toHaveBeenCalledTimes(1);
      const message = warn.mock.calls[0]?.[0] as string;
      expect(message).toContain(`plugin "${plugin.name}" listed 1 mdast node(s)`);
      expect(message).toContain("stays removed");
    }
    warn.mockClear();
    const { html } = markdownToHtml("- [x] b\n- [ ] a\n", { mdastPlugins: [doneItems] });
    expect(html).not.toContain("> b</li>");
    expect(warn).toHaveBeenCalledTimes(1);
  } finally {
    warn.mockRestore();
  }
});

test("hast: a removed child listed again stays removed and the plugin is warned", () => {
  const plugin = defineHastPlugin({
    name: "hast-remove-then-relist",
    element: {
      filter: ["blockquote"],
      visit(node, ctx) {
        const first = node.children.find((child) => child.type === "element");
        if (!first) return;
        ctx.removeNode(first);
        return { ...node, children: [...node.children.filter((child) => child !== first), first] };
      },
    },
  });
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  try {
    const { html } = markdownToHtml("> a\n>\n> b\n", { hastPlugins: [plugin] });
    expect(html).toBe("<blockquote>\n\n<p>b</p>\n</blockquote>\n");
    expect(warn).toHaveBeenCalledTimes(1);
    expect(warn.mock.calls[0]?.[0]).toContain('plugin "hast-remove-then-relist"');
  } finally {
    warn.mockRestore();
  }
});

test("removing a node without listing it again does not warn", () => {
  const passOn = relistAfterRemoving("pass-on-a-copy", (node) => asSection(node));
  const passOnCopiedList = relistAfterRemoving("pass-on-a-copied-list", (node) => ({
    ...node,
    children: [...node.children],
  }));
  const alertFromMarker = defineMdastPlugin({
    name: "alert-from-marker",
    paragraph(node, ctx) {
      if (!isParagraphA(node)) return;
      const quote = ctx.parent(node);
      ctx.removeNode(node);
      ctx.replaceNode(quote, { ...quote, data: { hName: "aside" } });
    },
  });
  const rawSandwich = defineMdastPlugin({
    name: "raw-sandwich-after-removal",
    paragraph(node, ctx) {
      if (!isParagraphA(node)) return;
      const quote = ctx.parent(node);
      ctx.removeNode(node);
      ctx.replaceNode(quote, [{ raw: "<div>" }, ...quote.children, { raw: "</div>" }]);
    },
  });
  const unwrapByInsert = defineMdastPlugin({
    name: "unwrap-by-insert",
    blockquote(node, ctx) {
      ctx.insertBefore(node, node.children);
      ctx.removeNode(node);
    },
    paragraph: removeMdastParagraphA,
  });
  const laterRemoval = defineMdastPlugin({
    name: "reverse-then-remove-child",
    blockquote(node) {
      return { ...node, children: node.children.toReversed() };
    },
    paragraph: removeMdastParagraphA,
  });
  const moveAfterSibling = defineMdastPlugin({
    name: "move-heading-last",
    heading(node, ctx) {
      const last = ctx.parent(node)?.children.at(-1);
      if (!last || last === node) return;
      ctx.insertAfter(last, node);
      ctx.removeNode(node);
    },
  });
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  try {
    const passed = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [passOn] });
    expect(passed.html).toBe("<section>\n<p>b</p>\n</section>\n");
    const reversed = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [laterRemoval] });
    expect(reversed.html).toBe("<blockquote>\n<p>b</p>\n</blockquote>\n");
    const moved = markdownToHtml("# h\n\np\n", { mdastPlugins: [moveAfterSibling] });
    expect(moved.html).toBe("<p>p</p>\n<h1>h</h1>\n");
    const expected = [
      [passOnCopiedList, "<blockquote>\n<p>b</p>\n</blockquote>\n"],
      [alertFromMarker, "<aside>\n<p>b</p>\n</aside>\n"],
      [rawSandwich, "<div>\n<p>b</p>\n</div>\n"],
      [unwrapByInsert, "<p>b</p>\n"],
    ] as const;
    for (const [plugin, html] of expected) {
      expect(markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] }).html, plugin.name).toBe(
        html,
      );
    }
    expect(warn).not.toHaveBeenCalled();
  } finally {
    warn.mockRestore();
  }
});

test("an async visitor's list is checked against the removals made by the time it settles", async () => {
  const removedLater = defineMdastPlugin({
    name: "async-reverse-then-timed-removal",
    async blockquote(node) {
      return { ...node, children: node.children.toReversed() };
    },
    async paragraph(node, ctx) {
      await new Promise((resolve) => setTimeout(resolve, 0));
      removeMdastParagraphA(node, ctx);
    },
  });
  const removedFirst = defineMdastPlugin({
    name: "async-remove-then-relist",
    async blockquote(node, ctx) {
      const [first, ...rest] = node.children;
      if (!first) return;
      ctx.removeNode(first);
      await Promise.resolve();
      return { ...node, children: [...rest, first] };
    },
  });
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  try {
    for (const [plugin, warnings] of [
      [removedLater, 0],
      [removedFirst, 1],
    ] as const) {
      warn.mockClear();
      const { html } = await markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
      expect(html, plugin.name).toBe("<blockquote>\n<p>b</p>\n</blockquote>\n");
      expect(warn, plugin.name).toHaveBeenCalledTimes(warnings);
    }
  } finally {
    warn.mockRestore();
  }
});

test("a node moved out beside its replaced parent or grandparent lands there", () => {
  const beforeParent = defineMdastPlugin({
    name: "move-before-replaced-parent",
    blockquote: asSection,
    paragraph(node, ctx) {
      if (!isParagraphA(node)) return;
      ctx.insertBefore(ctx.parent(node), node);
      ctx.removeNode(node);
    },
  });
  const afterGrandparent = defineMdastPlugin({
    name: "move-after-replaced-grandparent",
    blockquote(node, ctx) {
      if (ctx.parent(node).type === "root") return asSection(node);
    },
    paragraph(node, ctx) {
      const grandparent = ctx.parent(ctx.parent(node));
      if (!isParagraphA(node) || !grandparent) return;
      ctx.removeNode(node);
      ctx.insertAfter(grandparent, node);
    },
  });
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  try {
    expect(markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [beforeParent] }).html).toBe(
      "<p>a</p>\n<section>\n<p>b</p>\n</section>\n",
    );
    expect(markdownToHtml("> > a\n> >\n> > b\n", { mdastPlugins: [afterGrandparent] }).html).toBe(
      "<section>\n<blockquote>\n<p>b</p>\n</blockquote>\n</section>\n<p>a</p>\n",
    );
    expect(warn).not.toHaveBeenCalled();
  } finally {
    warn.mockRestore();
  }
});

test("hast: a node moved out beside its replaced parent lands there", () => {
  const plugin = defineHastPlugin({
    name: "hast-move-before-replaced-parent",
    element: {
      filter: ["blockquote", "p"],
      visit(node, ctx) {
        if (node.tagName === "blockquote") return { ...node, tagName: "section" };
        const [text] = node.children;
        if (text?.type !== "text" || text.value !== "a") return;
        ctx.insertBefore(ctx.parent(node), node);
        ctx.removeNode(node);
      },
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { hastPlugins: [plugin] });
  expect(html).toBe("<p>a</p><section>\n\n<p>b</p>\n</section>\n");
});

function replaceParagraphAWithRule(node: Paragraph, ctx: MdastVisitorContext) {
  if (!isParagraphA(node)) return;
  ctx.insertAfter(node, { type: "thematicBreak" });
  ctx.removeNode(node);
}

test("a children list set on a parent keeps the content queued beside a removed child", () => {
  const onParent = defineMdastPlugin({
    name: "set-quote-children",
    blockquote(node, ctx) {
      ctx.setProperty(node, "children", [...node.children]);
    },
    paragraph: replaceParagraphAWithRule,
  });
  const reordered = defineMdastPlugin({
    name: "set-quote-children-reversed",
    blockquote(node, ctx) {
      ctx.setField(node, "children", node.children.toReversed());
    },
    paragraph: replaceParagraphAWithRule,
  });
  expect(markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [onParent] }).html).toBe(
    "<blockquote>\n<hr>\n<p>b</p>\n</blockquote>\n",
  );
  expect(markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [reordered] }).html).toBe(
    "<blockquote>\n<p>b</p>\n<hr>\n</blockquote>\n",
  );
});

test("hast: a children list set on a parent keeps the content queued beside a removed child", () => {
  const plugin = defineHastPlugin({
    name: "hast-set-quote-children",
    element: {
      filter: ["blockquote", "p"],
      visit(node, ctx) {
        if (node.tagName === "blockquote") {
          ctx.setProperty(node, "children", [...node.children]);
          return;
        }
        const [text] = node.children;
        if (text?.type !== "text" || text.value !== "a") return;
        ctx.insertAfter(node, { type: "element", tagName: "hr", properties: {}, children: [] });
        ctx.removeNode(node);
      },
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { hastPlugins: [plugin] });
  expect(html).toBe("<blockquote>\n<hr>\n<p>b</p>\n</blockquote>\n");
});

test("a removed node stays removed when it is also replaced", () => {
  const copiedByItsVisitor = defineMdastPlugin({
    name: "parent-removes-child-that-copies-itself",
    blockquote(node, ctx) {
      const [first] = node.children;
      if (first) ctx.removeNode(first);
    },
    paragraph(node) {
      return { ...node, data: { hProperties: { className: ["x"] } } };
    },
  });
  const replacedAfter = defineMdastPlugin({
    name: "remove-then-replace",
    paragraph(node, ctx) {
      if (!isParagraphA(node)) return;
      ctx.removeNode(node);
      ctx.replaceNode(node, { type: "thematicBreak" });
    },
  });
  expect(markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [copiedByItsVisitor] }).html).toBe(
    '<blockquote>\n<p class="x">b</p>\n</blockquote>\n',
  );
  expect(markdownToHtml("a\n\nb\n", { mdastPlugins: [replacedAfter] }).html).toBe("<p>b</p>\n");
});

test("hast: a removed node stays removed when its own visitor returns a copy", () => {
  const plugin = defineHastPlugin({
    name: "hast-parent-removes-child-that-copies-itself",
    element: {
      filter: ["blockquote", "p"],
      visit(node, ctx) {
        if (node.tagName === "p") return { ...node, properties: { className: ["x"] } };
        const first = node.children.find((child) => child.type === "element");
        if (first) ctx.removeNode(first);
      },
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { hastPlugins: [plugin] });
  expect(html).toBe('<blockquote>\n\n<p class="x">b</p>\n</blockquote>\n');
});

test("raw items replacing a node inside a paragraph stay inline", () => {
  const plugin = defineMdastPlugin({
    name: "inline-raw-and-text",
    text(node, ctx) {
      if (node.value === "x") ctx.replaceNode(node, [{ raw: "*x*" }, { type: "text", value: "!" }]);
    },
  });
  expect(markdownToHtml("x\n", { mdastPlugins: [plugin] }).html).toBe("<p><em>x</em>!</p>\n");
});

test("a field set after an array replacement does not bring back an earlier replacement", () => {
  const plugin = defineMdastPlugin({
    name: "replace-twice-then-set-field",
    heading(node, ctx) {
      ctx.replaceNode(node, { type: "paragraph", children: [{ type: "text", value: "A" }] });
      ctx.replaceNode(node, [{ type: "thematicBreak" }, { type: "thematicBreak" }]);
      ctx.setField(node, "data", { hProperties: { id: "x" } });
    },
  });
  const { html } = markdownToHtml("# h\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<hr>\n<hr>\n");
});

test("a node replaced by its children between raw items keeps the edits queued around them", () => {
  const plugin = defineMdastPlugin({
    name: "wrap-children-in-raw-html",
    blockquote(node, ctx) {
      ctx.replaceNode(node, [{ raw: "<section>" }, ...node.children, { raw: "</section>" }]);
    },
    paragraph(node, ctx) {
      if (ctx.indexOf(node) === 0) ctx.insertAfter(node, { type: "thematicBreak" });
    },
  });
  const { html } = markdownToHtml("> a\n>\n> b\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<section>\n<p>a</p>\n<hr>\n<p>b</p>\n</section>\n");
});

test("unwrapping a single-child node keeps the edits queued around that child", () => {
  const plugin = defineMdastPlugin({
    name: "unwrap-single-child-quote",
    blockquote(node, ctx) {
      ctx.replaceNode(node, node.children);
    },
    paragraph(node, ctx) {
      ctx.insertAfter(node, { type: "thematicBreak" });
    },
  });
  const { html } = markdownToHtml("> a\n", { mdastPlugins: [plugin] });
  expect(html).toBe("<p>a</p>\n<hr>\n");
});

test("mixed raw replace arrays preserve references to reused nodes", () => {
  const plugin = defineMdastPlugin({
    name: "mixed-raw-reuse",
    blockquote(node, ctx) {
      const index = ctx.indexOf(node);
      if (index === 0) {
        ctx.setProperty(node, "data", { hName: "section" });
      } else if (index === 1) {
        const first = ctx.parent(node).children[0]!;
        ctx.replaceNode(node, [first, { raw: "tail" }]);
      }
    },
  });
  const { html } = markdownToHtml(twoQuotes, { mdastPlugins: [plugin] });
  expect((html.match(/<section>/g) ?? []).length).toBe(2);
  expect(html).toContain("<p>tail</p>");
});

test("root self-replacement preserves queued mdast edits", () => {
  const plugin = defineMdastPlugin({
    name: "mdast-root-self-replacement",
    before(root, ctx) {
      ctx.setProperty(root, "children", [
        { type: "paragraph", children: [{ type: "text", value: "changed" }] },
      ]);
      ctx.replaceNode(root, root);
    },
  });
  expect(markdownToHtml("# old\n", { mdastPlugins: [plugin] }).html).toBe("<p>changed</p>\n");
});

test("root self-replacement preserves queued hast edits", () => {
  const plugin = defineHastPlugin({
    name: "hast-root-self-replacement",
    before(root, ctx) {
      ctx.setProperty(root, "children", [
        {
          type: "element",
          tagName: "p",
          properties: {},
          children: [{ type: "text", value: "changed" }],
        },
      ]);
      ctx.replaceNode(root, root);
    },
  });
  expect(markdownToHtml("# old\n", { hastPlugins: [plugin] }).html).toBe("<p>changed</p>\n");
});

test("replaceNode reuses a node the same way whatever form it is passed in", () => {
  const forms = {
    scalar: (ctx: never, a: never, c: never) =>
      (ctx as never as { replaceNode: Function }).replaceNode(c, a),
    array1: (ctx: never, a: never, c: never) =>
      (ctx as never as { replaceNode: Function }).replaceNode(c, [a]),
    array2: (ctx: never, a: never, c: never) =>
      (ctx as never as { replaceNode: Function }).replaceNode(c, [
        { type: "paragraph", children: [{ type: "text", value: "X" }] },
        a,
      ]),
  };
  for (const form of Object.values(forms)) {
    let done = false;
    const plugin = defineMdastPlugin({
      name: "replace-forms",
      text(node) {
        if (node.value === "a") return { type: "text", value: "SEEN" };
      },
      blockquote(node, ctx) {
        if (done) return;
        done = true;
        const siblings = ctx.parent(node).children;
        form(ctx as never, siblings[0] as never, siblings[siblings.length - 1] as never);
      },
    });
    const { html } = markdownToHtml(twoQuotes, { mdastPlugins: [plugin] });
    expect((html.match(/SEEN/g) ?? []).length).toBe(2);
  }
});
