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

// Pass MDX nodes through both reference transforms so raw-HTML reparsing preserves them.

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

const reference = unified()
  .use(remarkParse)
  .use(remarkMdx)
  .use(remarkMarkAndUnravel)
  .use(remarkRehype, { allowDangerousHtml: true, passThrough: MDX_PASS_THROUGH })
  .use(rehypeRaw, { passThrough: MDX_PASS_THROUGH as never })
  .use(rehypeStringify, { allowDangerousHtml: true });

const referenceRun = (md: string): string => reference.processSync(md).toString();

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
        // rehype-stringify cannot render the MDX nodes preserved by rehype-raw.
        expect(() => referenceRun(md)).toThrow(/unknown node/i);
      });
    }
  });

  describe("the MDX parse path never emits raw nodes", () => {
    const hasRaw = (n: HastNode): boolean =>
      n.type === "raw" ||
      ("children" in n && Array.isArray(n.children) && (n.children as HastNode[]).some(hasRaw));

    for (const md of [`<div>x</div>`, `text <span>y</span>`, `<Foo/>\n`]) {
      test(JSON.stringify(md), () => {
        expect(hasRaw(markdownToHast(md))).toBe(true);
        expect(hasRaw(mdxToHast(md))).toBe(false);
      });
    }
  });

  describe("MDX + rawHtml preserves MDX nodes (passthrough)", () => {
    for (const { name, md } of cases) {
      test(name, () => {
        const withRaw = mdxToHast(md, { features: { rawHtml: true } });
        const withoutRaw = mdxToHast(md);
        expect(clean(withRaw)).toEqual(clean(withoutRaw));
      });
    }
  });
});
