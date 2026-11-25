import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkCjkFriendly from "remark-cjk-friendly";
import remarkRehype from "remark-rehype";
import rehypeStringify from "rehype-stringify";
import { markdownToHtml, markdownToMdast } from "../../src/index.js";
import type { Features } from "../../src/index.js";
import { assertMdxPluginConformance } from "./helpers.js";

const FEATURES = { cjkFriendlyEmphasis: true, frontmatter: false } satisfies Features;

const referenceProcessor = unified().use(remarkParse).use(remarkGfm).use(remarkCjkFriendly);
const referenceHtmlProcessor = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkCjkFriendly)
  .use(remarkRehype, { allowDangerousHtml: true })
  .use(rehypeStringify, { allowDangerousHtml: true });

function referenceMdast(md: string): unknown {
  return JSON.parse(JSON.stringify(referenceProcessor.parse(md)));
}

function satteriMdast(md: string): unknown {
  return JSON.parse(JSON.stringify(markdownToMdast(md, { features: FEATURES })));
}

function referenceHtml(md: string): string {
  return String(referenceHtmlProcessor.processSync(md)).trim();
}

function html(md: string, features: Features = FEATURES): string {
  return markdownToHtml(md, { features }).html.trim();
}

// The specification's test corpus (tats-u/markdown-cjk-friendly at e7bad67), one case per paragraph.
// Its strikethrough file is left out: remark-cjk-friendly leaves `~` to remark-cjk-friendly-gfm-strikethrough.
const CORPUS = new URL(
  "../../../../crates/satteri-pulldown-cmark/third_party/markdown-cjk-friendly/",
  import.meta.url,
);
const CASES = [
  "strong",
  "korean",
  "pseudo-emoji",
  "non-bmp",
  "underscore-cjk-punct",
  "gfm-non-cjk",
].flatMap((file) =>
  readFileSync(new URL(`${file}.md`, CORPUS), "utf8")
    // A Windows checkout with `core.autocrlf` gives the corpus CRLF line endings.
    .replaceAll("\r\n", "\n")
    .split(/\n{2,}/)
    .filter((md) => md.trim() !== "")
    .map((md, index): [string, string] => [`${file} ${index + 1}`, md]),
);

const COMMONMARK_EXAMPLES = JSON.parse(
  readFileSync(
    new URL(
      "../../../../crates/satteri-pulldown-cmark/third_party/CommonMark/spec.json",
      import.meta.url,
    ),
    "utf8",
  ),
) as Array<{ markdown: string; example: number }>;

describe("cjkFriendlyEmphasis: conformance vs remark-cjk-friendly", () => {
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
        expect(html(md)).toBe(referenceHtml(md));
      });
    }
  });

  describe("MDX matches the reference", () => {
    const MDX_CASES: Array<[string, string]> = [
      ["emphasis beside CJK punctuation", "**テスト。**テスト"],
      ["emphasis around JSX", "これは**<A>「重要」</A>**です"],
    ];
    for (const [name, md] of MDX_CASES) {
      test(name, async () => {
        await assertMdxPluginConformance(md, {
          reference: { remarkPlugins: [remarkCjkFriendly] },
          satteri: { features: { cjkFriendlyEmphasis: true } },
          components: { A: "span" },
        });
      });
    }
  });

  // remark-cjk-friendly drops micromark's rule that a `*` run opens and closes
  // beside another attention marker, which changes text with no CJK character in it.
  test("a `*` run beside `_` still opens and closes, unlike the reference", () => {
    const md = "a*_b_*";
    expect(html(md)).toBe("<p>a<em><em>b</em></em></p>");
    expect(html(md, {})).toBe(html(md));
    expect(referenceHtml(md)).toBe("<p>a*<em>b</em>*</p>");
  });

  // As remark-cjk-friendly-gfm-strikethrough renders strikethrough; subscript and
  // superscript flank by the same rules.
  test("strikethrough, subscript and superscript open and close beside CJK punctuation", () => {
    const scripts = { ...FEATURES, subscript: true, superscript: true } satisfies Features;
    expect(html("これは~~「削除」~~です")).toBe("<p>これは<del>「削除」</del>です</p>");
    expect(html("これは~「下付き」~です", scripts)).toBe("<p>これは<sub>「下付き」</sub>です</p>");
    expect(html("これは^「上付き」^です", scripts)).toBe("<p>これは<sup>「上付き」</sup>です</p>");
    expect(referenceHtml("これは~~「削除」~~です")).toBe("<p>これは~~「削除」~~です</p>");
  });

  test("the CommonMark examples render as without the feature", () => {
    const off = { gfm: false, frontmatter: false } satisfies Features;
    const on = { ...off, cjkFriendlyEmphasis: true } satisfies Features;
    const changed = COMMONMARK_EXAMPLES.filter(
      ({ markdown }) => html(markdown, on) !== html(markdown, off),
    ).map(({ example }) => example);
    expect(changed).toEqual([]);
  });

  test("off by default", () => {
    expect(html("**テスト。**テスト", {})).toBe("<p>**テスト。**テスト</p>");
    expect(html("これは~~「削除」~~です", {})).toBe("<p>これは~~「削除」~~です</p>");
  });
});
