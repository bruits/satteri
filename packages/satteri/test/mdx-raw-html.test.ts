import { describe, test, expect } from "vitest";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkMdx from "remark-mdx";
import remarkRehype from "remark-rehype";
import rehypeRaw from "rehype-raw";
import rehypeStringify from "rehype-stringify";
import type { Nodes as MdastNodes } from "mdast";
import { pathToFileURL } from "node:url";
import { mdxToHast, markdownToHast } from "../src/index.js";
import type { HastNode } from "../src/hast/hast-materializer.js";

/**
 * Conformance probe for `rawHtml` (the `rehype-raw` equivalent) combined with
 * MDX — i.e. `mdxToHast(src, { features: { rawHtml: true } })`.
 *
 * The headline result of this file: **MDX + `rehype-raw` has no upstream
 * reference behavior to conform to.** In MDX, HTML-looking syntax is parsed as
 * JSX (`mdxJsxFlowElement` / `mdxFlowExpression`), not as `raw` nodes, so:
 *
 *  1. The reference unified pipeline (remark-mdx → remark-rehype → rehype-raw
 *     → rehype-stringify) throws when it reaches an MDX node — `rehype-raw`
 *     passes MDX nodes through untouched and `rehype-stringify` then refuses to
 *     serialize them (`Cannot compile unknown node 'mdxJsxFlowElement'`).
 *  2. Sätteri's raw reparse (`raw_to_hast_arena`) renders the tree back to HTML
 *     before reparsing, and MDX nodes have no HTML representation, so the whole
 *     MDX subtree is silently dropped.
 *
 * The contract (per the maintainer, see REVIEW-rawHtml-mdx.md): MDX + `rawHtml`
 * must WORK — the reparse preserves MDX nodes (real passthrough, mirroring
 * `hast-util-raw`'s `passThrough`), rather than gating `rawHtml` off for MDX.
 * Sätteri implements this by serialising each MDX node as a placeholder comment,
 * reparsing, then swapping the original subtree back in. For the MDX *parse*
 * path the observable result is that the tree is unchanged, because `mdxToHast`
 * never emits `raw` nodes (verified below), so passthrough finds nothing to
 * reparse. Passthrough (not gating) matters when `raw` nodes are *injected* into
 * an MDX-flagged tree via the plugin/command API — the scenario PR #160 covers.
 */

const MDX_PASS_THROUGH: Array<MdastNodes["type"]> = [
  "mdxJsxFlowElement",
  "mdxJsxTextElement",
  "mdxFlowExpression",
  "mdxTextExpression",
  "mdxjsEsm",
];

const { remarkMarkAndUnravel } = await import(
  pathToFileURL("node_modules/@mdx-js/mdx/lib/plugin/remark-mark-and-unravel.js").href
);

// remark-mdx → remark-rehype (mdx nodes passed through) → rehype-raw
// (mdx nodes passed through the raw reparse) → rehype-stringify.
const reference = unified()
  .use(remarkParse)
  .use(remarkMdx)
  .use(remarkMarkAndUnravel)
  .use(remarkRehype, { allowDangerousHtml: true, passThrough: MDX_PASS_THROUGH })
  .use(rehypeRaw, { passThrough: MDX_PASS_THROUGH as never })
  .use(rehypeStringify, { allowDangerousHtml: true });

const referenceRun = (md: string): string => reference.processSync(md).toString();

/** Strip positions so trees compare on structure/values alone. */
function clean(node: HastNode): unknown {
  const n = node as unknown as Record<string, unknown>;
  const out: Record<string, unknown> = { type: n.type };
  for (const k of ["tagName", "name", "value", "properties", "attributes"]) {
    if (k in n) out[k] = n[k];
  }
  if (Array.isArray(n.children)) {
    out.children = (n.children as HastNode[]).map(clean);
  }
  return out;
}

// Inputs that MDX parses into JSX/expression nodes (not `raw` nodes).
const cases: Array<{ name: string; md: string }> = [
  { name: "jsx flow element", md: `<Foo bar={1} />\n` },
  { name: "flow expression", md: `{1 + 1}\n` },
  {
    name: "html-looking block wrapping markdown",
    md: `<div class="note">\n\ntext **bold**\n\n</div>`,
  },
  { name: "heading then jsx", md: `# Hi\n\n<Foo />\n` },
];

describe("mdx + rawHtml (rehype-raw) conformance", () => {
  describe("the reference ecosystem cannot serialize MDX through rehype-raw", () => {
    for (const { name, md } of cases) {
      test(name, () => {
        // rehype-raw leaves the passed-through MDX nodes in the tree;
        // rehype-stringify then throws because it can't compile them.
        expect(() => referenceRun(md)).toThrow(/unknown node/i);
      });
    }
  });

  // Why gating vs. passthrough is behaviorally identical for the parse path:
  // MDX turns all HTML-looking syntax into JSX (or a hard parse error), so the
  // MDX parse path never yields `raw` nodes for the reparse to act on.
  describe("the MDX parse path never emits raw nodes", () => {
    const hasRaw = (n: HastNode): boolean =>
      n.type === "raw" ||
      ("children" in n && Array.isArray(n.children) && (n.children as HastNode[]).some(hasRaw));

    for (const md of [`<div>x</div>`, `text <span>y</span>`, `<Foo/>\n`]) {
      test(JSON.stringify(md), () => {
        // markdownToHast keeps these as raw; mdxToHast parses them as JSX.
        expect(hasRaw(markdownToHast(md))).toBe(true);
        expect(hasRaw(mdxToHast(md))).toBe(false);
      });
    }
  });

  describe("MDX + rawHtml preserves MDX nodes (passthrough)", () => {
    for (const { name, md } of cases) {
      // `rawHtml: true` serialises MDX nodes as placeholder comments and swaps
      // them back after the reparse, so a pure-MDX tree round-trips unchanged.
      test(name, () => {
        const withRaw = mdxToHast(md, { features: { rawHtml: true } });
        const withoutRaw = mdxToHast(md);
        expect(clean(withRaw)).toEqual(clean(withoutRaw));
      });
    }
  });
});
