import { test, expect, vi } from "vitest";
import { markdownToHtml, defineMdastPlugin, defineHastPlugin } from "../src/index.js";
import type { MdastNode } from "../src/types.js";
import type { HastNode } from "../src/hast/hast-materializer.js";

const variants = new Set(["note", "tip", "caution"]);

function asideTransform(node: { name: string; children: MdastNode[] }): MdastNode {
  return {
    type: "paragraph",
    data: { hName: "aside", hProperties: { "data-v": node.name } },
    children: [...node.children],
  } as unknown as MdastNode;
}

const nestedDirectives = "::::note\nouter\n\n:::tip\ninner\n:::\n::::";
const features = { directive: true, gfm: false } as const;

test.each(["mdast", "hast"] as const)(
  "%s async replacements keep their targets when they settle in reverse order",
  async (phase) => {
    const complete: (() => void)[] = [];
    const plugin = {
      name: "reverse-completion",
      text(node: { value: string }) {
        if (node.value.trim() === "") return;
        return new Promise<{ type: "text"; value: string }>((resolve) => {
          complete.push(() => resolve({ type: "text", value: node.value.toUpperCase() }));
        });
      },
    };
    const result = markdownToHtml(
      "one\n\ntwo\n\nthree",
      phase === "mdast" ? { mdastPlugins: [plugin] } : { hastPlugins: [plugin] },
    );
    expect(result).toBeInstanceOf(Promise);
    expect(complete).toHaveLength(3);
    for (const resolve of complete.reverse()) resolve();
    expect((await result).html).toBe("<p>ONE</p>\n<p>TWO</p>\n<p>THREE</p>\n");
  },
);

test("nested transforms compose in one pass, including across an async visitor", async () => {
  const plugin = defineMdastPlugin({
    name: "async-aside",
    async containerDirective(node) {
      await Promise.resolve();
      if (!variants.has(node.name)) return;
      return asideTransform(node);
    },
  });
  const { html } = await markdownToHtml(nestedDirectives, { features, mdastPlugins: [plugin] });
  expect((html.match(/<aside/g) ?? []).length).toBe(2);
  expect(html).toContain('data-v="note"');
  expect(html).toContain('data-v="tip"');
});

test("a transform stranded under a removed node is dropped, not fatal", () => {
  const plugin = defineMdastPlugin({
    name: "remove-outer",
    containerDirective(node, ctx) {
      if (node.name === "note") {
        ctx.removeNode(node);
        return;
      }
      if (node.name === "tip") {
        return { type: "paragraph", children: [{ type: "text", value: "TIP" }] } as MdastNode;
      }
    },
  });
  const { html } = markdownToHtml(nestedDirectives, { features, mdastPlugins: [plugin] });
  expect(html).not.toContain("TIP");
  expect(html).not.toContain("outer");
  expect(html.trim()).toBe("");
});

test("dropping a stranded transform warns, naming the plugin", () => {
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  try {
    const plugin = defineMdastPlugin({
      name: "remove-outer",
      containerDirective(node, ctx) {
        if (node.name === "note") {
          ctx.removeNode(node);
          return;
        }
        if (node.name === "tip") {
          return { type: "paragraph", children: [{ type: "text", value: "TIP" }] } as MdastNode;
        }
      },
    });
    markdownToHtml(nestedDirectives, { features, mdastPlugins: [plugin] });
    expect(warn).toHaveBeenCalledTimes(1);
    const message = warn.mock.calls[0]?.[0] as string;
    expect(message).toContain('plugin "remove-outer"');
    expect(message).toContain("dropped");
  } finally {
    warn.mockRestore();
  }
});

test("a stranded HAST transform is dropped with a warning, like MDAST", () => {
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  try {
    const plugin = defineHastPlugin({
      name: "remove-heading",
      element: {
        filter: ["h1", "em"],
        visit(node, ctx) {
          if (node.tagName === "h1") {
            ctx.removeNode(node);
            return;
          }
          if (node.tagName === "em") {
            return {
              type: "element",
              tagName: "strong",
              properties: {},
              children: node.children,
            } as unknown as HastNode;
          }
        },
      },
    });
    const { html } = markdownToHtml("# *Hi*", { hastPlugins: [plugin] });
    expect(html.trim()).toBe("");
    expect(warn).toHaveBeenCalledTimes(1);
    const message = warn.mock.calls[0]?.[0] as string;
    expect(message).toContain('plugin "remove-heading"');
    expect(message).toContain("hast");
    expect(message).toContain("dropped");
  } finally {
    warn.mockRestore();
  }
});

test("an array replaceNode strands a transform queued under the discarded target", () => {
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  try {
    const plugin = defineHastPlugin({
      name: "split-heading",
      element: {
        filter: ["h1", "em"],
        visit(node, ctx) {
          if (node.tagName === "h1") {
            ctx.replaceNode(node, [
              { type: "element", tagName: "h2", properties: {}, children: [] },
              { type: "element", tagName: "hr", properties: {}, children: [] },
            ]);
            return;
          }
          if (node.tagName === "em") {
            return { type: "element", tagName: "strong", properties: {}, children: node.children };
          }
        },
      },
    });
    const { html } = markdownToHtml("# *Hi*", { hastPlugins: [plugin] });
    expect(html.trim()).toBe("<h2></h2><hr>");
    expect(warn).toHaveBeenCalledTimes(1);
    expect(warn.mock.calls[0]?.[0]).toContain("dropped");
  } finally {
    warn.mockRestore();
  }
});

test("an array replaceNode that passes children through keeps their queued transforms", () => {
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  try {
    const plugin = defineHastPlugin({
      name: "split-heading-keep",
      element: {
        filter: ["h1", "em"],
        visit(node, ctx) {
          if (node.tagName === "h1") {
            ctx.replaceNode(node, [
              { type: "element", tagName: "hr", properties: {}, children: [] },
              { type: "element", tagName: "h2", properties: {}, children: node.children },
            ]);
            return;
          }
          if (node.tagName === "em") {
            return { type: "element", tagName: "strong", properties: {}, children: node.children };
          }
        },
      },
    });
    const { html } = markdownToHtml("# *Hi*", { hastPlugins: [plugin] });
    expect(html.trim()).toBe("<hr><h2><strong>Hi</strong></h2>");
    expect(warn).not.toHaveBeenCalled();
  } finally {
    warn.mockRestore();
  }
});

test("a passed-through child is fully transformed before the next plugin runs", () => {
  const aside = defineMdastPlugin({
    name: "aside",
    containerDirective(node) {
      if (variants.has(node.name)) return asideTransform(node);
    },
  });
  const upper = defineMdastPlugin({
    name: "upper",
    text(node) {
      return { type: "text", value: node.value.toUpperCase() } as MdastNode;
    },
  });
  const { html } = markdownToHtml(nestedDirectives, { features, mdastPlugins: [aside, upper] });
  expect((html.match(/<aside/g) ?? []).length).toBe(2);
  expect(html).toContain("OUTER");
  expect(html).toContain("INNER");
});

test("a plugin's own freshly-built node is not re-walked", () => {
  let calls = 0;
  const wrap = defineMdastPlugin({
    name: "wrap-once",
    blockquote() {
      calls++;
      return {
        type: "blockquote",
        data: { hProperties: { "data-wrapped": "1" } },
        children: [{ type: "paragraph", children: [{ type: "text", value: "x" }] }],
      } as MdastNode;
    },
  });
  const { html } = markdownToHtml("> a", { features: { gfm: false }, mdastPlugins: [wrap] });
  expect(calls).toBe(1);
  expect((html.match(/data-wrapped/g) ?? []).length).toBe(1);
});

test("a table moved out of a directive keeps its cells and alignment", () => {
  const move = defineMdastPlugin({
    name: "move-table",
    containerDirective(node, ctx) {
      ctx.insertAfter(node, node.children[1]!);
    },
  });
  const md = ":::tip\nintro\n\n| A | B | C |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n\n:::";
  const { html } = markdownToHtml(md, {
    features: { directive: true, gfm: true },
    mdastPlugins: [move],
  });
  expect(html).toContain('<td style="text-align: left">1</td>');
  expect(html).toContain('<td style="text-align: center">2</td>');
  expect(html).toContain('<td style="text-align: right">3</td>');
});

test("an inlineMath node survives a round-trip", () => {
  const dup = defineMdastPlugin({
    name: "dup-inline-math",
    inlineMath(node, ctx) {
      ctx.insertAfter(node, node);
    },
  });
  const { html } = markdownToHtml("hi $x$ end", { features: { math: true }, mdastPlugins: [dup] });
  expect((html.match(/math-inline">x</g) ?? []).length).toBe(2);
});

test("an imageReference keeps its alt through a round-trip", () => {
  const dup = defineMdastPlugin({
    name: "dup-image-ref",
    imageReference(node, ctx) {
      ctx.insertAfter(node, node);
    },
  });
  const md = '![alt text][logo]\n\n[logo]: /logo.png "Logo"';
  const { html } = markdownToHtml(md, { mdastPlugins: [dup] });
  expect((html.match(/alt="alt text"/g) ?? []).length).toBe(2);
});

test("a fresh table built without `align` still renders its cells", () => {
  const build = defineMdastPlugin({
    name: "build-table",
    paragraph() {
      return {
        type: "table",
        children: [
          {
            type: "tableRow",
            children: [{ type: "tableCell", children: [{ type: "text", value: "H" }] }],
          },
          {
            type: "tableRow",
            children: [{ type: "tableCell", children: [{ type: "text", value: "v" }] }],
          },
        ],
      } as unknown as MdastNode;
    },
  });
  const { html } = markdownToHtml("x", { features: { gfm: true }, mdastPlugins: [build] });
  expect(html).toContain("<th>H</th>");
  expect(html).toContain("<td>v</td>");
});

test("a freshly-generated node is transformed by a later plugin (the multi-plugin path)", () => {
  const emit = defineMdastPlugin({
    name: "emit-tip",
    containerDirective(node) {
      if (node.name !== "note") return;
      return {
        type: "containerDirective",
        name: "tip",
        children: [{ type: "paragraph", children: [{ type: "text", value: "generated" }] }],
      } as unknown as MdastNode;
    },
  });
  const toAside = defineMdastPlugin({
    name: "tip-to-aside",
    containerDirective(node) {
      if (node.name === "tip") return asideTransform(node);
    },
  });
  const md = ":::note\nx\n:::";
  const emitOnly = markdownToHtml(md, { features, mdastPlugins: [emit] }).html;
  expect(emitOnly).not.toContain("<aside");

  const both = markdownToHtml(md, { features, mdastPlugins: [emit, toAside] }).html;
  expect((both.match(/<aside/g) ?? []).length).toBe(1);
  expect(both).toContain('data-v="tip"');
});

const removedHeadingDoc = "# Head\n\nKept\n";

function removeHeadingKeeping(seen: { node?: MdastNode }) {
  return defineMdastPlugin({
    name: "remove-heading",
    heading(node, ctx) {
      seen.node = node;
      ctx.removeNode(node);
    },
  });
}

test("an insertAfter on a node another plugin removed is dropped with a warning", () => {
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  try {
    const seen: { node?: MdastNode } = {};
    const insert = defineMdastPlugin({
      name: "insert-after-heading",
      paragraph(_node, ctx) {
        if (seen.node) ctx.insertAfter(seen.node, { type: "thematicBreak" } satisfies MdastNode);
      },
    });
    const { html } = markdownToHtml(removedHeadingDoc, {
      mdastPlugins: [removeHeadingKeeping(seen), insert],
    });
    expect(html.trim()).toBe("<p>Kept</p>");
    expect(warn).toHaveBeenCalledTimes(1);
    const message = warn.mock.calls[0]?.[0] as string;
    expect(message).toContain('plugin "insert-after-heading"');
    expect(message).toContain("dropped");
  } finally {
    warn.mockRestore();
  }
});

test("a replaceNode on a node another plugin removed is dropped with a warning", () => {
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  try {
    const seen: { node?: MdastNode } = {};
    const replace = defineMdastPlugin({
      name: "replace-heading",
      paragraph(_node, ctx) {
        if (seen.node) ctx.replaceNode(seen.node, { type: "thematicBreak" } satisfies MdastNode);
      },
    });
    const { html } = markdownToHtml(removedHeadingDoc, {
      mdastPlugins: [removeHeadingKeeping(seen), replace],
    });
    expect(html.trim()).toBe("<p>Kept</p>");
    expect(warn).toHaveBeenCalledTimes(1);
    const message = warn.mock.calls[0]?.[0] as string;
    expect(message).toContain('plugin "replace-heading"');
    expect(message).toContain("dropped");
  } finally {
    warn.mockRestore();
  }
});
