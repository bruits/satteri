import { describe, expect, test } from "vitest";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkBreaks from "remark-breaks";
import remarkRehype from "remark-rehype";
import rehypeStringify from "rehype-stringify";
import { markdownToHtml, markdownToMdast } from "../../src/index.js";
import type { Features } from "../../src/index.js";
import { assertMdxPluginConformance, stripPositions } from "./helpers.js";

const FEATURES = { breaks: true, frontmatter: false } satisfies Features;

const referenceProcessor = unified().use(remarkParse).use(remarkGfm).use(remarkBreaks);
const referenceHtmlProcessor = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkBreaks)
  .use(remarkRehype, { allowDangerousHtml: true })
  .use(rehypeStringify, { allowDangerousHtml: true });

// remark-breaks gives its breaks and the text around them no position, so compare without.
function referenceMdast(md: string): unknown {
  return stripPositions(
    JSON.parse(JSON.stringify(referenceProcessor.runSync(referenceProcessor.parse(md)))),
  );
}

function satteriMdast(md: string, features: Features = FEATURES): unknown {
  return stripPositions(JSON.parse(JSON.stringify(markdownToMdast(md, { features }))));
}

function paragraphRoot(children: unknown[]): unknown {
  return { type: "root", children: [{ type: "paragraph", children }] };
}

function normalizeVoidTags(html: string): string {
  return html.replaceAll("<br>", "<br />").replaceAll("<hr>", "<hr />").trim();
}

const CASES: Array<[string, string]> = [
  ["paragraph lines", "a\nb\nc"],
  ["trailing whitespace before the line ending", "a \nb\t\nc"],
  ["an existing hard break stays one break", "a  \nb\\\nc"],
  ["indented continuation line", "a\n   b"],
  ["CRLF and CR line endings", "a\r\nb\rc"],
  ["blank lines still separate paragraphs", "a\n\nb"],
  ["setext heading", "a\nb\n==="],
  ["emphasis and link text", "*a\nb* [c\nd](u)"],
  ["blockquote and list item", "> a\n> b\n\n- c\n  d"],
  ["lazy continuation", "> a\nb"],
  ["footnote definition", "x[^1]\n\n[^1]: a\n    b"],
  ["autolink literal on its own line", "see\nwww.example.com\nnow"],
  ["image alt keeps the line ending", "![a\nb](u)"],
  ["inline code keeps the line ending", "`a\nb`"],
  ["inline html", "<span>a\nb</span>\nc"],
  ["html block", "<div>\na\nb\n</div>"],
  ["fenced code", "```\na\nb\n```"],
];

describe("breaks: conformance vs remark-breaks", () => {
  describe("mdast matches the reference", () => {
    for (const [name, md] of CASES) {
      test(name, () => {
        expect(satteriMdast(md)).toEqual(referenceMdast(md));
      });
    }
  });

  describe("html matches the reference", () => {
    for (const [name, md] of CASES) {
      test(name, () => {
        const reference = normalizeVoidTags(String(referenceHtmlProcessor.processSync(md)));
        expect(normalizeVoidTags(markdownToHtml(md, { features: FEATURES }).html)).toBe(reference);
      });
    }
  });

  describe("MDX matches the reference", () => {
    const MDX_CASES: Array<[string, string]> = [
      ["text and JSX across lines", "a <A />\nb"],
      ["JSX children", "<A>a\nb</A>"],
      ["inline JSX on every line still unravels", "<A>x</A>\n<B>y</B>"],
    ];
    for (const [name, md] of MDX_CASES) {
      test(name, async () => {
        await assertMdxPluginConformance(md, {
          reference: { remarkPlugins: [remarkBreaks] },
          satteri: { features: { breaks: true } },
          components: { A: "span", B: "span" },
        });
      });
    }
  });

  test("the break spans the line ending", () => {
    const tree = markdownToMdast("a \nb", { features: FEATURES });
    const paragraph = tree.children[0];
    if (paragraph?.type !== "paragraph") throw new Error("expected a paragraph");
    expect(
      paragraph.children.map((child) => [
        child.type,
        child.position?.start.offset,
        child.position?.end.offset,
      ]),
    ).toEqual([
      ["text", 0, 1],
      ["break", 2, 3],
      ["text", 3, 4],
    ]);
  });

  test("a line feed character reference stays text, unlike the reference", () => {
    expect(satteriMdast("a&#10;b")).toEqual(paragraphRoot([{ type: "text", value: "a\nb" }]));
    expect(referenceMdast("a&#10;b")).toEqual(
      paragraphRoot([
        { type: "text", value: "a" },
        { type: "break" },
        { type: "text", value: "b" },
      ]),
    );
  });

  test("off by default", () => {
    expect(satteriMdast("a\nb", {})).toEqual(paragraphRoot([{ type: "text", value: "a\nb" }]));
  });
});
