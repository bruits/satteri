import { test, expect } from "vitest";
import { markdownToHtml, mdxToJs, defineMdastPlugin, defineHastPlugin } from "../src/index.js";

// The native pool caps at 4 arenas per thread, so a few rounds cycle every compile through recycled arenas; missed reset state shows as drift vs round 0.

const big =
  "# Big\n\n" +
  "Some *rich* text with [a link](https://x.y) and `code`.\n\n".repeat(50) +
  "| a | b |\n|---|---|\n| 1 | 2 |\n";
const small = "tiny\n";
const mathDoc = "# M\n\nInline $x^2$ and\n\n$$\ny = mx\n$$\n";
const fmDoc = "---\ntitle: FM\n---\n\ncontent [ref][r]\n\n[r]: https://ref.example\n";
const mdxDoc = "# MDX\n\n<Widget a={1} />\n";

const dataSetter = defineMdastPlugin({
  name: "data-setter",
  heading(node, ctx) {
    ctx.setProperty(node, "data", { marker: "from-data-setter" });
  },
});
const linkSpans = defineHastPlugin({
  name: "link-spans",
  element: {
    filter: ["a"],
    visit(node) {
      return { type: "element", tagName: "span", properties: {}, children: node.children };
    },
  },
});

type Snapshot = { out: string; fm: unknown };

function sync<T>(result: T | Promise<T>): T {
  if (result instanceof Promise) throw new Error("expected sync");
  return result;
}

const shapes: (() => Snapshot)[] = [
  () => {
    const r = sync(markdownToHtml(big, { mdastPlugins: [dataSetter] }));
    return { out: r.html, fm: r.frontmatter };
  },
  () => {
    const r = sync(markdownToHtml(small));
    return { out: r.html, fm: r.frontmatter };
  },
  () => {
    const r = sync(markdownToHtml(mathDoc, { features: { math: true }, hastPlugins: [linkSpans] }));
    return { out: r.html, fm: r.frontmatter };
  },
  () => {
    const r = sync(markdownToHtml(mathDoc, { features: { math: false } }));
    return { out: r.html, fm: r.frontmatter };
  },
  () => {
    const r = sync(markdownToHtml(fmDoc, { hastPlugins: [linkSpans] }));
    return { out: r.html, fm: r.frontmatter };
  },
  () => {
    const r = sync(mdxToJs(mdxDoc, { hastPlugins: [linkSpans] }));
    return { out: r.code, fm: r.frontmatter };
  },
  () => {
    const r = sync(markdownToHtml(small, { mdastPlugins: [dataSetter], hastPlugins: [linkSpans] }));
    return { out: r.html, fm: r.frontmatter };
  },
];

test("pooled arenas never leak state across compiles (mixed workload drift check)", () => {
  const first = shapes.map((run) => run());
  for (let round = 1; round < 12; round++) {
    for (let s = 0; s < shapes.length; s++) {
      const shape = shapes[s];
      if (shape === undefined) continue;
      expect(shape(), `round ${round}, shape ${s}`).toEqual(first[s]);
    }
  }
});
