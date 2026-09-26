import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import rehypeStringify from "rehype-stringify";
import { unified } from "unified";
import { describe, expect, test } from "vitest";
import { hastToHtml, htmlToHast, markdownToHast, markdownToHtml } from "../../src/index.js";
import type { Features, HastNode } from "../../src/index.js";

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

const CMARK_ONLY_FEATURES: Features = {
  gfm: false,
  frontmatter: false,
  math: false,
  headingAttributes: false,
};

const reference = unified().use(rehypeStringify, {
  allowDangerousHtml: true,
  characterReferences: { useNamedReferences: true },
});

const referenceHtml = (tree: HastNode): string => reference.stringify(tree as never);

/** `hast-util-to-html` leaves `>`, `'` and `` ` `` bare where the renderer,
 *  following cmark, escapes them; both are valid HTML for the same text. */
const foldEscapes = (html: string): string =>
  html.replaceAll("&gt;", ">").replaceAll("&#x27;", "'").replaceAll("&#x60;", "`");

/** A document render ends in a newline; the serializer emits the tree as-is. */
const stripFinalNewline = (html: string): string =>
  html.endsWith("\n") ? html.slice(0, -1) : html;

/** The parser drops a newline directly after `<pre>`/`<textarea>` and no
 *  serializer re-adds it (`hast-util-to-html` included), so these never settle. */
const eatsALeadingNewline = (html: string): boolean =>
  /<(?:pre|textarea|listing)[^>]*>\n/.test(html);

interface Divergence {
  example: number;
  section: string;
  actual: string;
  expected: string;
}

function format(what: string, divergences: Divergence[]): string {
  const lines = divergences.map(
    (d) =>
      `  example ${d.example} [${d.section}]:\n    actual:   ${JSON.stringify(d.actual)}\n    expected: ${JSON.stringify(d.expected)}`,
  );
  return `${divergences.length} ${what}:\n${lines.join("\n")}`;
}

/** Compared count included so a suite that stops exercising the corpus fails
 *  instead of passing vacuously. */
function collect(run: (ex: SpecExample) => [actual: string, expected: string] | undefined): {
  divergences: Divergence[];
  compared: number;
} {
  const divergences: Divergence[] = [];
  let compared = 0;
  for (const ex of examples) {
    const pair = run(ex);
    if (pair === undefined) continue;
    compared++;
    const [actual, expected] = pair;
    if (actual !== expected) {
      divergences.push({ example: ex.example, section: ex.section, actual, expected });
    }
  }
  return { divergences, compared };
}

const MIN_COMPARED = 600;

const LEVEL_TIMEOUT_MS = 60_000;

describe("hastToHtml conformance (CommonMark spec.json)", () => {
  test(
    "serializes every spec tree like hast-util-to-html",
    () => {
      const { divergences, compared } = collect((ex) => {
        const tree = markdownToHast(ex.markdown, { features: CMARK_ONLY_FEATURES });
        return [foldEscapes(hastToHtml(tree)), foldEscapes(referenceHtml(tree))];
      });
      expect(compared).toBeGreaterThan(MIN_COMPARED);
      expect(
        divergences,
        format("serializer divergences vs hast-util-to-html", divergences),
      ).toEqual([]);
    },
    LEVEL_TIMEOUT_MS,
  );

  test(
    "agrees with markdownToHtml on every spec document",
    () => {
      const { divergences, compared } = collect((ex) => [
        stripFinalNewline(
          hastToHtml(markdownToHast(ex.markdown, { features: CMARK_ONLY_FEATURES })),
        ),
        stripFinalNewline(markdownToHtml(ex.markdown, { features: CMARK_ONLY_FEATURES }).html),
      ]);
      expect(compared).toBeGreaterThan(MIN_COMPARED);
      expect(divergences, format("divergences vs markdownToHtml", divergences)).toEqual([]);
    },
    LEVEL_TIMEOUT_MS,
  );

  test(
    "serializes every tree parsed from the spec's HTML like hast-util-to-html",
    () => {
      const { divergences, compared } = collect((ex) => {
        const tree = htmlToHast(ex.html, { fragment: true });
        return [foldEscapes(hastToHtml(tree)), foldEscapes(referenceHtml(tree))];
      });
      expect(compared).toBeGreaterThan(MIN_COMPARED);
      expect(
        divergences,
        format("serializer divergences vs hast-util-to-html", divergences),
      ).toEqual([]);
    },
    LEVEL_TIMEOUT_MS,
  );

  test(
    "re-parses its own output to the same HTML",
    () => {
      const { divergences, compared } = collect((ex) => {
        if (eatsALeadingNewline(ex.html)) return undefined;
        const once = hastToHtml(htmlToHast(ex.html, { fragment: true }));
        return [hastToHtml(htmlToHast(once, { fragment: true })), once];
      });
      expect(compared).toBeGreaterThan(MIN_COMPARED);
      expect(divergences, format("round-trip divergences", divergences)).toEqual([]);
    },
    LEVEL_TIMEOUT_MS,
  );
});
