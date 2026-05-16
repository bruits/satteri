import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Nodes } from "hast";
import remarkParse from "remark-parse";
import remarkRehype from "remark-rehype";
import rehypeStringify from "rehype-stringify";
import { unified } from "unified";
import { describe, expect, test } from "vitest";
import { markdownToHast, markdownToHtml, markdownToMdast } from "../../src/index.js";
import type { Features } from "../../src/index.js";

interface SpecExample {
  markdown: string;
  html: string;
  example: number;
  section: string;
}

const specPath = fileURLToPath(
  new URL(
    "../../../../crates/satteri-pulldown-cmark/third_party/CommonMark/spec.json",
    import.meta.url,
  ),
);
const examples = JSON.parse(readFileSync(specPath, "utf8")) as SpecExample[];

// spec.json is plain CommonMark — disable every extension on both sides so
// the comparison stays apples-to-apples. (The default helpers in `./helpers`
// enable GFM, which would skew strikethrough/table/autolink-literal cases.)
const CMARK_ONLY_FEATURES: Features = {
  gfm: false,
  frontmatter: false,
  math: false,
  headingAttributes: false,
};

const remarkMdastProcessor = unified().use(remarkParse);
const remarkHastProcessor = unified()
  .use(remarkParse)
  .use(remarkRehype, { allowDangerousHtml: true });
const remarkHtmlProcessor = unified()
  .use(remarkParse)
  .use(remarkRehype, { allowDangerousHtml: true })
  .use(rehypeStringify, { allowDangerousHtml: true });

function serialize<T>(node: T): T {
  return JSON.parse(JSON.stringify(node)) as T;
}

function stripPositions(value: unknown): unknown {
  if (typeof value !== "object" || value === null) return value;
  if (Array.isArray(value)) return value.map(stripPositions);
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
    if (k === "position") continue;
    out[k] = stripPositions(v);
  }
  return out;
}

// Intentional divergence: Sätteri keeps `data.lang` on HAST code elements;
// remark-rehype drops it. See website/content/docs/divergences.md.
function stripHastDataLang(value: unknown): unknown {
  if (typeof value !== "object" || value === null) return value;
  if (Array.isArray(value)) return value.map(stripHastDataLang);
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
    if (k === "data" && v && typeof v === "object" && "lang" in (v as object)) {
      const { lang: _lang, ...rest } = v as Record<string, unknown>;
      if (Object.keys(rest).length > 0) out[k] = rest;
      continue;
    }
    out[k] = stripHastDataLang(v);
  }
  return out;
}

function normalizeHtml(html: string): string {
  let out = html
    .replace(/<br>/g, "<br />")
    .replace(/<br\/>/g, "<br />")
    .replace(/<hr>/g, "<hr />")
    .replace(/<hr\/>/g, "<hr />");
  // Canonicalize entity encoding style — remark+rehype favours hex (`&#x26;`)
  // while satteri (and the spec) use named entities. Then collapse `&gt;`
  // and `&quot;` to their raw forms because rehype-stringify doesn't encode
  // `>` or `"` outside of contexts that require it. All produce semantically
  // identical HTML.
  out = out
    .replace(/&#x3C;/g, "&lt;")
    .replace(/&#x3E;/g, "&gt;")
    .replace(/&#x26;/g, "&amp;")
    .replace(/&#x22;/g, "&quot;")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"');
  // Drop the trailing ` /` on void elements so `<img ...>` and `<img ... />`
  // compare equal — both are HTML5-valid renderings of the same node.
  out = out.replace(/<(img|input|br|hr)([^>]*?)\s*\/?>/g, "<$1$2>");
  return out.trim();
}

function satteriMdast(md: string) {
  return stripPositions(serialize(markdownToMdast(md, { features: CMARK_ONLY_FEATURES })));
}
function satteriHast(md: string) {
  return stripHastDataLang(
    stripPositions(serialize(markdownToHast(md, { features: CMARK_ONLY_FEATURES }))),
  );
}
function satteriHtml(md: string) {
  return normalizeHtml(markdownToHtml(md, { features: CMARK_ONLY_FEATURES }).html);
}
function remarkMdast(md: string) {
  return stripPositions(serialize(remarkMdastProcessor.parse(md)));
}
function remarkHast(md: string) {
  const tree = remarkHastProcessor.parse(md);
  return stripPositions(serialize(remarkHastProcessor.runSync(tree) as Nodes));
}
function remarkHtml(md: string) {
  return normalizeHtml(String(remarkHtmlProcessor.processSync(md)));
}

type Level = "mdast" | "hast" | "html";

const RUNNERS: Record<
  Level,
  { satteri: (md: string) => unknown; remark: (md: string) => unknown }
> = {
  mdast: { satteri: satteriMdast, remark: remarkMdast },
  hast: { satteri: satteriHast, remark: remarkHast },
  html: { satteri: satteriHtml, remark: remarkHtml },
};

interface Divergence {
  example: number;
  section: string;
  markdown: string;
}

function collect(level: Level): Divergence[] {
  const out: Divergence[] = [];
  const { satteri, remark } = RUNNERS[level];
  for (const ex of examples) {
    let actual: unknown;
    let expected: unknown;
    try {
      actual = satteri(ex.markdown);
    } catch {
      actual = "PARSE_ERROR";
    }
    try {
      expected = remark(ex.markdown);
    } catch {
      expected = "PARSE_ERROR";
    }
    try {
      expect(actual).toEqual(expected);
    } catch {
      out.push({ example: ex.example, section: ex.section, markdown: ex.markdown });
    }
  }
  return out;
}

function format(level: Level, divergences: Divergence[]): string {
  const lines = divergences.map(
    (d) => `  example ${d.example} [${d.section}]: ${JSON.stringify(d.markdown)}`,
  );
  return `${divergences.length} ${level} divergences between satteri and remark:\n${lines.join("\n")}`;
}

const LEVEL_TIMEOUT_MS = 60_000;

describe("CommonMark spec.json mutual conformance (satteri vs remark, no GFM)", () => {
  test(
    "mdast",
    () => {
      const divergences = collect("mdast");
      expect(divergences, format("mdast", divergences)).toEqual([]);
    },
    LEVEL_TIMEOUT_MS,
  );

  test(
    "hast",
    () => {
      const divergences = collect("hast");
      expect(divergences, format("hast", divergences)).toEqual([]);
    },
    LEVEL_TIMEOUT_MS,
  );

  test(
    "html",
    () => {
      const divergences = collect("html");
      expect(divergences, format("html", divergences)).toEqual([]);
    },
    LEVEL_TIMEOUT_MS,
  );
});
