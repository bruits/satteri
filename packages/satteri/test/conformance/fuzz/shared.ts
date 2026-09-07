import fc from "fast-check";
import { readFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { expect } from "vitest";
import { mdxToMdast, mdxToHast, evaluate as satteriEvaluate } from "../../../src/index.js";
import { evaluate as mdxEvaluate } from "@mdx-js/mdx";
import { remark } from "remark";
import remarkMdx from "remark-mdx";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
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
  reconcileFnrPositions,
  assertSliceInvariantEverywhere,
} from "../helpers.js";

const { remarkMarkAndUnravel } = await import(
  pathToFileURL("node_modules/@mdx-js/mdx/lib/plugin/remark-mark-and-unravel.js").href
);

export const NUM_RUNS = Number(process.env.FUZZ_RUNS) || 200;
// Compile-and-render fuzzing is costlier than parsing, so use a lower default run count.
export const NUM_RUNS_EVAL = Number(process.env.FUZZ_RUNS_EVAL) || 50;

const FUZZ_SEED = Number(process.env.FUZZ_SEED) || Date.now();
if (!process.env.VITEST_QUIET) {
  console.log(`[fuzz] seed=${FUZZ_SEED}`);
}

// Let FUZZ_RUNS bound the work; a timeout must not mask the conformance failure.
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

const paragraph = INLINE_TEXT;
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

const MD_SIGNIFICANT_CHARS = "# *_~`[]()!<>|-\\{}@^+=$:/ \t\n\r".split("");
const ALNUM = "abcdefghijklmnopqrstuvwxyz 0123456789".split("");

// Spec examples expose feature interactions that small random inputs rarely reach.
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

export const commonmarkExample =
  COMMONMARK_EXAMPLES.length > 0 ? fc.constantFrom(...COMMONMARK_EXAMPLES) : fc.constant("");

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
        pos: fc.double({ min: 0, max: 1, noNaN: true, noDefaultInfinity: true }),
        chunk: fc.string({
          unit: fc.constantFrom(...MD_SIGNIFICANT_CHARS, ...ALNUM),
          minLength: 1,
          maxLength: 6,
        }),
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

const MDX_EXAMPLES: string[] = [
  "{1 + 2}",
  "value: {1 + 2}",
  "{`hello ${name}`}",
  "{ /a/.test('abc') ? 'yes' : 'no' }",
  "{ true ? 'a' : 'b' }",
  "{(() => { const o = {key: 'value'}; return o.key })()}",
  "{/* comment */}",
  "{/* one */ /* two */ x}",
  "{1 +\n2}",
  "before\n\n{1 + 2}\n\nafter",
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
  "<Ui.Button>Click</Ui.Button>",
  "<svg:circle/>",
  "<Tag {...props}/>",
  "<Tag {...{x: 'hi'}}/>",
  '<Foo\n  bar={1}\n  baz="two"\n/>',
  "<Box>\n  child\n</Box>",
  "<Box>\n  - a list\n  - inside\n</Box>",
  "<Foo/>\n",
  "<br/>",
  "before <Foo/> after",
  "before {1 + 2} after",
  "before <Foo {...{x: 1}}/> after {42}",
  '[{"label"}](url)',
  '[hello {"name"}](url)',
  "![{1+2}](u)",
  "<Foo/><Bar/><Box>c</Box>",
  "{1}{2}{3}",
  // Export local bindings so evaluation does not depend on external module resolution.
  "export const y = 1\n\n{y}",
  '# Heading with {1 + 2}\n\n- list with <Foo/>\n- and {"other"}',
  "> blockquote with {1 + 2}",
  '**bold {"x"} bold**',
  '`code` and {"x"}',
  "<Box>\n  # heading inside\n\n  paragraph inside\n</Box>",
  // Use supplied components so fuzzing tests evaluation rather than module availability.
  '<Box>\n  <Tag name="a">first</Tag>\n  <Tag name="b">second</Tag>\n</Box>',
];

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

const FRONTMATTER_EXAMPLES: string[] = [
  "---\ntitle: Hello\n---\n",
  "---\ntitle: Hello\nauthor: Erika\n---\n\nbody",
  "---\n---\n\nbody",
  "---\nnum: 42\nbool: true\nlist:\n  - a\n  - b\nmap:\n  k: v\n---\n",
  "---\nmulti: |\n  line one\n  line two\n---\n",
  '---\ntitle: "With: colon"\n---\n',
  "---\ndate: 2024-01-15\n---\n",
  '+++\ntitle = "Hello"\n+++\n',
  '+++\ntitle = "Hello"\nauthor = "Erika"\n+++\n\nbody',
  "+++\n+++\n\nbody",
  '+++\nnum = 42\nbool = true\nlist = ["a", "b"]\n[map]\nk = "v"\n+++\n',
  "---\ntitle: t\n---\n# heading right after",
  '+++\ntitle = "t"\n+++\n# heading right after',
  "---\n",
  "+++\n",
  "---\nbroken\n",
  "---\nkey: value\n",
  "---\n - not a list at start\n---\n",
  "---\nkey: value\n+++\n",
  '+++\nkey = "value"\n---\n',
  "# heading\n\n---\nkey: value\n---\n",
  " ---\nkey: value\n---\n",
  "---\n  title: Hello  \n---\n",
];

export const frontmatterExample =
  FRONTMATTER_EXAMPLES.length > 0 ? fc.constantFrom(...FRONTMATTER_EXAMPLES) : fc.constant("");

const generatedMarkdownDocument = fc
  .array(markdownBlock, { minLength: 1, maxLength: 12 })
  .map((blocks) => blocks.join("\n\n"));

export const markdownDocument = fc.oneof(
  { weight: 1, arbitrary: generatedMarkdownDocument },
  { weight: 2, arbitrary: commonmarkExample },
  { weight: 2, arbitrary: mutatedCommonmarkExample },
);

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

const AL_SCHEME = fc.constantFrom(
  "www.",
  "http://",
  "https://",
  "HTTP://",
  "HTTPS://",
  "WWW.",
  "wWw.",
  "HtTpS://",
  "ftp://",
  "www",
  "mailto:",
  "",
);
const AL_DOMAIN_UNIT = fc.constantFrom(..."abXYZ019", "-", "_", ".", "點", "é");
const AL_PATH_UNIT = fc.constantFrom(..."ab019", ..."/()?#&=-_.[]~*!,:;'\"+%".split(""), "點");
const AL_TRAIL = fc.constantFrom(
  "",
  ".",
  ",",
  "!",
  "?",
  ":",
  ";",
  ")",
  "]",
  "}",
  ">",
  "<",
  "&",
  "].",
  "...",
  ")))",
  "&amp;",
  "&copy;",
  "&notreal",
  "&#104;",
  "&#x68;",
  "\\,",
  "\\<a>",
  "?!",
  "*_~",
  '">',
  ");",
  "''",
  ".)",
  "(a)",
);
const AL_PREV = fc.constantFrom(
  "",
  "",
  " ",
  "(",
  "[",
  "*",
  "_",
  "~",
  "a",
  "5",
  ".",
  "/",
  "@",
  "é",
  "點",
  ")",
  ">",
  "x",
  ":",
  "<",
  "!",
  "\\",
  "\\\\",
);
const AL_POST = fc.constantFrom("", "", " ", "\n", ")", "]", ".", "x", " end\n", ">", "\t", "!");

const autolinkUrl = fc
  .tuple(
    AL_SCHEME,
    fc.array(AL_DOMAIN_UNIT, { minLength: 1, maxLength: 14 }),
    fc.array(AL_PATH_UNIT, { minLength: 0, maxLength: 16 }),
  )
  .map(([scheme, dom, path]) => scheme + dom.join("") + path.join(""));

const EMAIL_LOCAL_UNIT = fc.constantFrom(..."ab019", ".", "+", "-", "_");
const EMAIL_DOMAIN_UNIT = fc.constantFrom(..."ab019", ".", "-", "_", "點");
const autolinkEmail = fc
  .tuple(
    fc.array(EMAIL_LOCAL_UNIT, { minLength: 1, maxLength: 8 }),
    fc.array(EMAIL_DOMAIN_UNIT, { minLength: 1, maxLength: 12 }),
  )
  .map(([local, dom]) => `${local.join("")}@${dom.join("")}`);

const autolinkLine = fc
  .tuple(AL_PREV, fc.oneof(autolinkUrl, autolinkUrl, autolinkEmail), AL_TRAIL, AL_POST)
  .map(([prev, core, trail, post]) => prev + core + trail + post);

export const autolinkDocument = fc
  .tuple(
    autolinkLine,
    fc.constantFrom(
      "plain",
      "plain",
      "bq",
      "bq0",
      "list",
      "label",
      "angle",
      "code",
      "para2",
      "img",
      "dest",
      "destUnresolved",
      "destReference",
    ),
  )
  .map(([line, ctx]) => {
    switch (ctx) {
      case "bq":
        return `> ${line}\n`;
      case "bq0":
        return `>${line}\n`;
      case "list":
        return `- ${line}\n`;
      case "label":
        return `[${line}](/x)\n`;
      case "angle":
        return `<${line}>\n`;
      case "code":
        return "`" + line + "`\n";
      case "para2":
        return `text\n${line}\n`;
      case "img":
        return `![${line}](/i)\n`;
      case "dest":
        return `[a](${line})x\n`;
      case "destUnresolved":
        return `[[x]](${line})x\n\n[x]: /\n`;
      case "destReference":
        return `[a][b](${line})x\n\n[b]: /\n`;
      default:
        return line + "\n";
    }
  });

export const autolinkChaos = makeChaos("./:@~_-wWhHtTpP><&;()[]\\");

// Keep parser features aligned with the remark reference; other suites cover math and frontmatter.
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

// Empty reference handlers match Sätteri’s omission of unhandled directives.
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

export const mdxDocument = fc.oneof(
  { weight: 1, arbitrary: generatedMdxDocument },
  { weight: 2, arbitrary: commonmarkExample },
  { weight: 2, arbitrary: mutatedCommonmarkExample },
  { weight: 3, arbitrary: mdxExample },
  { weight: 3, arbitrary: mutatedMdxExample },
);

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

// Curated inputs cover malformed and boundary cases absent from the well-formed math generator.
const MATH_EXAMPLES: string[] = [
  "$x$",
  "$x = 1$",
  "$a + b$",
  "$\\alpha$",
  "$\\frac{a}{b}$",
  "$\\sqrt{x^2 + y^2}$",
  "$\\sum_{i=0}^{n} i$",
  "$\\int_0^1 f(x)\\, dx$",
  "$$x$$",
  "$$x = 1$$",
  "$$\nx = 1\n$$",
  "$$\n\\frac{a}{b}\n$$",
  "$$\n\\begin{matrix}\n  a & b \\\\\n  c & d\n\\end{matrix}\n$$",
  "$$\n\\begin{aligned}\n  x &= 1 \\\\\n  y &= 2\n\\end{aligned}\n$$",
  "It costs $5 and $10.",
  "$5 + $10 = $15",
  "Worth $1,000 today.",
  "Use \\$ for currency, $x$ for math.",
  "Plain text \\$5 and math $5x$.",
  "before$x$after",
  "($x$)",
  "[$x$]",
  "$$",
  "$$$$",
  "$x",
  "$x = 1",
  "$$\nx = 1",
  "$x and $y",
  "$\\$$",
  "$x \\text{ for } \\$y$",
  "# Heading with $x$ math",
  "## $E = mc^2$",
  "- list item $x$\n- another $y$",
  "1. ordered $a$\n2. items $b$",
  "> blockquote with $\\sum_i x_i$",
  "> $$\n> x = 1\n> $$",
  "| col | val |\n| --- | --- |\n| a   | $x$ |",
  "Define $f(x)$ as:\n\n$$\nf(x) = x^2\n$$\n\nThen $f(2) = 4$.",
  "First paragraph with $a$.\n\n$$\n\\int_0^\\infty e^{-x^2}\\, dx = \\frac{\\sqrt\\pi}{2}\n$$\n\nSecond paragraph with $b$.",
  "$a_i$",
  "$x^2$",
  "$x_i^2$",
  "$\\sum_{i=1}^{n} x_i^2$",
  "$\\vec{v}$",
  "$\\mathbf{A}$",
  "$\\frac{1}{1 + \\frac{1}{x}}$",
  "$\\binom{n}{k}$",
  "$e^{i\\pi} + 1 = 0$",
  "$\\cos^2\\theta + \\sin^2\\theta = 1$",
  "$ x $",
  "$$ x $$",
  "$x\ny$",
];

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

// Self-contained expressions expose math/MDX misclassification as output differences, not ReferenceErrors.
const MATH_INNER = fc.string({
  unit: fc.constantFrom(..."abcdefghijklmnopqrstuvwxyz0123456789".split("")),
  minLength: 1,
  maxLength: 8,
});
const mathSpanFrag = fc.oneof(
  MATH_INNER.map((t) => `$${t}$`),
  fc.constantFrom("$\\frac{a}{b}$", "$x^{2}$", "$e^{i\\pi}$", "$a_{i}$", "$\\sum_{i} x$"),
);
const selfContainedExpr = fc.oneof(
  fc.integer({ min: 0, max: 99 }).map((n) => `{${n}}`),
  fc.constantFrom("{1 + 2}", "{`x`}", "{true ? 'a' : 'b'}", "{String(7)}"),
);
const dollarAmount = fc.integer({ min: 1, max: 9999 }).map((n) => `$${n}`);
const safeWords = fc.string({
  unit: fc.constantFrom(..."abcdefghijklmnopqrstuvwxyz0123456789 ".split("")),
  minLength: 1,
  maxLength: 8,
});

const mathExprLine = fc
  .array(
    fc.oneof(
      { weight: 3, arbitrary: safeWords },
      { weight: 3, arbitrary: mathSpanFrag },
      { weight: 3, arbitrary: selfContainedExpr },
      { weight: 2, arbitrary: dollarAmount },
    ),
    { minLength: 1, maxLength: 8 },
  )
  .map((parts) => parts.join(" "));

const MDX_MATH_EXAMPLES: string[] = [
  "$\\frac{-b}{2a}$ and {1 + 1}",
  "Price is $5 and {x} costs $10",
  "Euler $e^{i\\pi}$ then {3 * 7}",
  "{1 + 2} then $x^2$ done",
  "$a$ {`b`} $c$ {`d`}",
  "value {2} and $\\sum_i x_i$ end",
  "$5 {1} costs $10 today",
  "<Box>before $x$ and {1 + 1} after</Box>",
  "result {7} for $\\frac{a}{b}$",
];
const mdxMathExample = fc.constantFrom(...MDX_MATH_EXAMPLES);

export const mdxMathDocument = fc.oneof(
  { weight: 5, arbitrary: mathExprLine },
  { weight: 3, arbitrary: mdxMathExample },
  {
    weight: 2,
    arbitrary: fc
      .array(fc.oneof(mathExprLine, mdxMathExample), { minLength: 1, maxLength: 4 })
      .map((blocks) => blocks.join("\n\n")),
  },
);

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

const seededFmDocument = fc
  .tuple(frontmatterExample, fc.array(markdownBlock, { minLength: 0, maxLength: 4 }))
  .map(([fm, blocks]) => (blocks.length > 0 ? `${fm}\n${blocks.join("\n\n")}` : fm));

export const fmDocument = fc.oneof(
  { weight: 1, arbitrary: generatedFmDocument },
  { weight: 2, arbitrary: frontmatterExample },
  { weight: 2, arbitrary: seededFmDocument },
);

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
  kind: "content" | "position-only";
  expected: unknown;
  actual: unknown;
}

const HTML_LEVELS = new Set<FuzzLevel>(["html", "math-html", "fm-html"]);

// Exclude exact known reference divergences so fuzz failures remain actionable.
const KNOWN_DIVERGENCES = new Set<string>([
  // Failed YAML detection in remark-frontmatter suppresses the following list marker.
  "fm-mdast\0---\n+",
  "fm-hast\0---\n+",
  "fm-html\0---\n+",
  // Failed YAML detection in remark-frontmatter changes table recognition on the following line.
  "fm-mdast\0+w*\n+-\n:-",
  "fm-hast\0+w*\n+-\n:-",
  "fm-html\0+w*\n+-\n:-",
  // Failed YAML detection in remark-frontmatter changes block parsing on the following line.
  "fm-mdast\0---\n-",
  "fm-hast\0---\n-",
  "fm-html\0---\n-",
  // Failed TOML detection in remark-frontmatter suppresses the following blockquote marker.
  "fm-mdast\0+++\n>!*+-",
  "fm-hast\0+++\n>!*+-",
  "fm-html\0+++\n>!*+-",
  // Failed YAML detection in remark-frontmatter suppresses the following indented list marker.
  "fm-mdast\0---\n + (",
  "fm-hast\0---\n + (",
  "fm-html\0---\n + (",
  // Failed TOML detection in remark-frontmatter suppresses the following list marker.
  "fm-mdast\0+++\t\n+ -",
  "fm-hast\0+++\t\n+ -",
  "fm-html\0+++\t\n+ -",
  // Failed YAML detection in remark-frontmatter suppresses the following list marker.
  "fm-mdast\0---\n- --:[",
  "fm-hast\0---\n- --:[",
  "fm-html\0---\n- --:[",
  // Tab-indented table continuations differ only in position metadata.
  "fm-mdast\0+-:\n:-\n\t:p",
  "fm-hast\0+-:\n:-\n\t:p",
  "fm-html\0+-:\n:-\n\t:p",
  // Sätteri uses CSS text-align where remark emits the deprecated align attribute.
  "mdx-hast\0x7} >=>\n-:",
  // Failed TOML detection in remark-frontmatter suppresses the following list marker.
  "fm-mdast\0+++\n- +i}(",
  "fm-hast\0+++\n- +i}(",
  "fm-html\0+++\n- +i}(",
]);

// Allow known differences in flow-JSX closing-tag placement and container scoping.

// Treat CSS text-align and the deprecated align attribute as equivalent table alignment.
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
  // Extra find-and-replace positions are verified before being dropped, so a
  // wrong one throws instead of passing as expected.
  if (!HTML_LEVELS.has(level)) {
    // A hast `text` inside `<code>` inherits the code span's span, delimiters
    // included, so it never decodes back to its own value.
    if (!HAST_LIKE_LEVELS.has(level)) assertSliceInvariantEverywhere(actual, input);
    reconcileFnrPositions(actual, expected, input);
  }
  try {
    expect(actual).toEqual(expected);
    return null;
  } catch {
    // Failed frontmatter detection can distort remark’s block parsing; compare its plain-Markdown baseline.
    if (isFrontmatterReferenceBug(input, level, actual, expected)) {
      return null;
    }
    if (isMdxOxcAcornRegexDivergence(input, level, actual, expected, refError)) {
      return null;
    }
    if (isMdxStrictScannerDivergence(input, level, actual, expected, refError)) {
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
  if (referenceContainsFrontmatter(expected)) return false;
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

// Sätteri’s inline scanners accept some cross-line JSX that mdx-js rejects at container boundaries.
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
  // mdx-js rejects unclosed flow JSX that Sätteri can recover.
  if (refError.includes("Expected a closing tag for")) return true;
  if (refError.includes("Expected the closing tag")) return true;
  // mdx-js rejects incomplete JSX tag syntax inside expressions.
  if (/Unexpected character `.+?`(?: \(U\+[0-9A-Fa-f]+\))? (?:in name|before name)/.test(refError))
    return true;
  // mdx-js scans expressions across block boundaries that terminate Sätteri’s scan.
  if (refError.includes("Unexpected end of file in expression")) {
    if (treeContainsCodeSpanWithBraces(actual)) return true;
    if (treeContainsTextWithBraces(actual)) return true;
    // mdx-js rejects lazy continuation lines that Sätteri accepts inside expressions.
    if (treeContainsMultilineMdxExpression(actual)) return true;
    // Code-span precedence can keep braces literal where mdx-js starts an expression.
    if (treeContainsCodeSpanWithOpenBrace(actual)) return true;
    // Sätteri accepts unmatched braces in reference labels that mdx-js scans as expressions.
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

function referenceContainsFrontmatter(node: unknown): boolean {
  if (!node || typeof node !== "object") return false;
  const obj = node as { type?: string; children?: unknown[] };
  if (obj.type === "yaml" || obj.type === "toml") return true;
  if (Array.isArray(obj.children)) {
    for (const c of obj.children) if (referenceContainsFrontmatter(c)) return true;
  }
  return false;
}

// Oxc accepts some regex and expression syntax that mdx-js’s Acorn parser rejects.
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
  // Acorn validates ESM and expression bodies separately; both can differ from Oxc.
  if (refError.includes("Could not parse expression with acorn")) {
    if (treeContainsMdxExpression(actual)) return true;
    // Code-span precedence can keep braces literal where mdx-js starts an expression.
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

const KNOWN_MDX_EVAL_DIVERGENCES = new Set<string>([
  // Code-span and JSX precedence differ when a multiline span contains a less-than sign.
  "`\n <`",
  // List continuations can nest differently after an empty marker and a blank line.
  "*\n\n  2. b\n\n    3. c\n",
]);

export interface MdxEvalOptions {
  remarkPlugins: unknown[];
  features: Record<string, unknown>;
}
const DEFAULT_MDX_EVAL_OPTIONS: MdxEvalOptions = {
  remarkPlugins: [remarkGfm],
  features: MDX_FEATURES,
};
export const MDX_MATH_EVAL_OPTIONS: MdxEvalOptions = {
  remarkPlugins: [remarkGfm, remarkMath],
  features: { headingAttributes: false, math: true, frontmatter: false },
};

async function compareMdxEval(
  input: string,
  source: MdxEvalIssue["source"],
  opts: MdxEvalOptions = DEFAULT_MDX_EVAL_OPTIONS,
): Promise<MdxEvalIssue | null> {
  if (KNOWN_MDX_EVAL_DIVERGENCES.has(input)) return null;
  let refHtml: string | undefined;
  let refError = false;
  let refErrorMessage: string | null = null;
  try {
    const { default: RefComponent } = (await mdxEvaluate(input, {
      ...runtime,
      remarkPlugins: opts.remarkPlugins as any,
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
      features: opts.features,
    } as any);
    satHtml = normalizeHtml(
      renderToStaticMarkup(createElement(SatComponent as any, { components: jsxComponents })),
    );
  } catch (e: any) {
    satError = true;
    satErrorMessage = String(e?.message ?? e ?? "");
  }

  // Surface Rust panics even when the reference also throws.
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
    // Apply the same container-boundary divergence filters to parsing and evaluation.
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
        // Acorn may include a Unicode code-point annotation in this JSX-name error.
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
    // Strikethrough and emphasis resolution order can differ from the reference.
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
  opts: MdxEvalOptions = DEFAULT_MDX_EVAL_OPTIONS,
): Promise<MdxEvalIssue[]> {
  const issues: MdxEvalIssue[] = [];
  await fc.assert(
    fc.asyncProperty(arbitrary, async (input) => {
      const issue = await compareMdxEval(input, source, opts);
      if (issue) issues.push(issue);
      return true;
    }),
    FC_OPTIONS_EVAL,
  );
  return issues;
}

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
