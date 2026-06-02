import { test, expect, vi } from "vitest";
import { markdownToHtml, defineMdastPlugin } from "../src/index.js";
import type { MdastNode } from "../src/types.js";

// Nested transforms compose in a SINGLE pass: when a plugin replaces a node and
// passes its children through, those children keep their identity, so a patch
// the same pass queued on a nested one still applies. A plugin's own freshly
// *built* nodes are not re-walked — transform them up front, or hand off to a
// later plugin, which sees the materialized tree.

const variants = new Set(["note", "tip", "caution"]);

/** Replace a directive with an `<aside>`-rendering paragraph, passing its
 *  children through so a nested directive among them is still visited. */
function asideTransform(node: { name: string; children: MdastNode[] }): MdastNode {
  return {
    type: "paragraph",
    data: { hName: "aside", hProperties: { "data-v": node.name } },
    children: [...node.children],
  } as unknown as MdastNode;
}

const nestedDirectives = "::::note\nouter\n\n:::tip\ninner\n:::\n::::";
const features = { directive: true, gfm: false } as const;

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
  // Removing the outer note discards the tip transform queued in the same pass:
  // the plugin chose to drop that subtree, so the tip transform is moot. Quiet
  // drop, not an error.
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
  expect(html).not.toContain("TIP"); // the stranded tip transform was dropped
  expect(html).not.toContain("outer"); // the whole note subtree is gone
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
  // Both asides formed (nesting composed in one pass), and `upper` saw the
  // finished tree.
  expect((html.match(/<aside/g) ?? []).length).toBe(2);
  expect(html).toContain("OUTER");
  expect(html).toContain("INNER");
});

test("a plugin's own freshly-built node is not re-walked", () => {
  // Each blockquote is replaced with a blockquote nesting a *fresh* one. The
  // fresh inner blockquote is brand-new (not passed through), so it is not
  // re-entered: one pass, terminates, exactly one wrap.
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
  expect(calls).toBe(1); // the single original blockquote, visited once
  expect((html.match(/data-wrapped/g) ?? []).length).toBe(1); // output not re-wrapped
});

test("a freshly-generated node is transformed by a later plugin (the multi-plugin path)", () => {
  // `emit` produces a NEW :::tip directive; it is not re-walked within `emit`.
  // `toAside`, running afterward over the materialized tree, transforms it.
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
  expect(emitOnly).not.toContain("<aside"); // generated tip not re-walked by emit

  const both = markdownToHtml(md, { features, mdastPlugins: [emit, toAside] }).html;
  expect((both.match(/<aside/g) ?? []).length).toBe(1);
  expect(both).toContain('data-v="tip"');
});
