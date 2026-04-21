import { describe, test, expect } from "vitest";
import { remark } from "remark";
import remarkMdx from "remark-mdx";
import { toHast } from "mdast-util-to-hast";
import { pathToFileURL } from "node:url";
import { mdxToMdast, mdxToHast } from "../../src/index.js";

const { remarkMarkAndUnravel } = await import(
  pathToFileURL("node_modules/@mdx-js/mdx/lib/plugin/remark-mark-and-unravel.js").href
);
const mdxParser = remark().use(remarkMdx).use(remarkMarkAndUnravel);

const MDX_PASS_THROUGH_NODES = [
  "mdxJsxFlowElement",
  "mdxJsxTextElement",
  "mdxFlowExpression",
  "mdxTextExpression",
  "mdxjsEsm",
];

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
    else if (typeof v === "object" && v !== null)
      out[k] = stripPositionsAndEstree(v);
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
  const mdast = mdxParser.runSync(mdxParser.parse(input));
  return stripPositionsAndEstree(
    toHast(mdast, { allowDangerousHtml: true, passThrough: MDX_PASS_THROUGH_NODES }),
  );
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
