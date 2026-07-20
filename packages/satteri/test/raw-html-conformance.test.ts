import { describe, test, expect } from "vitest";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkRehype from "remark-rehype";
import rehypeRaw from "rehype-raw";
import rehypeStringify from "rehype-stringify";
import { markdownToHast } from "../src/index.js";
import type { HastNode } from "../src/hast/hast-materializer.js";

/**
 * Conformance suite for the `rawHtml` feature (the `rehype-raw` equivalent).
 *
 * Each input is run through both:
 *  - Sätteri: `markdownToHast(md, { features: { rawHtml: true } })`
 *  - unified: remark-parse → remark-rehype (allowDangerousHtml) → rehype-raw
 *
 * and compared two ways — serialized HTML (via rehype-stringify) and the hast
 * tree itself (structure + normalized properties, positions stripped). The
 * inputs are chosen so Sätteri's baseline Markdown→hast already matches
 * remark-rehype's; the feature under test is the raw-HTML reparsing.
 */

const reference = unified()
  .use(remarkParse)
  .use(remarkRehype, { allowDangerousHtml: true })
  .use(rehypeRaw)
  .use(rehypeStringify);

const referenceTree = (md: string): HastNode =>
  reference.runSync(reference.parse(md)) as unknown as HastNode;

const stringify = (tree: HastNode): string =>
  unified()
    .use(rehypeStringify)
    .stringify(tree as never);

/** Keep only structural fields so trees compare regardless of positions/internals. */
function clean(node: HastNode): unknown {
  const out: Record<string, unknown> = { type: node.type };
  if (node.type === "element") {
    out.tagName = node.tagName;
    out.properties = { ...node.properties };
  }
  if ((node.type === "text" || node.type === "comment" || node.type === "raw") && "value" in node) {
    out.value = node.value;
  }
  if ("children" in node && node.children) {
    out.children = (node.children as HastNode[]).map(clean);
  }
  return out;
}

const cases: Array<{ name: string; md: string }> = [
  { name: "block element wrapping markdown", md: `<div class="note">\n\ntext **bold**\n\n</div>` },
  { name: "inline html", md: `A <span id="s">x</span> and text` },
  {
    name: "normalized attributes",
    md: `<img src="a.png" width="10" disabled class="a b">`,
  },
  { name: "data + aria attributes", md: `<div data-foo-bar="1" aria-label="x"></div>` },
  { name: "comment", md: `<div><!--note--></div>` },
  { name: "tag split across raw blocks", md: `<div class="wrap">\n\n# Heading\n\n</div>` },
  { name: "misnested tags (adoption agency)", md: `<b>1<p>2</b>3</p>` },
  { name: "table with implied tbody", md: `<table><tr><td>y</td></tr></table>` },
  { name: "void + boolean attrs", md: `<input type="checkbox" checked>` },
  { name: "heading then raw", md: `# Hi\n\n<p class="x">para</p>` },
];

describe("rawHtml conformance vs rehype-raw", () => {
  for (const { name, md } of cases) {
    test(`serialized HTML matches: ${name}`, () => {
      const ours = markdownToHast(md, { features: { rawHtml: true } });
      expect(stringify(ours)).toBe(reference.stringify(referenceTree(md)));
    });

    test(`hast tree matches: ${name}`, () => {
      const ours = markdownToHast(md, { features: { rawHtml: true } });
      expect(clean(ours)).toEqual(clean(referenceTree(md)));
    });
  }
});
