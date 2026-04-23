import { describe, test, expect } from "vitest";
import { writeFileSync } from "node:fs";
import fc from "fast-check";
import { markdownToMdast, markdownToHast, mdxToMdast, mdxToHast } from "../../src/index.js";
import { evaluate as mdxEvaluate } from "@mdx-js/mdx";
import { evaluate as satteriEvaluate } from "../../src/index.js";
import { remark } from "remark";
import remarkMdx from "remark-mdx";
import { toHast } from "mdast-util-to-hast";
import { pathToFileURL } from "node:url";

const { remarkMarkAndUnravel } = await import(
  pathToFileURL("node_modules/@mdx-js/mdx/lib/plugin/remark-mark-and-unravel.js").href
);
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
} from "./helpers.js";

const INLINE_TEXT = fc.string({
  unit: fc.constantFrom(..."abcdefghijklmnopqrstuvwxyz 0123456789".split("")),
  minLength: 1,
  maxLength: 30,
});

const WORD = fc.string({
  unit: fc.constantFrom(..."abcdefghijklmnopqrstuvwxyz".split("")),
  minLength: 1,
  maxLength: 12,
});

const URL_ARB = WORD.map((w) => `https://example.com/${w}`);

const heading = fc
  .tuple(fc.integer({ min: 1, max: 6 }), INLINE_TEXT)
  .map(([level, text]) => `${"#".repeat(level)} ${text}`);

const paragraph = INLINE_TEXT;

const bold = INLINE_TEXT.map((t) => `**${t}**`);
const italic = INLINE_TEXT.map((t) => `*${t}*`);
const inlineCode = WORD.map((t) => `\`${t}\``);
const strikethrough = INLINE_TEXT.map((t) => `~~${t}~~`);

const link = fc.tuple(INLINE_TEXT, URL_ARB).map(([text, url]) => `[${text}](${url})`);

const image = fc.tuple(WORD, URL_ARB).map(([alt, url]) => `![${alt}](${url})`);

const blockquote = INLINE_TEXT.map((t) => `> ${t}`);

const codeBlock = fc
  .tuple(
    fc.constantFrom("", "js", "ts", "python", "rust", "html"),
    fc.string({
      unit: fc.constantFrom(..."abcdefghijklmnopqrstuvwxyz 0123456789=;.\n".split("")),
      minLength: 1,
      maxLength: 60,
    }),
  )
  .map(([lang, code]) => `\`\`\`${lang}\n${code}\n\`\`\``);

const horizontalRule = fc.constantFrom("---", "***", "___");

const unorderedList = fc
  .array(INLINE_TEXT, { minLength: 1, maxLength: 5 })
  .map((items) => items.map((i) => `- ${i}`).join("\n"));

const orderedList = fc
  .array(INLINE_TEXT, { minLength: 1, maxLength: 5 })
  .map((items) => items.map((item, idx) => `${idx + 1}. ${item}`).join("\n"));

const taskList = fc
  .array(fc.tuple(fc.boolean(), INLINE_TEXT), { minLength: 1, maxLength: 5 })
  .map((items) => items.map(([checked, text]) => `- [${checked ? "x" : " "}] ${text}`).join("\n"));

const table = fc
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

const markdownBlock = fc.oneof(
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
);

const markdownDocument = fc
  .array(markdownBlock, { minLength: 1, maxLength: 12 })
  .map((blocks) => blocks.join("\n\n"));

const MD_SIGNIFICANT_CHARS = "# *_~`[]()!<>|-\\{}@^+= \t\n".split("");

const chaosString = fc.string({
  unit: fc.oneof(
    fc.constantFrom(..."abcdefghijklmnopqrstuvwxyz 0123456789".split("")),
    fc.constantFrom(...MD_SIGNIFICANT_CHARS),
  ),
  minLength: 0,
  maxLength: 500,
});

const mdxParser = remark().use(remarkMdx).use(remarkMarkAndUnravel);
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

function referenceMdxMdast(input: string): unknown {
  const mdast = mdxParser.runSync(mdxParser.parse(input));
  return stripPositionsAndEstree(mdast);
}

function satteriMdxMdast(input: string): unknown {
  return stripPositionsAndEstree(mdxToMdast(input));
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

function referenceMdxHast(input: string): unknown {
  const mdast = mdxParser.runSync(mdxParser.parse(input));
  return stripPositionsAndEstree(toHast(mdast, REF_TO_HAST_OPTIONS));
}

function satteriMdxHast(input: string): unknown {
  return stripPositionsAndEstree(mdxToHast(input));
}

const JSX_TAG = fc.constantFrom("Foo", "Bar", "Box", "Item", "Wrapper");

const jsxComponents: Record<string, Function> = {
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

const mdxDocument = fc
  .array(mdxBlock, { minLength: 1, maxLength: 8 })
  .map((blocks) => blocks.join("\n\n"));

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

const mathDocument = fc
  .array(mathBlock, { minLength: 1, maxLength: 10 })
  .map((blocks) => blocks.join("\n\n"));

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

const fmDocument = fc
  .tuple(
    fc.oneof(yamlFrontmatter, tomlFrontmatter),
    fc.array(markdownBlock, { minLength: 0, maxLength: 8 }),
  )
  .map(([fm, blocks]) => (blocks.length > 0 ? `${fm}\n\n${blocks.join("\n\n")}` : fm));

const NUM_RUNS = Number(process.env.FUZZ_RUNS) || 200;
const FC_OPTIONS: fc.Parameters<unknown> = {
  numRuns: NUM_RUNS,
  endOnFailure: false,
  verbose: fc.VerbosityLevel.None,
};

type FuzzLevel =
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

interface FuzzIssue {
  input: string;
  level: FuzzLevel;
  source: "structured" | "chaos";
  expected: unknown;
  actual: unknown;
}

const LEVEL_FUNS: Record<
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

function collectIssues(
  arbitrary: fc.Arbitrary<string>,
  level: FuzzLevel,
  source: "structured" | "chaos",
): FuzzIssue[] {
  const issues: FuzzIssue[] = [];
  const { parse, ref } = LEVEL_FUNS[level];

  fc.assert(
    fc.property(arbitrary, (input) => {
      try {
        const actual = parse(input);
        const expected = ref(input);
        expect(actual).toEqual(expected);
      } catch {
        try {
          issues.push({ input, level, source, expected: ref(input), actual: parse(input) });
        } catch {
          issues.push({ input, level, source, expected: "PARSE_ERROR", actual: "PARSE_ERROR" });
        }
      }
      return true;
    }),
    FC_OPTIONS,
  );

  return issues;
}

function diffFingerprint(expected: unknown, actual: unknown, path = ""): string[] {
  if (typeof expected !== typeof actual)
    return [`${path}: type ${typeof expected} vs ${typeof actual}`];
  if (typeof expected !== "object" || expected === null || actual === null) {
    if (expected !== actual)
      return [`${path}: ${JSON.stringify(expected)} vs ${JSON.stringify(actual)}`];
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

function deduplicateIssues(issues: FuzzIssue[]): FuzzIssue[] {
  const seen = new Map<string, FuzzIssue>();
  for (const issue of issues) {
    const key = `${issue.level}:${classifyDiff(issue.expected, issue.actual)}`;
    if (!seen.has(key) || issue.input.length < seen.get(key)!.input.length) {
      seen.set(key, issue);
    }
  }
  return [...seen.values()];
}

function formatIssue(issue: FuzzIssue, index: number): string {
  return [
    `## ${index + 1}. [${issue.level.toUpperCase()}] (${issue.source})`,
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

function normalizeHtml(html: string): string {
  return html.replace(/>\s+</g, "><").replace(/\s+</g, "<").replace(/>\s+/g, ">").trim();
}

interface MdxEvalIssue {
  input: string;
  source: "structured";
  kind: "mismatch" | "satteri-error" | "both-error-disagree";
  referenceHtml?: string | undefined;
  satteriHtml?: string | undefined;
  error?: string | undefined;
}

async function collectMdxEvalIssues(
  arbitrary: fc.Arbitrary<string>,
  source: "structured",
): Promise<MdxEvalIssue[]> {
  const issues: MdxEvalIssue[] = [];

  await fc.assert(
    fc.asyncProperty(arbitrary, async (input) => {
      let refHtml: string | undefined;
      let refError = false;
      try {
        const { default: RefComponent } = (await mdxEvaluate(input, {
          ...runtime,
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
        } as any);
        satHtml = normalizeHtml(
          renderToStaticMarkup(createElement(SatComponent as any, { components: jsxComponents })),
        );
      } catch {
        satError = true;
      }

      if (refError && satError) return true;

      if (refError !== satError) {
        issues.push({
          input,
          source,
          kind: satError ? "satteri-error" : "both-error-disagree",
          referenceHtml: refHtml,
          satteriHtml: satHtml,
          error: satError
            ? "satteri threw but @mdx-js/mdx succeeded"
            : "@mdx-js/mdx threw but satteri succeeded",
        });
        return true;
      }

      if (refHtml !== satHtml) {
        issues.push({
          input,
          source,
          kind: "mismatch",
          referenceHtml: refHtml,
          satteriHtml: satHtml,
        });
      }

      return true;
    }),
    FC_OPTIONS,
  );

  return issues;
}

function deduplicateMdxEvalIssues(issues: MdxEvalIssue[]): MdxEvalIssue[] {
  const seen = new Map<string, MdxEvalIssue>();
  for (const issue of issues) {
    const key = `${issue.kind}:${issue.referenceHtml ?? ""}:${issue.satteriHtml ?? ""}`;
    if (!seen.has(key) || issue.input.length < seen.get(key)!.input.length) {
      seen.set(key, issue);
    }
  }
  return [...seen.values()];
}

function formatMdxEvalIssue(issue: MdxEvalIssue, index: number): string {
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

describe("fuzz: no crashes", () => {
  test("MDAST: no crash on arbitrary input", () => {
    fc.assert(
      fc.property(chaosString, (input) => {
        markdownToMdast(input);
      }),
      FC_OPTIONS,
    );
  });

  test("HAST: no crash on arbitrary input", () => {
    fc.assert(
      fc.property(chaosString, (input) => {
        markdownToHast(input);
      }),
      FC_OPTIONS,
    );
  });
});

describe("fuzz: conformance", () => {
  test("collect and report all issues", () => {
    const allIssues = [
      ...collectIssues(markdownDocument, "mdast", "structured"),
      ...collectIssues(markdownDocument, "hast", "structured"),
      ...collectIssues(markdownDocument, "html", "structured"),
      ...collectIssues(chaosString, "mdast", "chaos"),
      ...collectIssues(chaosString, "hast", "chaos"),
      ...collectIssues(chaosString, "html", "chaos"),
      ...collectIssues(mdxDocument, "mdx-mdast", "structured"),
      ...collectIssues(mdxDocument, "mdx-hast", "structured"),
    ];

    const unique = deduplicateIssues(allIssues);

    if (unique.length > 0) {
      const report = [
        "# Fuzz-discovered conformance issues",
        "",
        `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatIssue),
      ].join("\n");

      const issuesPath = new URL("./FUZZ-ISSUES.md", import.meta.url);
      writeFileSync(issuesPath, report + "\n");

      const inputs = unique.map((i) => JSON.stringify(i.input));
      expect
        .soft(unique, `Found ${unique.length} conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    }
  });
});

describe("fuzz: math conformance", () => {
  test("collect and report math issues", () => {
    const allIssues = [
      ...collectIssues(mathDocument, "math-mdast", "structured"),
      ...collectIssues(mathDocument, "math-hast", "structured"),
      ...collectIssues(mathDocument, "math-html", "structured"),
    ];

    const unique = deduplicateIssues(allIssues);

    if (unique.length > 0) {
      const report = [
        "# Math fuzz-discovered conformance issues",
        "",
        `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatIssue),
      ].join("\n");

      const issuesPath = new URL("./FUZZ-ISSUES-MATH.md", import.meta.url);
      writeFileSync(issuesPath, report + "\n");

      const inputs = unique.map((i) => JSON.stringify(i.input));
      expect
        .soft(unique, `Found ${unique.length} math conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    }
  });
});

describe("fuzz: frontmatter conformance", () => {
  test("collect and report frontmatter issues", () => {
    const allIssues = [
      ...collectIssues(fmDocument, "fm-mdast", "structured"),
      ...collectIssues(fmDocument, "fm-hast", "structured"),
      ...collectIssues(fmDocument, "fm-html", "structured"),
    ];

    const unique = deduplicateIssues(allIssues);

    if (unique.length > 0) {
      const report = [
        "# Frontmatter fuzz-discovered conformance issues",
        "",
        `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatIssue),
      ].join("\n");

      const issuesPath = new URL("./FUZZ-ISSUES-FM.md", import.meta.url);
      writeFileSync(issuesPath, report + "\n");

      const inputs = unique.map((i) => JSON.stringify(i.input));
      expect
        .soft(
          unique,
          `Found ${unique.length} frontmatter conformance issue(s):\n${inputs.join("\n")}`,
        )
        .toHaveLength(0);
    }
  });
});

describe("fuzz: MDX no crashes", () => {
  test("MDX: no crash on structured input", async () => {
    await fc.assert(
      fc.asyncProperty(mdxDocument, async (input) => {
        try {
          const { default: Component } = await satteriEvaluate(input, {
            ...runtime,
          } as any);
          renderToStaticMarkup(createElement(Component as any, { components: jsxComponents }));
        } catch {
          // compile errors are fine, crashes are not
        }
      }),
      FC_OPTIONS,
    );
  });
});

describe("fuzz: MDX eval conformance", () => {
  test("collect and report MDX eval issues", async () => {
    const allIssues = await collectMdxEvalIssues(mdxDocument, "structured");
    const unique = deduplicateMdxEvalIssues(allIssues);

    if (unique.length > 0) {
      const report = [
        "# MDX fuzz-discovered conformance issues",
        "",
        `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatMdxEvalIssue),
      ].join("\n");

      const issuesPath = new URL("./FUZZ-ISSUES-MDX.md", import.meta.url);
      writeFileSync(issuesPath, report + "\n");

      const inputs = unique.map((i) => JSON.stringify(i.input));
      expect
        .soft(unique, `Found ${unique.length} MDX conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    }
  });
});
