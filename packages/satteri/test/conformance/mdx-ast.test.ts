import { describe, test, expect } from "vitest";
import { remark } from "remark";
import remarkMdx from "remark-mdx";
import { toHast } from "mdast-util-to-hast";
import type { Root as MdastRoot, Nodes as MdastNodes } from "mdast";
import { pathToFileURL } from "node:url";
import { mdxToMdast, mdxToHast } from "../../src/index.js";

const { remarkMarkAndUnravel } = await import(
  pathToFileURL("node_modules/@mdx-js/mdx/lib/plugin/remark-mark-and-unravel.js").href
);
const mdxParser = remark().use(remarkMdx).use(remarkMarkAndUnravel);

const MDX_PASS_THROUGH_NODES: Array<MdastNodes["type"]> = [
  "mdxJsxFlowElement",
  "mdxJsxTextElement",
  "mdxFlowExpression",
  "mdxTextExpression",
  "mdxjsEsm",
];

// Satteri drops directives during mdast→hast conversion (JS-level handlers
// aren't visible to the Rust converter). Match that by passing empty handlers
// on the reference side.
const emptyHandler = () => undefined;
const REF_TO_HAST_OPTIONS = {
  allowDangerousHtml: true,
  passThrough: MDX_PASS_THROUGH_NODES,
  handlers: {
    containerDirective: emptyHandler,
    leafDirective: emptyHandler,
    textDirective: emptyHandler,
  },
};

type AnyNode = Record<string, unknown>;

function stripPositionsAndEstree(node: unknown): unknown {
  if (typeof node !== "object" || node === null) return node;
  if (Array.isArray(node)) return node.map(stripPositionsAndEstree);
  const out: AnyNode = {};
  for (const [k, v] of Object.entries(node as AnyNode)) {
    if (k === "position") continue;
    // remark-mdx includes parsed estree in `data`; satteri doesn't
    if (k === "data") continue;
    if (Array.isArray(v)) out[k] = v.map(stripPositionsAndEstree);
    else if (typeof v === "object" && v !== null) out[k] = stripPositionsAndEstree(v);
    else out[k] = v;
  }
  return out;
}

function referenceMdast(input: string): unknown {
  return stripPositionsAndEstree(mdxParser.runSync(mdxParser.parse(input)));
}

function satteriMdast(input: string): unknown {
  return stripPositionsAndEstree(mdxToMdast(input));
}

function assertMdastConformance(input: string): void {
  const sat = satteriMdast(input);
  const ref = referenceMdast(input);
  expect(sat).toEqual(ref);
}

function referenceHast(input: string): unknown {
  // unified's `runSync` is typed to return a bare `Node`; the remark MDX
  // pipeline always yields a Root here.
  const mdast = mdxParser.runSync(mdxParser.parse(input)) as MdastRoot;
  return stripPositionsAndEstree(toHast(mdast, REF_TO_HAST_OPTIONS));
}

function satteriHastTree(input: string): unknown {
  return stripPositionsAndEstree(mdxToHast(input));
}

function assertHastConformance(input: string): void {
  const sat = satteriHastTree(input);
  const ref = referenceHast(input);
  expect(sat).toEqual(ref);
}

describe("MDX MDAST conformance", () => {
  test("self-closing flow element", () => {
    assertMdastConformance("<Foo bar={1}/>\n");
  });

  test("flow element with children", () => {
    assertMdastConformance("<Box>hello</Box>\n");
  });

  test("inline JSX in paragraph", () => {
    assertMdastConformance("hello <Foo/> world\n");
  });

  test("fragment", () => {
    assertMdastConformance("<>hello</>\n");
  });

  test("flow expression", () => {
    assertMdastConformance("{1 + 2}\n");
  });

  test("inline expression", () => {
    assertMdastConformance("result: {1 + 2}\n");
  });

  test("multiple self-closing on one line", () => {
    assertMdastConformance("<Foo bar={1}/><Bar baz={2}/>\n");
  });

  // mdast `value` for a multi-line attribute expression keeps the original
  // indentation verbatim — the dedent only happens for the JS handed to the
  // parser. Regression for the phantom-space pipeline.
  test("multi-line JSX attribute expression preserves indent in mdast value", () => {
    assertMdastConformance("<Foo bar={\n  1 +\n    2\n}/>\n");
  });

  test("multi-line JSX child flow expression preserves indent in mdast value", () => {
    assertMdastConformance("<Box>\n  {\n    1 +\n      2\n  }\n</Box>\n");
  });

  // Tab-indented continuation lines hit the partial-tab dedent path: a tab
  // covers up to TAB_WIDTH=4 cols, and only the first INDENT=2 are stripped.
  // Reference and Sätteri have to agree on how the leftover columns appear
  // in the mdast `value`.
  test("multi-line JSX attribute expression with tab indent", () => {
    assertMdastConformance("<Foo bar={\n\t1 +\n\t2\n}/>\n");
  });

  test("balanced open/close", () => {
    assertMdastConformance("<a></a>\n");
  });

  test("ESM import", () => {
    assertMdastConformance('import Foo from "foo"\n');
  });

  test("ESM export", () => {
    assertMdastConformance("export const x = 42\n");
  });

  test("boolean attribute", () => {
    assertMdastConformance("<Foo disabled/>\n");
  });

  test("string attribute", () => {
    assertMdastConformance('<Foo label="hello"/>\n');
  });

  test("expression attribute", () => {
    assertMdastConformance("<Foo bar={1 + 2}/>\n");
  });

  test("spread attribute", () => {
    assertMdastConformance("<Foo {...props}/>\n");
  });

  test("JSX with expression child", () => {
    assertMdastConformance("<Box>{1 + 2}</Box>\n");
  });

  test("nested JSX", () => {
    assertMdastConformance("<Box><Foo/></Box>\n");
  });

  test("paragraph with expression and text", () => {
    assertMdastConformance("a {1} b\n");
  });

  test("heading with JSX", () => {
    assertMdastConformance("# <Foo/>\n");
  });

  test("blockquote with expression", () => {
    assertMdastConformance("> {1 + 2}\n");
  });

  test("list item with JSX", () => {
    assertMdastConformance("- <Foo/>\n");
  });
});

describe("MDX HAST conformance", () => {
  test("self-closing flow element", () => {
    assertHastConformance("<Foo bar={1}/>\n");
  });

  test("flow element with children", () => {
    assertHastConformance("<Box>hello</Box>\n");
  });

  test("inline JSX in paragraph", () => {
    assertHastConformance("hello <Foo/> world\n");
  });

  test("flow expression", () => {
    assertHastConformance("{1 + 2}\n");
  });

  test("inline expression", () => {
    assertHastConformance("result: {1 + 2}\n");
  });

  test("ESM import", () => {
    assertHastConformance('import Foo from "foo"\n');
  });

  test("ESM export", () => {
    assertHastConformance("export const x = 42\n");
  });

  test("heading with JSX", () => {
    assertHastConformance("# <Foo/>\n");
  });

  test("blockquote with expression", () => {
    assertHastConformance("> {1 + 2}\n");
  });

  test("markdown paragraph with JSX and text", () => {
    assertHastConformance("hello <Foo/> world\n");
  });

  test("fragment with expression is flow", () => {
    assertMdastConformance("<>{998}</>");
    assertHastConformance("<>{998}</>");
  });

  test("fragment with text unraveled to flow", () => {
    assertMdastConformance("<>hello</>");
    assertHastConformance("<>hello</>");
  });

  test("fragment with backtick expression is flow", () => {
    assertMdastConformance("<>{`code`}</>");
    assertHastConformance("<>{`code`}</>");
  });

  test("expression then JSX on same line is flow", () => {
    assertMdastConformance("{-83} <Box/>");
    assertHastConformance("{-83} <Box/>");
  });

  test("two consecutive expressions unraveled to flow", () => {
    assertMdastConformance("{-417} {-333}");
    assertHastConformance("{-417} {-333}");
  });

  test("JSX then two expressions unraveled to flow", () => {
    assertMdastConformance("<Box/> {42} {43}");
    assertHastConformance("<Box/> {42} {43}");
  });

  test("expr JSX expr is flow", () => {
    assertMdastConformance("{expr} <Box/> {42}");
  });
});

describe("MDX mark-and-unravel: paragraph inside flow JSX parent", () => {
  // Regression for bug A: when a flow JSX element contains a paragraph whose
  // only children are text-level JSX, remark unravels the paragraph and
  // promotes the child to a flow element. Satteri previously skipped
  // unraveling whenever the paragraph's parent was itself a flow/text JSX
  // element, leaving `<summary>` nested inside an extra paragraph wrapper.

  test("details/summary with blank-line body", () => {
    assertMdastConformance("<details>\n<summary>X</summary>\n\nparagraph content\n\n</details>");
    assertHastConformance("<details>\n<summary>X</summary>\n\nparagraph content\n\n</details>");
  });

  test("single-line flow JSX inside flow parent is unraveled", () => {
    assertMdastConformance("<section>\n<Callout>hello</Callout>\n\nbody\n</section>");
    assertHastConformance("<section>\n<Callout>hello</Callout>\n\nbody\n</section>");
  });

  test("self-closing JSX inside flow parent is unraveled", () => {
    assertMdastConformance("<section>\n<Foo/>\n\nbody\n</section>");
    assertHastConformance("<section>\n<Foo/>\n\nbody\n</section>");
  });

  test("JSX with inline code child is unraveled", () => {
    assertMdastConformance("<details>\n<Spoiler>`inline code`</Spoiler>\n\nbody\n</details>");
  });

  test("JSX with attributes is unraveled", () => {
    assertMdastConformance(
      "<Question>\n<Option isCorrect>yes</Option>\n\n<Option>no</Option>\n</Question>",
    );
  });

  test("text expression inside flow parent is unraveled", () => {
    assertMdastConformance("<Box>\n{value}\n\nbody\n</Box>");
  });
});

describe("MDX listItem.spread: non-trailing blank lines mark item loose", () => {
  // Regression for remark's `listItem._spread` rule: any blank line inside an
  // item — including blanks consumed atomically inside a multi-line flow JSX
  // element — makes the item loose. The gap-between-children heuristic isn't
  // enough on its own (a single child whose span contains a blank line would
  // otherwise escape detection).

  test("blank line between block children of an item", () => {
    assertMdastConformance("- para1\n- para2\n\n  para3\n");
  });

  test("blank lines inside a multi-line flow JSX child", () => {
    assertMdastConformance("- <details>\n\n    body\n\n  </details>\n");
  });

  test("fenced code then details with internal blanks", () => {
    assertMdastConformance(
      "<Steps>\n1. ```js\n   code\n   ```\n   <details>\n       <summary>X</summary>\n\n       body\n   </details>\n</Steps>\n",
    );
  });

  test("tight list with nested sublist stays tight", () => {
    // The blank between inner items must mark the INNER item, not the outer.
    assertMdastConformance("- a\n- b\n  - nested1\n\n  - nested2\n");
  });
});

describe("MDX mdxFlowExpression: continuation-line dedent", () => {
  // Regression for micromark-factory-mdx-expression's `indentSize + 1` prefix
  // strip: up to 2 columns of whitespace are eaten from each continuation line
  // (tabs expand to the next multiple of 4; any leftover tab columns spill out
  // as literal spaces). Must also preserve UTF-8 continuation bytes verbatim.

  test("strips single leading space on continuation", () => {
    assertMdastConformance("{/* hello\n - line2\n*/}\n");
  });

  test("strips exactly 2 columns per continuation line", () => {
    assertMdastConformance("{/* x\n    a\n     b\n*/}\n");
  });

  test("leading tab becomes 2 spaces of remainder", () => {
    assertMdastConformance("{/* x\n\ta\n*/}\n");
  });

  test("space-then-tab: tab fills to column 4, then 2 stripped", () => {
    assertMdastConformance("{/* x\n \ta\n*/}\n");
  });

  test("second tab after full-strip is preserved", () => {
    assertMdastConformance("{/* x\n\t\ta\n*/}\n");
  });

  test("utf-8 content on continuation lines is byte-safe", () => {
    // Previously the dedent walked by bytes and corrupted multi-byte chars.
    assertMdastConformance("{/* x\n  café\n   über\n*/}\n");
  });
});

describe("MDX flow expression interrupts paragraphs", () => {
  // Regression: a line starting with `{…}` that scans as a flow expression
  // must interrupt an open paragraph, matching remark's paragraph-interrupt
  // set. Without this, `{/* TODO */}` nested between a paragraph/list and the
  // next block gets swallowed as an inline MdxTextExpression.

  test("expression between paragraph and heading", () => {
    assertMdastConformance("Text.\n{/* TODO */}\n## Heading\n");
  });

  test("expression between two lists", () => {
    assertMdastConformance("- A\n{/* TODO */}\n- B\n");
  });

  test("non-flow `{` stays inline in paragraph", () => {
    assertMdastConformance("Text {1 + 1} more text.\n");
  });
});

describe("MDX nested deep-indent lists", () => {
  // Regression for the continuation-indent calculation in MDX-mode's
  // "scan past 4 spaces to find a deeper marker" branch. The extra
  // whitespace consumed by `scan_all_space` must be added to the item's
  // `indent`, or sibling markers at the same column get swallowed as
  // nested sublists.

  test("bullet list at 6 spaces inside MDX flow", () => {
    assertMdastConformance("      - a\n      - b\n      - c\n");
  });

  test("bullet list at 10 spaces inside MDX flow", () => {
    assertMdastConformance("          - a\n          - b\n          - c\n");
  });

  test("ordered outer + deeply-indented inner list", () => {
    assertMdastConformance("7. outer\n\n          - a\n          - b\n          - c\n");
  });
});

// A reference label is scanned over raw source, so it can stop on a `]` that
// sits inside an expression — the expression loses to the label, and the bytes
// past the label's `]` are literal text. remark resolves them the same way;
// they used to belong to no node at all here and vanished from the output.
describe("MDX expression holding the `]` that ends a reference label", () => {
  test("full reference", () => {
    assertMdastConformance('[a][{"]"}]\n\n[{"]: /u\n');
  });

  test("image reference", () => {
    assertMdastConformance('![a][{"]"}]\n\n[{"]: /u\n');
  });

  test("the expression's own quoting does not matter", () => {
    assertMdastConformance("[a][{`]`}]\n\n[{`]: /u\n");
    assertMdastConformance('[a][{"]" + "]"}]\n\n[{"]: /u\n');
  });

  test("with content around it", () => {
    assertMdastConformance('x [a][{"]"}] y\n\n[{"]: /u\n');
    assertMdastConformance('[a][{"]"}][b]\n\n[{"]: /u\n\n[b]: /v\n');
    assertMdastConformance('[a][{"]"}]\n\n[{"]: /u "t"\n');
  });

  test("in every container", () => {
    assertMdastConformance('> [a][{"]"}]\n\n[{"]: /u\n');
    assertMdastConformance('- [a][{"]"}]\n\n[{"]: /u\n');
    assertMdastConformance('# [a][{"]"}]\n\n[{"]: /u\n');
  });

  // Controls: the expression survives whole when no label ends inside it.
  test("an expression the label does not cut keeps its node", () => {
    assertMdastConformance('[{"]"}][a]\n\n[a]: /u\n');
    assertMdastConformance('[a][{"x"}]\n\n[{"x"}]: /u\n');
    assertMdastConformance('x {"]"} y\n');
  });

  // The tree comparison above drops positions, so the span is pinned here.
  test("the tail spans exactly the bytes past the label", () => {
    const md = '[a][{"]"}]\n\n[{"]: /u\n';
    const tree = mdxToMdast(md) as unknown as { children: Array<{ children: AnyNode[] }> };
    expect(tree.children[0]?.children[1]).toEqual({
      type: "text",
      value: '"}]',
      position: {
        start: { line: 1, column: 8, offset: 7 },
        end: { line: 1, column: 11, offset: 10 },
      },
    });
  });

  // Pre-existing: the expression is handed to oxc before reference resolution
  // takes its `]`, so a diagnostic outlives the node it was about. The tree the
  // arena builds is correct; only the error is spurious.
  describe("divergence: an expression the label cuts still reports its error", () => {
    test.fails("an expression that does not parse on its own", () => {
      assertMdastConformance("[a][{x]}]\n\n[{x]: /u\n");
    });

    test.fails("a nested expression the label cuts", () => {
      assertMdastConformance('[a][{f({"]"})}]\n\n[{f({"]: /u\n');
    });
  });

  // The tail is emitted as literal text, so inline markup inside it is never
  // tokenized. Pre-fix those bytes were dropped outright, so this is the
  // remaining half of the class rather than a regression.
  describe("divergence: markup in the tail stays literal", () => {
    test.fails("emphasis", () => {
      assertMdastConformance('[a][{"]*x*"}]\n\n[{"]: /u\n');
    });

    test.fails("inline code", () => {
      assertMdastConformance('[a][{"]`c`"}]\n\n[{"]: /u\n');
    });

    test.fails("character reference", () => {
      assertMdastConformance('[a][{"]&amp;"}]\n\n[{"]: /u\n');
    });
  });
});

// Found by `fuzz/mdx.test.ts` at seed 3. A `//` line comment ends at any line
// ending, so every case below has to behave the same for `\n`, `\r\n` and a
// lone `\r`.
describe.each([
  ["LF", "\n"],
  ["CRLF", "\r\n"],
  ["CR", "\r"],
])("MDX expression comment ended by %s", (_name, eol) => {
  test("the `}` after the comment still closes the expression", () => {
    assertMdastConformance(`a{//${eol}}`);
    assertMdastConformance(`a{//${eol}} b`);
  });

  test("comment mid-expression", () => {
    assertMdastConformance(`a{1 + // c${eol}2}`);
    assertMdastConformance(`a{[1, //${eol}2]}`);
    assertMdastConformance(`a{{x:1} //${eol}}`);
  });

  test("the comment body is not re-lexed", () => {
    assertMdastConformance(`a{// }${eol}1}`);
    assertMdastConformance(`a{// don't${eol}1}`);
    assertMdastConformance(`a{// \`x${eol}1}`);
    assertMdastConformance(`a{// /*${eol}1}`);
    assertMdastConformance(`a{// /x/${eol}1}`);
  });

  test("comment-only body", () => {
    assertMdastConformance(`{//${eol}}${eol}`);
    assertMdastConformance(`{//a${eol}//b${eol}}`);
  });

  test("in JSX attributes and children", () => {
    assertMdastConformance(`<Foo bar={//${eol}1}/>${eol}`);
    assertMdastConformance(`<Box>{//${eol}1}</Box>${eol}`);
  });

  test("after a value that could swallow the line", () => {
    assertMdastConformance(`a{\`t\` //${eol}}`);
  });

  // Block comments legitimately span line endings, so the same shapes must
  // still run past one rather than stopping with the line.
  test("block comments still span the line ending", () => {
    assertMdastConformance(`a{/*${eol}*/}`);
    assertMdastConformance(`a{/* x${eol}y${eol} */ 1}`);
    assertMdastConformance(`<Foo bar={/* x${eol}*/ 1}/>${eol}`);
  });

  test("a comment in an ESM block ends with the line", () => {
    assertMdastConformance(`import a from "b" //${eol}${eol}x${eol}`);
    assertMdastConformance(`export const a = 1 // c${eol}${eol}x${eol}`);
    assertMdastConformance(`import a from "b" //x${eol}import c from "d"${eol}${eol}y${eol}`);
    assertMdastConformance(`export const a = 1 /* c${eol}*/${eol}${eol}x${eol}`);
  });
});

// Pre-existing and unrelated to line endings — both reproduce identically for
// `\n`, `\r\n` and `\r`. `export` followed by a line ending opens an ESM block
// in Sätteri but not in micromark, and a block comment spanning the blank line
// that should end the block is accepted rather than cut short.
describe.each([
  ["LF", "\n"],
  ["CRLF", "\r\n"],
  ["CR", "\r"],
])("MDX ESM block opener divergences (%s)", (_name, eol) => {
  test.fails("`export` followed by a line ending is not an ESM block", () => {
    assertMdastConformance(`export${eol}const a = 1${eol}${eol}x${eol}`);
  });

  test.fails("a block comment does not span the blank line ending the block", () => {
    assertMdastConformance(`export const a = 1 /*${eol}${eol}*/${eol}z${eol}`);
  });
});

// A `\` before a line ending is one JS line continuation, so the string runs on
// to the next line. CRLF is one ending, and eating only half of it used to end
// the string on the orphan `\n` and run the expression to end of input.
describe.each([
  ["LF", "\n"],
  ["CRLF", "\r\n"],
  ["CR", "\r"],
])("MDX expression string continued over a line ending (%s)", (_name, eol) => {
  test("a double-quoted string keeps going", () => {
    assertMdastConformance(`a{"x\\${eol}y"}${eol}`);
  });

  test("a single-quoted string keeps going", () => {
    assertMdastConformance(`a{'x\\${eol}y'}${eol}`);
  });
});
