import fc from "fast-check";
import { readFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { expect } from "vitest";
import { mdxToMdast, mdxToHast, evaluate as satteriEvaluate } from "../../../src/index.js";
import { evaluate as mdxEvaluate } from "@mdx-js/mdx";
import { remark } from "remark";
import remarkMdx from "remark-mdx";
import remarkGfm from "remark-gfm";
import { toHast } from "mdast-util-to-hast";
import type { Root as MdastRoot, Nodes as MdastNodes } from "mdast";
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

// Set FUZZ_SEED to reproduce a previous failing run. Set VITEST_QUIET to
// suppress the seed log.
const FUZZ_SEED = Number(process.env.FUZZ_SEED) || Date.now();
if (!process.env.VITEST_QUIET) {
  console.log(`[fuzz] seed=${FUZZ_SEED}`);
}

// Wall-clock cap for each fuzz test. Vitest's default is 5s, which large
// `FUZZ_RUNS` values (e.g. 1_000_000 → ~12 min) blow past — and the timeout
// then masks the assertion failure that actually matters. The work here is
// bounded by `NUM_RUNS`, not this cap; we just need it generous enough for
// the largest practical run.
export const FUZZ_TIMEOUT_MS = 60 * 60 * 1000;

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

const MD_SIGNIFICANT_CHARS = "# *_~`[]()!<>|-\\{}@^+=$:/ \t\n".split("");
const ALNUM = "abcdefghijklmnopqrstuvwxyz 0123456789".split("");

// Spec-seeded arbitraries
//
// Pure random small inputs miss interactions between features (links inside
// blockquotes inside lists, tight/loose list edge cases, fenced code with
// info strings, …). Seeding fast-check with the CommonMark spec examples
// means each fuzz draw can start from a realistic, complex input and (via
// the mutator) explore variants of it. The reference parsers already
// handle these inputs verbatim, so any divergence we find is a real bug
// rather than noise from synthetic chaos.
const SPEC_DIR = fileURLToPath(
  new URL("../../../../../crates/satteri-pulldown-cmark/third_party/", import.meta.url),
);

function loadSpecMarkdown(relPath: string): string[] {
  try {
    const cases = JSON.parse(readFileSync(`${SPEC_DIR}${relPath}`, "utf8")) as {
      markdown: string;
    }[];
    return cases.map((c) => c.markdown);
  } catch {
    return [];
  }
}

const COMMONMARK_EXAMPLES = loadSpecMarkdown("CommonMark/spec.json");

/** A single CommonMark spec example, drawn at random. */
export const commonmarkExample =
  COMMONMARK_EXAMPLES.length > 0 ? fc.constantFrom(...COMMONMARK_EXAMPLES) : fc.constant("");

/**
 * A CommonMark spec example with 0–N small mutations applied: random
 * character insertions/deletions/substitutions, or splicing in a slice of
 * another example. Generates inputs that look like real markdown but
 * exercise edge cases the spec doesn't cover directly.
 */
export const mutatedCommonmarkExample = fc
  .tuple(
    commonmarkExample,
    fc.array(
      fc.record({
        op: fc.constantFrom(
          "insert" as const,
          "delete" as const,
          "replace" as const,
          "splice" as const,
        ),
        // Position offset (0..1, scaled to string length at apply time).
        pos: fc.double({ min: 0, max: 1, noNaN: true, noDefaultInfinity: true }),
        // For insert/replace: a small bit of MD-significant text.
        chunk: fc.string({
          unit: fc.constantFrom(...MD_SIGNIFICANT_CHARS, ...ALNUM),
          minLength: 1,
          maxLength: 6,
        }),
        // For splice: another spec example to weave in.
        other: commonmarkExample,
      }),
      { minLength: 0, maxLength: 4 },
    ),
  )
  .map(([base, mutations]) => {
    let s = base;
    for (const m of mutations) {
      if (s.length === 0) {
        s = m.chunk;
        continue;
      }
      const i = Math.min(s.length, Math.floor(m.pos * (s.length + 1)));
      switch (m.op) {
        case "insert":
          s = s.slice(0, i) + m.chunk + s.slice(i);
          break;
        case "delete":
          s = s.slice(0, i) + s.slice(Math.min(s.length, i + m.chunk.length));
          break;
        case "replace":
          s = s.slice(0, i) + m.chunk + s.slice(Math.min(s.length, i + m.chunk.length));
          break;
        case "splice": {
          const o = m.other;
          const cut = Math.min(o.length, Math.max(1, Math.floor(o.length * m.pos)));
          s = s.slice(0, i) + o.slice(0, cut) + s.slice(i);
          break;
        }
      }
    }
    return s;
  });

// Curated MDX examples covering the syntactic surface (expressions, JSX,
// ESM, comments, spreads, member/namespaced names, multi-line forms, mixed
// inline content). Authored rather than scraped because mdx-js's own
// fixtures aren't in a single load-friendly format, and a small focused
// set catches the relevant interactions.
const MDX_EXAMPLES: string[] = [
  // Inline expressions
  "{1 + 2}",
  "value: {1 + 2}",
  "{`hello ${name}`}",
  "{ /a/.test('abc') ? 'yes' : 'no' }",
  "{ true ? 'a' : 'b' }",
  "{(() => { const o = {key: 'value'}; return o.key })()}",
  "{/* comment */}",
  "{/* one */ /* two */ x}",
  "{1 +\n2}",
  // Flow expressions (own line)
  "before\n\n{1 + 2}\n\nafter",
  // JSX inline
  "<Foo/>",
  "<Foo bar={1}/>",
  '<Foo bar={1} baz="two"/>',
  '<Tag label="hello"/>',
  "<Box>hello</Box>",
  "<Box>{1 + 2}</Box>",
  "<>hello</>",
  "<>{1 + 2}</>",
  "<Check disabled/>",
  "<Foo $bar/>",
  // Member / namespaced names
  "<Ui.Button>Click</Ui.Button>",
  "<svg:circle/>",
  // Spread
  "<Tag {...props}/>",
  "<Tag {...{x: 'hi'}}/>",
  // Multiline JSX
  '<Foo\n  bar={1}\n  baz="two"\n/>',
  "<Box>\n  child\n</Box>",
  "<Box>\n  - a list\n  - inside\n</Box>",
  // Self-closing with newline
  "<Foo/>\n",
  "<br/>",
  // Mixed inline (expressions are self-contained so render-time eval
  // exercises real output rather than tripping on undefined identifiers).
  "before <Foo/> after",
  "before {1 + 2} after",
  "before <Foo {...{x: 1}}/> after {42}",
  // Link with expression in text
  '[{"label"}](url)',
  '[hello {"name"}](url)',
  // Image with expression body in alt
  "![{1+2}](u)",
  // Multiple consecutive
  "<Foo/><Bar/><Box>c</Box>",
  "{1}{2}{3}",
  // ESM (`export const` so we don't depend on a resolvable module).
  "export const y = 1\n\n{y}",
  // Mixed with markdown
  '# Heading with {1 + 2}\n\n- list with <Foo/>\n- and {"other"}',
  "> blockquote with {1 + 2}",
  '**bold {"x"} bold**',
  '`code` and {"x"}',
  // Children with markdown
  "<Box>\n  # heading inside\n\n  paragraph inside\n</Box>",
  // Common patterns. Uses already-provided `Box`/`Tag` components so the
  // seed exercises eval rather than tripping on module resolution.
  '<Box>\n  <Tag name="a">first</Tag>\n  <Tag name="b">second</Tag>\n</Box>',
];

/** A single curated MDX example. */
export const mdxExample =
  MDX_EXAMPLES.length > 0 ? fc.constantFrom(...MDX_EXAMPLES) : fc.constant("");

export const mutatedMdxExample = fc
  .tuple(
    mdxExample,
    fc.array(
      fc.record({
        op: fc.constantFrom(
          "insert" as const,
          "delete" as const,
          "replace" as const,
          "splice" as const,
        ),
        pos: fc.double({ min: 0, max: 1, noNaN: true, noDefaultInfinity: true }),
        // Bias toward MDX-significant chars so mutations stress JSX/expr
        // edges (broken tags, bare braces, etc.).
        chunk: fc.string({
          unit: fc.constantFrom("<", ">", "{", "}", "/", "=", '"', "'", " ", "\n", ...ALNUM),
          minLength: 1,
          maxLength: 6,
        }),
        other: fc.oneof(mdxExample, commonmarkExample),
      }),
      { minLength: 0, maxLength: 4 },
    ),
  )
  .map(([base, mutations]) => {
    let s = base;
    for (const m of mutations) {
      if (s.length === 0) {
        s = m.chunk;
        continue;
      }
      const i = Math.min(s.length, Math.floor(m.pos * (s.length + 1)));
      switch (m.op) {
        case "insert":
          s = s.slice(0, i) + m.chunk + s.slice(i);
          break;
        case "delete":
          s = s.slice(0, i) + s.slice(Math.min(s.length, i + m.chunk.length));
          break;
        case "replace":
          s = s.slice(0, i) + m.chunk + s.slice(Math.min(s.length, i + m.chunk.length));
          break;
        case "splice": {
          const o = m.other;
          const cut = Math.min(o.length, Math.max(1, Math.floor(o.length * m.pos)));
          s = s.slice(0, i) + o.slice(0, cut) + s.slice(i);
          break;
        }
      }
    }
    return s;
  });

// Curated frontmatter examples covering YAML and TOML, simple and
// nested values, edge cases (empty body, missing close, mixed delimiters,
// content immediately after the close fence, frontmatter in the wrong
// position, etc.).
const FRONTMATTER_EXAMPLES: string[] = [
  // Simple YAML
  "---\ntitle: Hello\n---\n",
  "---\ntitle: Hello\nauthor: Erika\n---\n\nbody",
  "---\n---\n\nbody",
  // YAML with various value types
  "---\nnum: 42\nbool: true\nlist:\n  - a\n  - b\nmap:\n  k: v\n---\n",
  "---\nmulti: |\n  line one\n  line two\n---\n",
  '---\ntitle: "With: colon"\n---\n',
  "---\ndate: 2024-01-15\n---\n",
  // Simple TOML
  '+++\ntitle = "Hello"\n+++\n',
  '+++\ntitle = "Hello"\nauthor = "Erika"\n+++\n\nbody',
  "+++\n+++\n\nbody",
  '+++\nnum = 42\nbool = true\nlist = ["a", "b"]\n[map]\nk = "v"\n+++\n',
  // Adjacent content
  "---\ntitle: t\n---\n# heading right after",
  '+++\ntitle = "t"\n+++\n# heading right after',
  // Edge cases
  "---\n", // unclosed
  "+++\n", // unclosed
  "---\nbroken\n", // unclosed with content
  "---\nkey: value\n", // unclosed with body
  "---\n - not a list at start\n---\n", // weird YAML
  "---\nkey: value\n+++\n", // mixed delimiters
  '+++\nkey = "value"\n---\n',
  // Frontmatter in wrong position (must be first)
  "# heading\n\n---\nkey: value\n---\n", // not at start → not frontmatter
  " ---\nkey: value\n---\n", // indented opener
  // With surrounding whitespace
  "---\n  title: Hello  \n---\n",
];

/** A single curated frontmatter example. */
export const frontmatterExample =
  FRONTMATTER_EXAMPLES.length > 0 ? fc.constantFrom(...FRONTMATTER_EXAMPLES) : fc.constant("");

const generatedMarkdownDocument = fc
  .array(markdownBlock, { minLength: 1, maxLength: 12 })
  .map((blocks) => blocks.join("\n\n"));

// Mix synthetic markdown with spec examples and mutated spec examples.
// The weights bias toward spec-seeded inputs, since pure synthetic fuzz
// already had thousands of iterations of coverage and the spec examples
// are where most realistic complexity lives.
export const markdownDocument = fc.oneof(
  { weight: 1, arbitrary: generatedMarkdownDocument },
  { weight: 2, arbitrary: commonmarkExample },
  { weight: 2, arbitrary: mutatedCommonmarkExample },
);

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
const MDX_PASS_THROUGH_NODES: Array<MdastNodes["type"]> = [
  "mdxJsxFlowElement",
  "mdxJsxTextElement",
  "mdxFlowExpression",
  "mdxTextExpression",
  "mdxjsEsm",
];

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
  // unified's `runSync` is typed to return a bare `Node`; the remark MDX
  // pipeline always yields a Root here.
  const mdast = mdxParser.runSync(mdxParser.parse(input)) as MdastRoot;
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
  Tag: (props: any) => createElement("span", null, `tag=${JSON.stringify(props)}`),
  Check: (props: any) => createElement("input", { type: "checkbox", ...props }),
  // `<Ui.Button/>` resolves via member access on the components map.
  Ui: {
    Button: (props: any) => createElement("button", null, props.children),
  } as unknown as Function,
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

const generatedMdxDocument = fc
  .array(mdxBlock, { minLength: 1, maxLength: 8 })
  .map((blocks) => blocks.join("\n\n"));

// MDX is a superset of CommonMark, so spec examples are valid input. Mix
// them in alongside MDX-specific examples (curated for JSX/expressions/ESM)
// and the synthetic blocks. Heavier weight on MDX-specific seeds since
// those exercise the parser surface that's actually unique to MDX.
export const mdxDocument = fc.oneof(
  { weight: 1, arbitrary: generatedMdxDocument },
  { weight: 2, arbitrary: commonmarkExample },
  { weight: 2, arbitrary: mutatedCommonmarkExample },
  { weight: 3, arbitrary: mdxExample },
  { weight: 3, arbitrary: mutatedMdxExample },
);

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

// Curated math examples. The synthetic generator only produces well-formed
// `$x$` / `$$x$$`. The curated set covers things it doesn't: pandoc's
// "no-digit-after-$" rule (so `$5 and $10` isn't math), escaped dollars,
// unclosed delimiters, empty bodies, multi-line display math, math touching
// word boundaries, math nested in lists/blockquotes/tables/headings.
const MATH_EXAMPLES: string[] = [
  // Inline basics
  "$x$",
  "$x = 1$",
  "$a + b$",
  "$\\alpha$",
  "$\\frac{a}{b}$",
  "$\\sqrt{x^2 + y^2}$",
  "$\\sum_{i=0}^{n} i$",
  "$\\int_0^1 f(x)\\, dx$",
  // Display math
  "$$x$$",
  "$$x = 1$$",
  "$$\nx = 1\n$$",
  "$$\n\\frac{a}{b}\n$$",
  "$$\n\\begin{matrix}\n  a & b \\\\\n  c & d\n\\end{matrix}\n$$",
  "$$\n\\begin{aligned}\n  x &= 1 \\\\\n  y &= 2\n\\end{aligned}\n$$",
  // Pandoc dollar-as-currency rule
  "It costs $5 and $10.",
  "$5 + $10 = $15",
  "Worth $1,000 today.",
  // Escaped dollars
  "Use \\$ for currency, $x$ for math.",
  "Plain text \\$5 and math $5x$.",
  // Math touching word boundaries
  "before$x$after",
  "($x$)",
  "[$x$]",
  // Empty / odd
  "$$",
  "$$$$",
  // Unclosed
  "$x",
  "$x = 1",
  "$$\nx = 1",
  "$x and $y", // two opens with content between
  // Math with `$` in body via escapes / commands
  "$\\$$",
  "$x \\text{ for } \\$y$",
  // Math in markdown contexts
  "# Heading with $x$ math",
  "## $E = mc^2$",
  "- list item $x$\n- another $y$",
  "1. ordered $a$\n2. items $b$",
  "> blockquote with $\\sum_i x_i$",
  "> $$\n> x = 1\n> $$",
  "| col | val |\n| --- | --- |\n| a   | $x$ |",
  // Mixed inline + display
  "Define $f(x)$ as:\n\n$$\nf(x) = x^2\n$$\n\nThen $f(2) = 4$.",
  // Multi-paragraph display
  "First paragraph with $a$.\n\n$$\n\\int_0^\\infty e^{-x^2}\\, dx = \\frac{\\sqrt\\pi}{2}\n$$\n\nSecond paragraph with $b$.",
  // Math with subscripts/superscripts
  "$a_i$",
  "$x^2$",
  "$x_i^2$",
  "$\\sum_{i=1}^{n} x_i^2$",
  // Math with matrices/vectors
  "$\\vec{v}$",
  "$\\mathbf{A}$",
  // Math with fractions / nested
  "$\\frac{1}{1 + \\frac{1}{x}}$",
  "$\\binom{n}{k}$",
  // Common identities
  "$e^{i\\pi} + 1 = 0$",
  "$\\cos^2\\theta + \\sin^2\\theta = 1$",
  // Spaces around delimiters (CommonMark-significant)
  "$ x $",
  "$$ x $$",
  // Newlines in inline math (illegal — should not parse as math)
  "$x\ny$",
];

/** A single curated math example. */
export const mathExample =
  MATH_EXAMPLES.length > 0 ? fc.constantFrom(...MATH_EXAMPLES) : fc.constant("");

export const mutatedMathExample = fc
  .tuple(
    mathExample,
    fc.array(
      fc.record({
        op: fc.constantFrom(
          "insert" as const,
          "delete" as const,
          "replace" as const,
          "splice" as const,
        ),
        pos: fc.double({ min: 0, max: 1, noNaN: true, noDefaultInfinity: true }),
        // Bias toward math-significant chars so mutations stress `$`/`\`/
        // brace boundaries.
        chunk: fc.string({
          unit: fc.constantFrom("$", "\\", "{", "}", "_", "^", " ", "\n", ...ALNUM),
          minLength: 1,
          maxLength: 6,
        }),
        other: fc.oneof(mathExample, commonmarkExample),
      }),
      { minLength: 0, maxLength: 4 },
    ),
  )
  .map(([base, mutations]) => {
    let s = base;
    for (const m of mutations) {
      if (s.length === 0) {
        s = m.chunk;
        continue;
      }
      const i = Math.min(s.length, Math.floor(m.pos * (s.length + 1)));
      switch (m.op) {
        case "insert":
          s = s.slice(0, i) + m.chunk + s.slice(i);
          break;
        case "delete":
          s = s.slice(0, i) + s.slice(Math.min(s.length, i + m.chunk.length));
          break;
        case "replace":
          s = s.slice(0, i) + m.chunk + s.slice(Math.min(s.length, i + m.chunk.length));
          break;
        case "splice": {
          const o = m.other;
          const cut = Math.min(o.length, Math.max(1, Math.floor(o.length * m.pos)));
          s = s.slice(0, i) + o.slice(0, cut) + s.slice(i);
          break;
        }
      }
    }
    return s;
  });

const generatedMathDocument = fc
  .array(mathBlock, { minLength: 1, maxLength: 10 })
  .map((blocks) => blocks.join("\n\n"));

export const mathDocument = fc.oneof(
  { weight: 1, arbitrary: generatedMathDocument },
  { weight: 2, arbitrary: mathExample },
  { weight: 2, arbitrary: mutatedMathExample },
);

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

const generatedFmDocument = fc
  .tuple(
    fc.oneof(yamlFrontmatter, tomlFrontmatter),
    fc.array(markdownBlock, { minLength: 0, maxLength: 8 }),
  )
  .map(([fm, blocks]) => (blocks.length > 0 ? `${fm}\n\n${blocks.join("\n\n")}` : fm));

// A curated frontmatter example optionally followed by markdown content.
// Mixing the curated edge cases (unclosed fences, mixed delimiters,
// frontmatter-in-wrong-position) is the main value here — synthetic
// generation only produces well-formed frontmatter.
const seededFmDocument = fc
  .tuple(frontmatterExample, fc.array(markdownBlock, { minLength: 0, maxLength: 4 }))
  .map(([fm, blocks]) => (blocks.length > 0 ? `${fm}\n${blocks.join("\n\n")}` : fm));

export const fmDocument = fc.oneof(
  { weight: 1, arbitrary: generatedFmDocument },
  { weight: 2, arbitrary: frontmatterExample },
  { weight: 2, arbitrary: seededFmDocument },
);

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

export type FuzzSource = "structured" | "chaos";

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
  // Same family: failed TOML detection (`+++` + trailing whitespace, no
  // closing fence) leaves the following line as a paragraph in remark
  // instead of opening a list.
  "fm-mdast\0+++\t\n+ -",
  "fm-hast\0+++\t\n+ -",
  "fm-html\0+++\t\n+ -",
  // Same family: `---` followed by a list marker line in remark-frontmatter
  // suppresses list recognition.
  "fm-mdast\0---\n- --:[",
  "fm-hast\0---\n- --:[",
  "fm-html\0---\n- --:[",
  // Cosmetic divergence: tab-indented table-cell continuation differs in
  // some position-only fields that aren't caught by the position-strip
  // classifier. Tree shape (rows, cells, alignment, text) matches.
  "fm-mdast\0+-:\n:-\n\t:p",
  "fm-hast\0+-:\n:-\n\t:p",
  "fm-html\0+-:\n:-\n\t:p",
  // Intentional design divergence: Sätteri represents table-cell alignment
  // as `style="text-align: …"` while remark-rehype uses the deprecated
  // `align="…"` attribute. Both render correctly, but the hast properties
  // differ.
  "mdx-hast\0x7} >=>\n-:",
  // Same family: oxc accepts an expression body shape that acorn rejects.
  // After tightening `try_parse_expression_body` to acorn-style strictness
  // most cases are caught, but a few edge cases (e.g. unmatched braces in
  // regex-vs-division contexts) still slip through.
  // (oxc-vs-acorn expression-body divergences are now handled by
  // isMdxOxcAcornRegexDivergence.)
  // Same family as `+++\t\n+ -`: remark-frontmatter "poisoned line 1"
  // suppresses list detection after a `+++` opener that doesn't close.
  "fm-mdast\0+++\n- +i}(",
  "fm-hast\0+++\n- +i}(",
  "fm-html\0+++\n- +i}(",
]);

// mdx-js enforces strict flow JSX scoping rules that Sätteri's pairing
// pass treats more leniently. Two known patterns:
//
// 1. Trailing non-whitespace after a flow close tag (e.g. `</Box>"`):
//    mdx-js throws `end-tag-mismatch` because the paragraph swallows
//    the close tag as inline JSX, leaving the flow `<Box>` unclosed.
//    Sätteri pairs the close anyway and emits the trailing text as a
//    sibling.
//
// 2. Flow JSX that opens inside a container (blockquote / list item)
//    but whose close tag falls outside it (e.g. `><Box>\n- a\n</Box>`):
//    mdx-js throws "Expected a closing tag for `<Box>` before the end
//    of `blockQuote`". Sätteri pairs across the container boundary,
//    producing an empty `<Box>` followed by sibling content.
//
// Matching mdx-js exactly here would require cross-pass coordination
// between firstpass (which tokenizes flow JSX line-by-line) and
// arena_build (which performs open/close pairing).

// Intentional design divergence: Sätteri represents table-cell
// alignment as `style="text-align: <align>"` while remark-rehype emits
// the deprecated `align="<align>"` attribute. The trees differ in the
// `properties` of `td` / `th` elements but render the same. Filter
// any hast-level diff where the only meaningful difference matches
// that swap.
function isAlignAttributeDivergence(
  _input: string,
  level: FuzzLevel,
  actual: unknown,
  expected: unknown,
): boolean {
  if (!HAST_LIKE_LEVELS.has(level)) return false;
  if (typeof actual !== "object" || actual === null) return false;
  if (typeof expected !== "object" || expected === null) return false;
  const a = JSON.stringify(stripPositions(normalizeAlignProps(actual)));
  const e = JSON.stringify(stripPositions(normalizeAlignProps(expected)));
  return a === e;
}

const HAST_LIKE_LEVELS = new Set<FuzzLevel>(["hast", "mdx-hast", "math-hast", "fm-hast"]);

function normalizeAlignProps(node: unknown): unknown {
  if (typeof node !== "object" || node === null) return node;
  if (Array.isArray(node)) return node.map(normalizeAlignProps);
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(node as Record<string, unknown>)) {
    if (k === "properties" && v && typeof v === "object") {
      const props = v as Record<string, unknown>;
      const next: Record<string, unknown> = {};
      let align: string | undefined;
      for (const [pk, pv] of Object.entries(props)) {
        if (pk === "align" && typeof pv === "string") {
          align = pv;
        } else if (pk === "style" && typeof pv === "string" && /^text-align:\s*\w+;?$/.test(pv)) {
          align = pv.replace(/^text-align:\s*/, "").replace(/;$/, "");
        } else {
          next[pk] = pv;
        }
      }
      if (align !== undefined) next["__align"] = align;
      out[k] = next;
    } else if (typeof v === "object" && v !== null) {
      out[k] = normalizeAlignProps(v);
    } else {
      out[k] = v;
    }
  }
  return out;
}

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
  let refError: string | null = null;
  let actualError: string | null = null;
  try {
    actual = parse(input);
  } catch (e: any) {
    actual = "PARSE_ERROR";
    actualError = String(e?.message ?? e ?? "");
  }
  try {
    expected = ref(input);
  } catch (e: any) {
    expected = "PARSE_ERROR";
    refError = String(e?.message ?? e ?? "");
  }
  // Surface satteri internal crashes when both sides error — otherwise
  // panics hide behind the both-PARSE_ERROR agreement.
  if (
    actual === "PARSE_ERROR" &&
    expected === "PARSE_ERROR" &&
    actualError &&
    !/^\d+:\d+: /.test(actualError)
  ) {
    return {
      input,
      level,
      source,
      kind: "content",
      expected,
      actual: `INTERNAL_ERROR: ${actualError}`,
    };
  }
  try {
    expect(actual).toEqual(expected);
    return null;
  } catch {
    // Reference-bug filter for the frontmatter suite. remark-frontmatter has
    // a known issue where loading it changes how non-frontmatter content
    // gets tokenized after a `---`/`+++` line that isn't a real frontmatter
    // block (e.g. `---\n\n- a\n- b` becomes a paragraph instead of a list).
    // When the reference produces no actual frontmatter node and our output
    // matches the no-frontmatter baseline, the divergence is the reference's
    // fault, not ours.
    if (isFrontmatterReferenceBug(input, level, actual, expected)) {
      return null;
    }
    if (isMdxOxcAcornRegexDivergence(input, level, actual, expected, refError)) {
      return null;
    }
    if (isMdxStrictScannerDivergence(input, level, actual, expected, refError)) {
      return null;
    }
    if (isStrikethroughPhaseOrderingDivergence(input, level, actual, expected)) {
      return null;
    }
    if (isAlignAttributeDivergence(input, level, actual, expected)) {
      return null;
    }
    return { input, level, source, kind: classifyKind(level, actual, expected), expected, actual };
  }
}

function isFrontmatterReferenceBug(
  input: string,
  level: FuzzLevel,
  actual: unknown,
  expected: unknown,
): boolean {
  if (level !== "fm-mdast" && level !== "fm-hast" && level !== "fm-html") return false;
  // If the reference recognised frontmatter (its mdast contains a yaml/toml
  // node), the divergence is real and we should report it.
  if (referenceContainsFrontmatter(expected)) return false;
  // Otherwise, fall back to the no-frontmatter baseline reference. If we
  // match it, the difference is purely remark-frontmatter affecting
  // non-frontmatter content.
  let baseline: unknown;
  try {
    if (level === "fm-mdast") baseline = referenceMdast(input);
    else if (level === "fm-hast") baseline = referenceHast(input);
    else baseline = referenceHtml(input);
  } catch {
    baseline = "PARSE_ERROR";
  }
  try {
    expect(actual).toEqual(baseline);
    return true;
  } catch {
    return false;
  }
}

// Narrow classifier for the remaining MDX strict-scanner edge cases that
// the inline-JSX / inline-expression scanners don't fully reproduce. These
// stem from satteri's inline scanners not propagating container_check on
// every newline-crossing call: a `<Foo\n  bar={1}/>` tag inside a
// blockquote continues without `>` prefixes, which mdx-js rejects
// ("Unexpected lazy line in container"). Similar story for `<j/\n>`
// crossing a container marker, which mdx-js reports as a self-closing
// slash error. Threading container_check through the inline scanners
// would fix it (see §J for the larger inline-resolve rewrite).
function isMdxStrictScannerDivergence(
  _input: string,
  level: FuzzLevel,
  actual: unknown,
  expected: unknown,
  refError: string | null,
): boolean {
  if (level !== "mdx-mdast" && level !== "mdx-hast") return false;
  if (expected !== "PARSE_ERROR") return false;
  if (typeof actual !== "object" || actual === null) return false;
  if (!refError) return false;
  if (refError.includes("Unexpected lazy line in container")) return true;
  if (refError.includes("Unexpected lazy line in expression in container")) return true;
  if (refError.includes("after self-closing slash")) return true;
  if (refError.includes("Unexpected end of file before name")) return true;
  if (refError.includes("Unexpected character `!`")) return true;
  if (refError.includes("Unexpected character `?`")) return true;
  // mdx-js rejects an unclosed JSX fragment (`<>`) or named flow element
  // when satteri silently drops/recovers.
  if (refError.includes("Expected a closing tag for")) return true;
  if (refError.includes("Expected the closing tag")) return true;
  // mdx-js's expression-body scanner rejects a `<` followed by a name
  // char that doesn't form a valid JSX tag (`{a <foo}` etc.).
  if (/Unexpected character `.+?`(?: \(U\+[0-9A-Fa-f]+\))? (?:in name|before name)/.test(refError))
    return true;
  // mdx-js's `{` scanner reaches across block boundaries (a blockquote
  // interruption or a code-span closing backtick) looking for the matching
  // `}`. Satteri respects block-level interrupts and tokenizes the `{`
  // as text or as the body of a code span. Catch the resulting
  // "Unexpected end of file in expression" mismatch when satteri's
  // output has a code span / inline expression that contains the brace.
  if (refError.includes("Unexpected end of file in expression")) {
    if (treeContainsCodeSpanWithBraces(actual)) return true;
    if (treeContainsTextWithBraces(actual)) return true;
    // mdx-js's `{` scanner requires container_check on every line — a lazy
    // continuation inside a blockquote (no `>` prefix on a later line) is
    // rejected. Satteri accepts lazy continuation, so the expression body
    // spans multiple lines. Classify when our tree contains a multi-line
    // mdxFlow/TextExpression body.
    if (treeContainsMultilineMdxExpression(actual)) return true;
    // mdx-js's `{` scan also fires when an unclosed `{` ends up inside a
    // code span. Satteri's inline parser resolved the code span first, so
    // the `{` ended up wrapped in `` `…` ``. The closing `}` need not be
    // present (and often isn't, since that's why the reference errored).
    if (treeContainsCodeSpanWithOpenBrace(actual)) return true;
    // mdx-js's `{` scan also rejects refdef labels whose multi-line label
    // contains an unmatched `{` (e.g. `[d_5\n{oo]: /url "title"`). Satteri
    // accepts the refdef and the `{` ends up in the label string.
    if (treeContainsDefinitionLabelWithBraces(actual)) return true;
  }
  return false;
}

function treeContainsDefinitionLabelWithBraces(node: unknown): boolean {
  if (typeof node !== "object" || node === null) return false;
  const n = node as { type?: string; label?: unknown; children?: unknown[] };
  if (n.type === "definition" && typeof n.label === "string" && n.label.includes("{")) {
    return true;
  }
  if (Array.isArray(n.children)) {
    return n.children.some((c) => treeContainsDefinitionLabelWithBraces(c));
  }
  return false;
}

function treeContainsTextWithBraces(node: unknown): boolean {
  if (typeof node !== "object" || node === null) return false;
  const n = node as { type?: string; value?: unknown; children?: unknown[] };
  if (n.type === "text" && typeof n.value === "string" && n.value.includes("{")) {
    return true;
  }
  if (Array.isArray(n.children)) {
    return n.children.some((c) => treeContainsTextWithBraces(c));
  }
  return false;
}

function treeContainsMultilineMdxExpression(node: unknown): boolean {
  if (typeof node !== "object" || node === null) return false;
  const n = node as { type?: string; value?: unknown; children?: unknown[] };
  if (
    (n.type === "mdxFlowExpression" || n.type === "mdxTextExpression") &&
    typeof n.value === "string" &&
    n.value.includes("\n")
  ) {
    return true;
  }
  if (Array.isArray(n.children)) {
    return n.children.some((c) => treeContainsMultilineMdxExpression(c));
  }
  return false;
}

// `find_match`'s single-pass strikethrough/subscript phase-ordering rule
// refuses a `~…~` (or `^…^`) match when any `*`/`_` opener sits earlier on
// the stack — a proxy for "emphasis claims its pair first" (micromark
// resolves emphasis before tildes/circumflexes). The proxy is too broad:
// when the earlier `*`/`_` opener has no real closer, emphasis can't claim
// its pair, so strikethrough/subscript should still win. Two-pass resolve
// would handle this exactly; see §J in plans/mdx-conformance.md.
//
// Signal: input has `_` or `*` somewhere before a `~X~` or `^X^` pair; the
// reference produces a `delete`/`sub`/`sup` node (mdast) or `del`/`sub`/`sup`
// element (hast); satteri does not.
function isStrikethroughPhaseOrderingDivergence(
  input: string,
  level: FuzzLevel,
  actual: unknown,
  expected: unknown,
): boolean {
  // Strikethrough/subscript is GFM and runs at every fuzz level the suite
  // covers (mdast/hast/html and their mdx-/math-/fm- variants).
  void level;
  if (!/[_*][\s\S]*?[~^][\s\S]*?[~^]/.test(input)) return false;
  if (HTML_LEVELS.has(level)) {
    if (typeof actual !== "string" || typeof expected !== "string") return false;
    const refHasMark = /<(del|sub|sup)\b/.test(expected);
    const satHasMark = /<(del|sub|sup)\b/.test(actual);
    return refHasMark && !satHasMark;
  }
  if (typeof actual !== "object" || actual === null) return false;
  if (typeof expected !== "object" || expected === null) return false;
  const refHas = treeHasStrikeOrSubSup(expected);
  const satHas = treeHasStrikeOrSubSup(actual);
  return refHas && !satHas;
}

function treeHasStrikeOrSubSup(node: unknown): boolean {
  if (typeof node !== "object" || node === null) return false;
  const n = node as { type?: string; tagName?: string; children?: unknown[] };
  if (n.type === "delete" || n.type === "sub" || n.type === "sup") return true;
  if (n.type === "element" && (n.tagName === "del" || n.tagName === "sub" || n.tagName === "sup")) {
    return true;
  }
  if (Array.isArray(n.children)) return n.children.some((c) => treeHasStrikeOrSubSup(c));
  return false;
}

function referenceContainsFrontmatter(node: unknown): boolean {
  if (!node || typeof node !== "object") return false;
  const obj = node as { type?: string; children?: unknown[] };
  if (obj.type === "yaml" || obj.type === "toml") return true;
  if (Array.isArray(obj.children)) {
    for (const c of obj.children) if (referenceContainsFrontmatter(c)) return true;
  }
  return false;
}

// oxc accepts JS expression shapes that acorn (used by mdx-js) rejects.
// Most relevant: regex syntax validation (`/+/` is invalid because `+`
// has nothing to quantify) and ambiguous regex-vs-division parsing in
// rare lexer states. When the reference threw and our output is a
// single mdxFlowExpression / mdxTextExpression node, the divergence is
// almost certainly this oxc-vs-acorn split rather than a genuine
// structural bug. Tightening try_parse_expression_body to acorn's
// exact strictness would require swapping engines, so document it.
function isMdxOxcAcornRegexDivergence(
  _input: string,
  level: FuzzLevel,
  actual: unknown,
  expected: unknown,
  refError: string | null,
): boolean {
  if (level !== "mdx-mdast" && level !== "mdx-hast") return false;
  if (expected !== "PARSE_ERROR") return false;
  if (typeof actual !== "object" || actual === null) return false;
  if (!refError) return false;
  // Same root cause for two distinct acorn checks: expression-body
  // validation (`Could not parse expression with acorn`) and ESM
  // import/export validation (`Could not parse import/exports with
  // acorn`). oxc is more permissive in both cases — for instance, with
  // jsx enabled, oxc will accept `import X from 'x'<` (treating `<` as
  // a JSX-element opener), while acorn-with-acorn-jsx rejects it.
  if (refError.includes("Could not parse expression with acorn")) {
    // Direct case: satteri produced an mdxExpression node that acorn
    // would reject.
    if (treeContainsMdxExpression(actual)) return true;
    // Indirect case: mdx-js's `{` expression scan ran BEFORE code-span
    // resolution and claimed the `{...}` body (which acorn then failed
    // to parse). Satteri's inline parser resolved the code span first,
    // so the `{` content ended up wrapped in `\`...\`` instead. The
    // signal is a code span whose value contains `{` and `}` —
    // mdx-js's scan would have grabbed those braces as an expression.
    if (treeContainsCodeSpanWithBraces(actual)) return true;
  }
  if (refError.includes("Could not parse import/exports with acorn")) {
    return treeContainsMdxEsm(actual);
  }
  return false;
}

function treeContainsCodeSpanWithBraces(node: unknown): boolean {
  if (typeof node !== "object" || node === null) return false;
  const n = node as { type?: string; tagName?: string; value?: unknown; children?: unknown[] };
  if (
    (n.type === "inlineCode" || (n.type === "element" && n.tagName === "code")) &&
    typeof n.value === "string" &&
    n.value.includes("{") &&
    n.value.includes("}")
  ) {
    return true;
  }
  if (Array.isArray(n.children)) {
    return n.children.some((c) => treeContainsCodeSpanWithBraces(c));
  }
  return false;
}

function treeContainsCodeSpanWithOpenBrace(node: unknown): boolean {
  if (typeof node !== "object" || node === null) return false;
  const n = node as { type?: string; tagName?: string; value?: unknown; children?: unknown[] };
  if (
    (n.type === "inlineCode" || (n.type === "element" && n.tagName === "code")) &&
    typeof n.value === "string" &&
    n.value.includes("{")
  ) {
    return true;
  }
  if (Array.isArray(n.children)) {
    return n.children.some((c) => treeContainsCodeSpanWithOpenBrace(c));
  }
  return false;
}

function treeContainsMdxExpression(node: unknown): boolean {
  if (typeof node !== "object" || node === null) return false;
  const n = node as {
    type?: string;
    children?: unknown[];
    attributes?: unknown[];
  };
  if (n.type === "mdxFlowExpression" || n.type === "mdxTextExpression") return true;
  // JSX expression attributes are body-validated the same way.
  if (n.type === "mdxJsxExpressionAttribute") return true;
  if (Array.isArray(n.children) && n.children.some((c) => treeContainsMdxExpression(c))) {
    return true;
  }
  if (Array.isArray(n.attributes) && n.attributes.some((c) => treeContainsMdxExpression(c))) {
    return true;
  }
  return false;
}

function treeContainsMdxEsm(node: unknown): boolean {
  if (typeof node !== "object" || node === null) return false;
  const n = node as { type?: string; children?: unknown[] };
  if (n.type === "mdxjsEsm") return true;
  if (Array.isArray(n.children)) {
    return n.children.some((c) => treeContainsMdxEsm(c));
  }
  return false;
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
  source: "structured" | "chaos";
  kind: "mismatch" | "satteri-error" | "both-error-disagree";
  referenceHtml?: string | undefined;
  satteriHtml?: string | undefined;
  error?: string | undefined;
}

// MDX evaluation divergences we accept — the @mdx-js path rejects but
// satteri's parser+runtime evaluates successfully. Each entry corresponds
// to a documented divergence; lenient recovery vs strict rejection is a
// deliberate satteri design choice.
const KNOWN_MDX_EVAL_DIVERGENCES = new Set<string>([
  // Multi-line code span containing `<`: satteri pairs the backticks across
  // the newline and emits `<code>&lt;</code>`; mdx-js's tokenizer interprets
  // the `<` as a JSX tag start before the second backtick closes the span.
  // To fix in a follow-up pass.
  "`\n <`",
  // Same shape as the `-\n\n  2. b\n\n    3. c\n` case in FUZZ-ISSUES.md
  // (tracked as a pending md task): a top-level list-marker line followed
  // by a blank line and an indented continuation gets nested by satteri
  // and kept as siblings by mdx-js. Hit by the `*` variant too.
  "*\n\n  2. b\n\n    3. c\n",
]);

async function compareMdxEval(
  input: string,
  source: MdxEvalIssue["source"],
): Promise<MdxEvalIssue | null> {
  if (KNOWN_MDX_EVAL_DIVERGENCES.has(input)) return null;
  let refHtml: string | undefined;
  let refError = false;
  let refErrorMessage: string | null = null;
  try {
    const { default: RefComponent } = (await mdxEvaluate(input, {
      ...runtime,
      remarkPlugins: [remarkGfm],
    })) as { default: Function };
    refHtml = normalizeHtml(
      renderToStaticMarkup(createElement(RefComponent as any, { components: jsxComponents })),
    );
  } catch (e: any) {
    refError = true;
    refErrorMessage = String(e?.message ?? e ?? "");
  }

  let satHtml: string | undefined;
  let satError = false;
  let satErrorMessage: string | null = null;
  try {
    const { default: SatComponent } = await satteriEvaluate(input, {
      ...runtime,
      features: MDX_FEATURES,
    } as any);
    satHtml = normalizeHtml(
      renderToStaticMarkup(createElement(SatComponent as any, { components: jsxComponents })),
    );
  } catch (e: any) {
    satError = true;
    satErrorMessage = String(e?.message ?? e ?? "");
  }

  // If both sides threw, suppress only when satteri's error looks like a
  // legitimate rejection (parse error or a runtime exception from the
  // compiled component). Rust-side panics surface so they don't hide.
  if (refError && satError) {
    if (
      satErrorMessage &&
      /panic|unreachable|index out of bounds|unwrap\(\) on|RuntimeError/i.test(satErrorMessage)
    ) {
      return {
        input,
        source,
        kind: "satteri-error",
        referenceHtml: refHtml,
        satteriHtml: satHtml,
        error: `satteri internal error (both threw): ${satErrorMessage}`,
      };
    }
    return null;
  }

  if (refError !== satError) {
    // Mirror compareSingle's classifiers for the satteri-succeeds case:
    // when mdx-js rejects with a strict-scanner error (lazy line in
    // container / self-closing slash / unexpected EOF before name) but
    // satteri accepts, the divergence is the inline scanner not
    // propagating container_check across newlines. See §J.
    if (refError && !satError && refErrorMessage) {
      if (
        refErrorMessage.includes("Unexpected lazy line in container") ||
        refErrorMessage.includes("Unexpected lazy line in expression in container") ||
        refErrorMessage.includes("after self-closing slash") ||
        refErrorMessage.includes("Unexpected end of file before name") ||
        refErrorMessage.includes("Unexpected end of file in expression") ||
        refErrorMessage.includes("Unexpected character `!`") ||
        refErrorMessage.includes("Unexpected character `?`") ||
        refErrorMessage.includes("Expected a closing tag for") ||
        refErrorMessage.includes("Expected the closing tag") ||
        // Backtick code-span content where mdx-js's `<` expression scan
        // tries to treat the trailing backtick as a JSX name start. The
        // message may include `(U+0060)` between the backtick and the
        // "in name"/"before name" qualifier.
        /Unexpected character ``` .*?(?:in name|before name)/.test(refErrorMessage)
      ) {
        return null;
      }
    }
    return {
      input,
      source,
      kind: satError ? "satteri-error" : "both-error-disagree",
      referenceHtml: refHtml,
      satteriHtml: satHtml,
      error: satError
        ? `satteri threw but @mdx-js/mdx succeeded${satErrorMessage ? `: ${satErrorMessage}` : ""}`
        : "@mdx-js/mdx threw but satteri succeeded",
    };
  }

  if (refHtml !== satHtml) {
    // Strikethrough phase-ordering: the find_match single-pass rule may
    // reject a `~…~` match when an unmatched `*`/`_` opener exists earlier
    // on the stack. The reference's two-pass resolve catches this; satteri
    // doesn't yet (see §J).
    if (typeof refHtml === "string" && typeof satHtml === "string") {
      if (/[_*][\s\S]*?[~^][\s\S]*?[~^]/.test(input)) {
        const refHasMark = /<(del|sub|sup)\b/.test(refHtml);
        const satHasMark = /<(del|sub|sup)\b/.test(satHtml);
        if (refHasMark && !satHasMark) return null;
      }
    }
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
