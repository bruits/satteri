import { test, expect } from "vitest";
import { markdownToHtml, defineMdastPlugin } from "../src/index.js";
import type { MdastNode } from "../src/types.js";

// Exercises the mdast plugin re-visit fixpoint in `runMdastPluginsOnHandle`:
// when applying a pass drops patches (a transform subsumed a descendant
// transform), the plugin is re-visited so the descendant gets its turn. These
// cover the parts the nested-aside conformance tests don't: the loop cap, the
// async path, clean termination on an unrecoverable drop, and plugin ordering.

const variants = new Set(["note", "tip", "caution"]);

/** Turn a directive into a paragraph that renders as `<aside>`, re-parenting
 *  its children so a nested directive is re-matched on the next pass. */
function asideTransform(node: { name: string; children: MdastNode[] }): MdastNode {
  return {
    type: "paragraph",
    data: { hName: "aside", hProperties: { "data-v": node.name } },
    children: [...node.children],
  } as unknown as MdastNode;
}

const nestedDirectives = "::::note\nouter\n\n:::tip\ninner\n:::\n::::";
const features = { directive: true, gfm: false } as const;

test("re-visits across an async visitor so nested transforms still compose", async () => {
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

test("a transform stranded under a removed node terminates cleanly, not by throwing", () => {
  // Removing the outer note drops the (now-orphaned) tip transform. This used
  // to throw `patch targets node N inside a removed subtree`; now it's a quiet
  // drop and the pass converges with nothing left to do.
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

test("a nesting plugin re-visits fully before the next plugin runs", () => {
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
  // Both asides formed (the re-visit ran), and `upper` saw the finished tree.
  expect((html.match(/<aside/g) ?? []).length).toBe(2);
  expect(html).toContain("OUTER");
  expect(html).toContain("INNER");
});

test("a non-converging plugin stops at the re-visit cap instead of looping forever", () => {
  // Every blockquote is replaced with a blockquote nesting another, so a matched
  // descendant is stranded on every pass and the count never reaches zero. The
  // cap must stop it; without the cap this would loop until it timed out.
  let calls = 0;
  const plugin = defineMdastPlugin({
    name: "bq-bomb",
    blockquote() {
      calls++;
      return {
        type: "blockquote",
        children: [
          {
            type: "blockquote",
            children: [{ type: "paragraph", children: [{ type: "text", value: "x" }] }],
          },
        ],
      } as MdastNode;
    },
  });
  const { html } = markdownToHtml("> > x", { features: { gfm: false }, mdastPlugins: [plugin] });
  // It terminated, and re-visited well past a single pass (proving the cap, not
  // early convergence, is what stopped it).
  expect(html).toContain("<blockquote>");
  expect(calls).toBeGreaterThan(16);
});
