import fc from "fast-check";
import { pathToFileURL } from "node:url";
import { existsSync, readFileSync, appendFileSync } from "node:fs";
import { expect } from "vitest";
import { mdxToMdast, mdxToHast, evaluate as satteriEvaluate } from "../../../src/index.js";
import { evaluate as mdxEvaluate } from "@mdx-js/mdx";
import { remark } from "remark";
import remarkMdx from "remark-mdx";
import remarkGfm from "remark-gfm";
import { toHast } from "mdast-util-to-hast";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import * as runtime from "react/jsx-runtime";
import {
  referenceMdast,
  referenceHast,
  referenceHtml,
  satteriMdast,
  satteriHast,
  satteriHtml,
  referenceFmMdast,
  referenceFmHast,
  referenceFmHtml,
  satteriFmMdast,
  satteriFmHast,
  satteriFmHtml,
  referenceMathMdast,
  referenceMathHast,
  referenceMathHtml,
  satteriMathMdast,
  satteriMathHast,
  satteriMathHtml,
} from "../helpers.js";

const { remarkMarkAndUnravel } = await import(
  pathToFileURL("node_modules/@mdx-js/mdx/lib/plugin/remark-mark-and-unravel.js").href
);

export const NUM_RUNS = Number(process.env.FUZZ_RUNS) || 200;
// MDX eval compiles + renders per run, so it's far heavier than parse-only
// fuzzers. Keep its default low; override with FUZZ_RUNS_EVAL for a thorough
// pass.
export const NUM_RUNS_EVAL = Number(process.env.FUZZ_RUNS_EVAL) || 50;

// Set FUZZ_SEED to reproduce a previous failing run. Each test file logs its
// seed at import time so failures can be replayed deterministically.
const FUZZ_SEED = Number(process.env.FUZZ_SEED) || Date.now();
console.log(`[fuzz] seed=${FUZZ_SEED}`);

export const FC_OPTIONS: fc.Parameters<unknown> = {
  numRuns: NUM_RUNS,
  seed: FUZZ_SEED,
  endOnFailure: false,
  verbose: fc.VerbosityLevel.None,
};
export const FC_OPTIONS_EVAL: fc.Parameters<unknown> = {
  numRuns: NUM_RUNS_EVAL,
  seed: FUZZ_SEED,
  endOnFailure: false,
  verbose: fc.VerbosityLevel.None,
};

// Arbitraries — markdown building blocks

export const INLINE_TEXT = fc.string({
  unit: fc.constantFrom(..."abcdefghijklmnopqrstuvwxyz 0123456789".split("")),
  minLength: 1,
  maxLength: 30,
});

export const WORD = fc.string({
  unit: fc.constantFrom(..."abcdefghijklmnopqrstuvwxyz".split("")),
  minLength: 1,
  maxLength: 12,
});

const URL_ARB = WORD.map((w) => `https://example.com/${w}`);

export const heading = fc
  .tuple(fc.integer({ min: 1, max: 6 }), INLINE_TEXT)
  .map(([level, text]) => `${"#".repeat(level)} ${text}`);

export const paragraph = INLINE_TEXT;
export const bold = INLINE_TEXT.map((t) => `**${t}**`);
export const italic = INLINE_TEXT.map((t) => `*${t}*`);
export const inlineCode = WORD.map((t) => `\`${t}\``);
const strikethrough = INLINE_TEXT.map((t) => `~~${t}~~`);
export const link = fc.tuple(INLINE_TEXT, URL_ARB).map(([text, url]) => `[${text}](${url})`);
const image = fc.tuple(WORD, URL_ARB).map(([alt, url]) => `![${alt}](${url})`);
export const blockquote = INLINE_TEXT.map((t) => `> ${t}`);

export const codeBlock = fc
  .tuple(
    fc.constantFrom("", "js", "ts", "python", "rust", "html"),
    fc.string({
      unit: fc.constantFrom(..."abcdefghijklmnopqrstuvwxyz 0123456789=;.\n".split("")),
      minLength: 1,
      maxLength: 60,
    }),
  )
  .map(([lang, code]) => `\`\`\`${lang}\n${code}\n\`\`\``);

export const horizontalRule = fc.constantFrom("---", "***", "___");

export const unorderedList = fc
  .array(INLINE_TEXT, { minLength: 1, maxLength: 5 })
  .map((items) => items.map((i) => `- ${i}`).join("\n"));

const orderedList = fc
  .array(INLINE_TEXT, { minLength: 1, maxLength: 5 })
  .map((items) => items.map((item, idx) => `${idx + 1}. ${item}`).join("\n"));

const taskList = fc
  .array(fc.tuple(fc.boolean(), INLINE_TEXT), { minLength: 1, maxLength: 5 })
  .map((items) => items.map(([checked, text]) => `- [${checked ? "x" : " "}] ${text}`).join("\n"));

export const table = fc
  .tuple(
    fc.array(WORD, { minLength: 2, maxLength: 4 }),
    fc.array(fc.array(WORD, { minLength: 2, maxLength: 4 }), { minLength: 1, maxLength: 3 }),
  )
  .map(([headers, rows]) => {
    const cols = headers.length;
    const headerRow = `| ${headers.join(" | ")} |`;
    const sepRow = `| ${headers.map(() => "---").join(" | ")} |`;
    const dataRows = rows
      .map((row) => {
        const padded = Array.from({ length: cols }, (_, i) => row[i] ?? "");
        return `| ${padded.join(" | ")} |`;
      })
      .join("\n");
    return `${headerRow}\n${sepRow}\n${dataRows}`;
  });

const definition = fc
  .tuple(WORD, URL_ARB, fc.option(INLINE_TEXT, { nil: undefined }))
  .map(([id, url, title]) =>
    title !== undefined ? `[${id}]: ${url} "${title}"` : `[${id}]: ${url}`,
  );

const autolink = fc.oneof(
  URL_ARB.map((u) => `<${u}>`),
  WORD.map((w) => `<${w}@example.com>`),
);

const footnoteRef = fc.tuple(INLINE_TEXT, WORD).map(([text, id]) => `${text}[^${id}]`);

const footnoteDef = fc.tuple(WORD, INLINE_TEXT).map(([id, text]) => `[^${id}]: ${text}`);

const nestedList = fc
  .array(fc.tuple(INLINE_TEXT, fc.array(INLINE_TEXT, { minLength: 0, maxLength: 3 })), {
    minLength: 1,
    maxLength: 3,
  })
  .map((items) =>
    items
      .map(([parent, children]) =>
        children.length === 0
          ? `- ${parent}`
          : `- ${parent}\n${children.map((c) => `  - ${c}`).join("\n")}`,
      )
      .join("\n"),
  );

const htmlBlock = fc
  .tuple(fc.constantFrom("div", "section", "article", "aside"), INLINE_TEXT)
  .map(([tag, body]) => `<${tag}>\n\n${body}\n\n</${tag}>`);

export const markdownBlock = fc.oneof(
  { weight: 3, arbitrary: heading },
  { weight: 5, arbitrary: paragraph },
  { weight: 2, arbitrary: bold },
  { weight: 2, arbitrary: italic },
  { weight: 2, arbitrary: inlineCode },
  { weight: 1, arbitrary: strikethrough },
  { weight: 2, arbitrary: link },
  { weight: 1, arbitrary: image },
  { weight: 2, arbitrary: blockquote },
  { weight: 2, arbitrary: codeBlock },
  { weight: 1, arbitrary: horizontalRule },
  { weight: 2, arbitrary: unorderedList },
  { weight: 2, arbitrary: orderedList },
  { weight: 1, arbitrary: taskList },
  { weight: 1, arbitrary: table },
  { weight: 1, arbitrary: definition },
  { weight: 1, arbitrary: autolink },
  { weight: 1, arbitrary: footnoteRef },
  { weight: 1, arbitrary: footnoteDef },
  { weight: 2, arbitrary: nestedList },
  { weight: 1, arbitrary: htmlBlock },
);

export const markdownDocument = fc
  .array(markdownBlock, { minLength: 1, maxLength: 12 })
  .map((blocks) => blocks.join("\n\n"));

const MD_SIGNIFICANT_CHARS = "# *_~`[]()!<>|-\\{}@^+=$:/ \t\n".split("");
const ALNUM = "abcdefghijklmnopqrstuvwxyz 0123456789".split("");

// Feature-biased chaos: alnum + markdown-significant chars + extra weight on
// chars relevant to the suite's parser features. Same overall surface, biased
// distribution so suites stress their own syntax more often.
function makeChaos(extras: string): fc.Arbitrary<string> {
  const oneof: { weight: number; arbitrary: fc.Arbitrary<string> }[] = [
    { weight: 1, arbitrary: fc.constantFrom(...ALNUM) },
    { weight: 2, arbitrary: fc.constantFrom(...MD_SIGNIFICANT_CHARS) },
  ];
  if (extras.length > 0) {
    oneof.push({ weight: 3, arbitrary: fc.constantFrom(...extras.split("")) });
  }
  return fc.string({ unit: fc.oneof(...oneof), minLength: 0, maxLength: 500 });
}

export const chaosString = makeChaos("");
export const mathChaos = makeChaos("$\\");
export const fmChaos = makeChaos("-+:");
export const mdxChaos = makeChaos("<>{}/");

// MDX arbitraries

// Align with @mdx-js/mdx + remarkGfm. Disable satteri features that don't
// have an easy remark equivalent in the MDX pipeline (heading attributes) or
// that the math/frontmatter suites cover separately.
const mdxParser = remark().use(remarkGfm).use(remarkMdx).use(remarkMarkAndUnravel);
const MDX_FEATURES = {
  headingAttributes: false,
  math: false,
  frontmatter: false,
} as const;
const MDX_PASS_THROUGH_NODES = [
  "mdxJsxFlowElement",
  "mdxJsxTextElement",
  "mdxFlowExpression",
  "mdxTextExpression",
  "mdxjsEsm",
] as any[];

function stripPositionsAndEstree(node: unknown): unknown {
  if (typeof node !== "object" || node === null) return node;
  if (Array.isArray(node)) return node.map(stripPositionsAndEstree);
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(node as Record<string, unknown>)) {
    if (k === "position" || k === "data") continue;
    if (Array.isArray(v)) out[k] = v.map(stripPositionsAndEstree);
    else if (typeof v === "object" && v !== null) out[k] = stripPositionsAndEstree(v);
    else out[k] = v;
  }
  return out;
}

export function referenceMdxMdast(input: string): unknown {
  const mdast = mdxParser.runSync(mdxParser.parse(input));
  return stripPositionsAndEstree(mdast);
}

export function satteriMdxMdast(input: string): unknown {
  return stripPositionsAndEstree(mdxToMdast(input, { features: MDX_FEATURES }));
}

// Satteri drops directive nodes during mdast→hast; match that on the
// reference with empty directive handlers.
const emptyDirectiveHandler = () => undefined;
const REF_TO_HAST_OPTIONS = {
  allowDangerousHtml: true,
  passThrough: MDX_PASS_THROUGH_NODES,
  handlers: {
    containerDirective: emptyDirectiveHandler,
    leafDirective: emptyDirectiveHandler,
    textDirective: emptyDirectiveHandler,
  },
};

export function referenceMdxHast(input: string): unknown {
  const mdast = mdxParser.runSync(mdxParser.parse(input));
  return stripPositionsAndEstree(toHast(mdast, REF_TO_HAST_OPTIONS));
}

export function satteriMdxHast(input: string): unknown {
  return stripPositionsAndEstree(mdxToHast(input, { features: MDX_FEATURES }));
}

const JSX_TAG = fc.constantFrom("Foo", "Bar", "Box", "Item", "Wrapper");

export const jsxComponents: Record<string, Function> = {
  Foo: (props: any) => createElement("div", null, `foo=${JSON.stringify(props)}`),
  Bar: (props: any) => createElement("em", null, `bar=${JSON.stringify(props)}`),
  Box: (props: any) => createElement("section", null, props.children),
  Item: (props: any) => createElement("li", null, props.children),
  Wrapper: (props: any) => createElement("div", null, props.children),
};

const SAFE_EXPR_TEXT = fc.string({
  unit: fc.constantFrom(..."abcdefghijklmnopqrstuvwxyz0123456789 ".split("")),
  minLength: 1,
  maxLength: 20,
});

const jsExpression = fc.oneof(
  fc.integer({ min: -999, max: 999 }).map((n) => `{${n}}`),
  SAFE_EXPR_TEXT.map((t) => `{\`${t}\`}`),
  fc.constantFrom("{1 + 2}", "{true ? 'a' : 'b'}", "{`hello`}", "{/* comment */}", "{String(42)}"),
);

const jsxSelfClosing = fc
  .tuple(
    JSX_TAG,
    fc.array(
      fc.tuple(
        WORD,
        fc.oneof(
          fc.integer({ min: 0, max: 99 }).map((n) => `{${n}}`),
          WORD.map((w) => `"${w}"`),
        ),
      ),
      { minLength: 0, maxLength: 3 },
    ),
  )
  .map(([tag, attrs]) => {
    const attrStr = attrs.map(([k, v]) => ` ${k}=${v}`).join("");
    return `<${tag}${attrStr}/>`;
  });

const jsxWithChildren = fc
  .tuple(fc.constantFrom("Box", "Wrapper"), fc.oneof(SAFE_EXPR_TEXT, jsExpression))
  .map(([tag, child]) => `<${tag}>${child}</${tag}>`);

const jsxFragment = fc.oneof(SAFE_EXPR_TEXT, jsExpression).map((child) => `<>${child}</>`);

const mdxInlineElement = fc.oneof(
  { weight: 3, arbitrary: jsExpression },
  { weight: 3, arbitrary: jsxSelfClosing },
  { weight: 2, arbitrary: jsxWithChildren },
  { weight: 1, arbitrary: jsxFragment },
);

const mdxParagraph = fc
  .array(
    fc.oneof({ weight: 3, arbitrary: SAFE_EXPR_TEXT }, { weight: 2, arbitrary: mdxInlineElement }),
    { minLength: 1, maxLength: 4 },
  )
  .map((parts) => parts.join(" "));

const mdxBlock = fc.oneof(
  { weight: 4, arbitrary: mdxParagraph },
  { weight: 2, arbitrary: heading },
  { weight: 2, arbitrary: jsxSelfClosing },
  { weight: 2, arbitrary: jsxWithChildren },
  { weight: 1, arbitrary: jsxFragment },
  { weight: 2, arbitrary: jsExpression },
  { weight: 1, arbitrary: blockquote },
  { weight: 1, arbitrary: codeBlock },
  { weight: 1, arbitrary: unorderedList },
  { weight: 1, arbitrary: bold },
  { weight: 1, arbitrary: italic },
  { weight: 1, arbitrary: link },
  { weight: 1, arbitrary: inlineCode },
);

export const mdxDocument = fc
  .array(mdxBlock, { minLength: 1, maxLength: 8 })
  .map((blocks) => blocks.join("\n\n"));

// Math arbitraries

const MATH_CONTENT = fc.string({
  unit: fc.constantFrom(..."abcdefghijklmnopqrstuvwxyz0123456789 +-=^_{}\\".split("")),
  minLength: 1,
  maxLength: 30,
});

const MATH_COMMAND = fc.constantFrom(
  "\\alpha",
  "\\beta",
  "\\gamma",
  "\\delta",
  "\\sum",
  "\\int",
  "\\frac{a}{b}",
  "\\sqrt{x}",
  "\\mathbb{R}",
  "\\cdot",
  "\\times",
  "\\leq",
  "\\geq",
  "\\neq",
  "\\infty",
  "\\partial",
);

const inlineMath = fc.oneof(
  MATH_CONTENT.map((t) => `$${t}$`),
  MATH_COMMAND.map((t) => `$${t}$`),
  fc.tuple(INLINE_TEXT, MATH_CONTENT).map(([t, m]) => `${t} $${m}$`),
);

const displayMath = fc.oneof(
  MATH_CONTENT.map((t) => `$$\n${t}\n$$`),
  MATH_COMMAND.map((t) => `$$\n${t}\n$$`),
  fc
    .tuple(fc.constantFrom("", "js", "math"), MATH_CONTENT)
    .map(([meta, content]) => (meta ? `$$ ${meta}\n${content}\n$$` : `$$\n${content}\n$$`)),
);

const mathBlock = fc.oneof(
  { weight: 3, arbitrary: paragraph },
  { weight: 3, arbitrary: heading },
  { weight: 3, arbitrary: inlineMath },
  { weight: 3, arbitrary: displayMath },
  { weight: 2, arbitrary: bold },
  { weight: 2, arbitrary: italic },
  { weight: 1, arbitrary: codeBlock },
  { weight: 1, arbitrary: blockquote },
  { weight: 1, arbitrary: unorderedList },
  { weight: 1, arbitrary: link },
  { weight: 1, arbitrary: inlineCode },
  { weight: 1, arbitrary: horizontalRule },
  { weight: 1, arbitrary: table },
);

export const mathDocument = fc
  .array(mathBlock, { minLength: 1, maxLength: 10 })
  .map((blocks) => blocks.join("\n\n"));

// Frontmatter arbitraries

const YAML_KEY = fc.string({
  unit: fc.constantFrom(..."abcdefghijklmnopqrstuvwxyz_".split("")),
  minLength: 1,
  maxLength: 12,
});

const YAML_VALUE = fc.oneof(
  WORD,
  fc.integer({ min: -999, max: 9999 }).map(String),
  fc.boolean().map(String),
  INLINE_TEXT.map((t) => `"${t}"`),
);

const yamlFrontmatter = fc
  .array(fc.tuple(YAML_KEY, YAML_VALUE), { minLength: 1, maxLength: 5 })
  .map((pairs) => {
    const fields = pairs.map(([k, v]) => `${k}: ${v}`).join("\n");
    return `---\n${fields}\n---`;
  });

const tomlFrontmatter = fc
  .array(fc.tuple(YAML_KEY, YAML_VALUE), { minLength: 1, maxLength: 5 })
  .map((pairs) => {
    const fields = pairs.map(([k, v]) => `${k} = ${v}`).join("\n");
    return `+++\n${fields}\n+++`;
  });

export const fmDocument = fc
  .tuple(
    fc.oneof(yamlFrontmatter, tomlFrontmatter),
    fc.array(markdownBlock, { minLength: 0, maxLength: 8 }),
  )
  .map(([fm, blocks]) => (blocks.length > 0 ? `${fm}\n\n${blocks.join("\n\n")}` : fm));

// Conformance harness

export type FuzzLevel =
  | "mdast"
  | "hast"
  | "html"
  | "mdx-mdast"
  | "mdx-hast"
  | "math-mdast"
  | "math-hast"
  | "math-html"
  | "fm-mdast"
  | "fm-hast"
  | "fm-html";

export type FuzzSource = "structured" | "chaos" | "corpus";

export interface FuzzIssue {
  input: string;
  level: FuzzLevel;
  source: FuzzSource;
  /** "position-only" if trees match after stripping `position`; "content" otherwise. */
  kind: "content" | "position-only";
  expected: unknown;
  actual: unknown;
}

const HTML_LEVELS = new Set<FuzzLevel>(["html", "math-html", "fm-html"]);

// Divergences we deliberately don't try to match because they stem from
// upstream behaviour we consider buggy or undesirable. Listing them here keeps
// the fuzz signal focused on real regressions.
//
// Format: `${level}\0${input}` — direct equality, no patterns. Inputs come
// straight from past fuzz reports.
const KNOWN_DIVERGENCES = new Set<string>([
  // remark-frontmatter quirk: when YAML/TOML detection fails for `---\n…`,
  // the failed attempt prevents the next line from being recognized as a list
  // marker. Reference: paragraph; satteri (and bare remark + GFM): list.
  "fm-mdast\0---\n+",
  "fm-hast\0---\n+",
  "fm-html\0---\n+",
  // Same root cause for `+...`: the failed YAML attempt also disables table
  // detection on the subsequent line. Reference: paragraph + table; satteri:
  // single paragraph.
  "fm-mdast\0+w*\n+-\n:-",
  "fm-hast\0+w*\n+-\n:-",
  "fm-html\0+w*\n+-\n:-",
  // Same root cause again: `---` followed by content that almost looks like
  // YAML disables block parsing on the next line.
  "fm-mdast\0---\n-",
  "fm-hast\0---\n-",
  "fm-html\0---\n-",
  // `+++` analog for TOML frontmatter: failed TOML attempt leaves the next
  // line as a paragraph instead of letting it open a blockquote.
  "fm-mdast\0+++\n>!*+-",
  "fm-hast\0+++\n>!*+-",
  "fm-html\0+++\n>!*+-",
  // Same family of remark-frontmatter quirks: a `---` followed by an
  // indented list marker disables list detection on the next line.
  "fm-mdast\0---\n + (",
  "fm-hast\0---\n + (",
  "fm-html\0---\n + (",
]);

function stripPositions(node: unknown): unknown {
  if (typeof node !== "object" || node === null) return node;
  if (Array.isArray(node)) return node.map(stripPositions);
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(node as Record<string, unknown>)) {
    if (k === "position") continue;
    out[k] = stripPositions(v);
  }
  return out;
}

function classifyKind(
  level: FuzzLevel,
  actual: unknown,
  expected: unknown,
): "content" | "position-only" {
  if (HTML_LEVELS.has(level)) return "content";
  try {
    expect(stripPositions(actual)).toEqual(stripPositions(expected));
    return "position-only";
  } catch {
    return "content";
  }
}

function compareSingle(input: string, level: FuzzLevel, source: FuzzSource): FuzzIssue | null {
  if (KNOWN_DIVERGENCES.has(`${level}\0${input}`)) return null;
  const { parse, ref } = LEVEL_FUNS[level];
  let actual: unknown;
  let expected: unknown;
  try {
    actual = parse(input);
  } catch {
    actual = "PARSE_ERROR";
  }
  try {
    expected = ref(input);
  } catch {
    expected = "PARSE_ERROR";
  }
  try {
    expect(actual).toEqual(expected);
    return null;
  } catch {
    return { input, level, source, kind: classifyKind(level, actual, expected), expected, actual };
  }
}

export const LEVEL_FUNS: Record<
  FuzzLevel,
  { parse: (s: string) => unknown; ref: (s: string) => unknown }
> = {
  mdast: { parse: satteriMdast, ref: referenceMdast },
  hast: { parse: satteriHast, ref: referenceHast },
  html: { parse: satteriHtml, ref: referenceHtml },
  "mdx-mdast": { parse: satteriMdxMdast, ref: referenceMdxMdast },
  "mdx-hast": { parse: satteriMdxHast, ref: referenceMdxHast },
  "math-mdast": { parse: satteriMathMdast, ref: referenceMathMdast },
  "math-hast": { parse: satteriMathHast, ref: referenceMathHast },
  "math-html": { parse: satteriMathHtml, ref: referenceMathHtml },
  "fm-mdast": { parse: satteriFmMdast, ref: referenceFmMdast },
  "fm-hast": { parse: satteriFmHast, ref: referenceFmHast },
  "fm-html": { parse: satteriFmHtml, ref: referenceFmHtml },
};

export function collectIssues(
  arbitrary: fc.Arbitrary<string>,
  level: FuzzLevel,
  source: "structured" | "chaos",
): FuzzIssue[] {
  const issues: FuzzIssue[] = [];
  fc.assert(
    fc.property(arbitrary, (input) => {
      const issue = compareSingle(input, level, source);
      if (issue) issues.push(issue);
      return true;
    }),
    FC_OPTIONS,
  );
  return issues;
}

export function loadCorpus(corpusPath: URL): string[] {
  if (!existsSync(corpusPath)) return [];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const line of readFileSync(corpusPath, "utf-8").split("\n")) {
    if (!line || seen.has(line)) continue;
    seen.add(line);
    try {
      out.push(JSON.parse(line) as string);
    } catch {
      // skip malformed lines silently — corpus is best-effort
    }
  }
  return out;
}

export function replayCorpus(inputs: string[], levels: FuzzLevel[]): FuzzIssue[] {
  const issues: FuzzIssue[] = [];
  for (const input of inputs) {
    for (const level of levels) {
      const issue = compareSingle(input, level, "corpus");
      if (issue) issues.push(issue);
    }
  }
  return issues;
}

export function appendCorpus(corpusPath: URL, inputs: string[]): void {
  if (inputs.length === 0) return;
  const existing = new Set(loadCorpus(corpusPath).map((s) => JSON.stringify(s)));
  const fresh = [...new Set(inputs.map((i) => JSON.stringify(i)))].filter(
    (line) => !existing.has(line),
  );
  if (fresh.length === 0) return;
  appendFileSync(corpusPath, fresh.join("\n") + "\n");
}

function diffFingerprint(expected: unknown, actual: unknown, path = ""): string[] {
  if (typeof expected !== typeof actual)
    return [`${path}: type ${typeof expected} vs ${typeof actual}`];
  if (typeof expected !== "object" || expected === null || actual === null) {
    if (expected !== actual) return [`${path}: <leaf-mismatch>`];
    return [];
  }
  if (Array.isArray(expected) && Array.isArray(actual)) {
    if (expected.length !== actual.length)
      return [`${path}: array length ${expected.length} vs ${actual.length}`];
    return expected.flatMap((_, i) => diffFingerprint(expected[i], actual[i], `${path}[${i}]`));
  }
  const eObj = expected as Record<string, unknown>;
  const aObj = actual as Record<string, unknown>;
  const allKeys = new Set([...Object.keys(eObj), ...Object.keys(aObj)]);
  const diffs: string[] = [];
  for (const key of allKeys) {
    if (!(key in eObj)) diffs.push(`${path}.${key}: missing in expected`);
    else if (!(key in aObj)) diffs.push(`${path}.${key}: missing in actual`);
    else diffs.push(...diffFingerprint(eObj[key], aObj[key], `${path}.${key}`));
  }
  return diffs;
}

function classifyDiff(expected: unknown, actual: unknown): string {
  const diffs = diffFingerprint(expected, actual);
  const patterns = diffs.map((d) => d.replace(/\[\d+\]/g, "[N]").replace(/\.\d+\./g, ".N."));
  return patterns.sort().join(" | ");
}

export function deduplicateIssues(issues: FuzzIssue[]): FuzzIssue[] {
  const seen = new Map<string, FuzzIssue>();
  for (const issue of issues) {
    const key = `${issue.level}:${issue.kind}:${classifyDiff(issue.expected, issue.actual)}`;
    if (!seen.has(key) || issue.input.length < seen.get(key)!.input.length) {
      seen.set(key, issue);
    }
  }
  return [...seen.values()];
}

export function formatIssue(issue: FuzzIssue, index: number): string {
  const kindTag = issue.kind === "position-only" ? " [position-only]" : "";
  return [
    `## ${index + 1}. [${issue.level.toUpperCase()}] (${issue.source})${kindTag}`,
    "",
    `**Input:** \`${JSON.stringify(issue.input)}\``,
    "",
    "**Expected (reference):**",
    "```json",
    JSON.stringify(issue.expected, null, 2).slice(0, 500),
    "```",
    "",
    "**Actual (Sätteri):**",
    "```json",
    JSON.stringify(issue.actual, null, 2).slice(0, 500),
    "```",
  ].join("\n");
}

// MDX eval harness

function normalizeHtml(html: string): string {
  return html.replace(/>\s+</g, "><").replace(/\s+</g, "<").replace(/>\s+/g, ">").trim();
}

export interface MdxEvalIssue {
  input: string;
  source: "structured" | "chaos" | "corpus";
  kind: "mismatch" | "satteri-error" | "both-error-disagree";
  referenceHtml?: string | undefined;
  satteriHtml?: string | undefined;
  error?: string | undefined;
}

async function compareMdxEval(
  input: string,
  source: MdxEvalIssue["source"],
): Promise<MdxEvalIssue | null> {
  let refHtml: string | undefined;
  let refError = false;
  try {
    const { default: RefComponent } = (await mdxEvaluate(input, {
      ...runtime,
      remarkPlugins: [remarkGfm],
    })) as { default: Function };
    refHtml = normalizeHtml(
      renderToStaticMarkup(createElement(RefComponent as any, { components: jsxComponents })),
    );
  } catch {
    refError = true;
  }

  let satHtml: string | undefined;
  let satError = false;
  try {
    const { default: SatComponent } = await satteriEvaluate(input, {
      ...runtime,
      features: MDX_FEATURES,
    } as any);
    satHtml = normalizeHtml(
      renderToStaticMarkup(createElement(SatComponent as any, { components: jsxComponents })),
    );
  } catch {
    satError = true;
  }

  if (refError && satError) return null;

  if (refError !== satError) {
    return {
      input,
      source,
      kind: satError ? "satteri-error" : "both-error-disagree",
      referenceHtml: refHtml,
      satteriHtml: satHtml,
      error: satError
        ? "satteri threw but @mdx-js/mdx succeeded"
        : "@mdx-js/mdx threw but satteri succeeded",
    };
  }

  if (refHtml !== satHtml) {
    return { input, source, kind: "mismatch", referenceHtml: refHtml, satteriHtml: satHtml };
  }

  return null;
}

export async function collectMdxEvalIssues(
  arbitrary: fc.Arbitrary<string>,
  source: "structured" | "chaos",
): Promise<MdxEvalIssue[]> {
  const issues: MdxEvalIssue[] = [];
  await fc.assert(
    fc.asyncProperty(arbitrary, async (input) => {
      const issue = await compareMdxEval(input, source);
      if (issue) issues.push(issue);
      return true;
    }),
    FC_OPTIONS_EVAL,
  );
  return issues;
}

export async function replayMdxEvalCorpus(inputs: string[]): Promise<MdxEvalIssue[]> {
  const issues: MdxEvalIssue[] = [];
  for (const input of inputs) {
    const issue = await compareMdxEval(input, "corpus");
    if (issue) issues.push(issue);
  }
  return issues;
}

// Strip attribute values and text content so structurally-equivalent HTML
// collapses to one fingerprint regardless of the specific chars in the input.
function structuralHtml(html: string | undefined): string {
  if (html === undefined) return "(none)";
  return html.replace(/=("[^"]*"|'[^']*')/g, "=$A").replace(/>([^<>]+)</g, ">$T<");
}

export function deduplicateMdxEvalIssues(issues: MdxEvalIssue[]): MdxEvalIssue[] {
  const seen = new Map<string, MdxEvalIssue>();
  for (const issue of issues) {
    const key = `${issue.kind}:${structuralHtml(issue.referenceHtml)}:${structuralHtml(issue.satteriHtml)}`;
    if (!seen.has(key) || issue.input.length < seen.get(key)!.input.length) {
      seen.set(key, issue);
    }
  }
  return [...seen.values()];
}

export function formatMdxEvalIssue(issue: MdxEvalIssue, index: number): string {
  const lines = [
    `## ${index + 1}. [MDX-EVAL] ${issue.kind} (${issue.source})`,
    "",
    `**Input:** \`${JSON.stringify(issue.input)}\``,
  ];
  if (issue.error) lines.push("", `**Error:** ${issue.error}`);
  if (issue.referenceHtml !== undefined)
    lines.push("", `**@mdx-js/mdx:** \`${issue.referenceHtml.slice(0, 300)}\``);
  if (issue.satteriHtml !== undefined)
    lines.push("", `**Sätteri:** \`${issue.satteriHtml.slice(0, 300)}\``);
  return lines.join("\n");
}
