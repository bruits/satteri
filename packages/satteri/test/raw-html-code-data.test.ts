import { expect, test } from "vitest";
import type { Element, Nodes } from "hast";
import {
  defineHastPlugin,
  defineMdastPlugin,
  markdownToHast,
  markdownToHtml,
  markdownToJs,
  mdxToJs,
  type CompileOptions,
} from "../src/index.js";

const source = '```js title="a.js"\nx\n```\n';
const fenceData = { lang: "js", meta: 'title="a.js"' };

test.each([false, true])("markdownToHast keeps fence data with rawHtml=%s", async (rawHtml) => {
  const tree = await markdownToHast(source, { features: { rawHtml } });
  if (tree.type !== "root") throw new Error("expected root");
  const pre = tree.children[0];
  expect(pre?.type).toBe("element");
  if (pre?.type !== "element") throw new Error("expected pre");
  const code = pre.children[0];
  if (!code) throw new Error("expected code");
  expect(code.data).toEqual(fenceData);
  expect(code.position).toEqual(pre.position);
  expect(code.position).toBeDefined();
});

test.each([false, true])("mdxToJs HAST plugins observe fence data with rawHtml=%s", (rawHtml) => {
  const seen: unknown[] = [];
  const observer = defineHastPlugin({
    name: "observe-fences",
    element: {
      filter: ["code"],
      visit(node) {
        seen.push(node.data);
      },
    },
  });
  const { code } = mdxToJs(source, { features: { rawHtml }, hastPlugins: [observer] });
  expect(code).toContain("language-js");
  expect(seen).toEqual([fenceData]);
});

function codeElements(node: Nodes): Element[] {
  return [
    ...(node.type === "element" && node.tagName === "code" ? [node] : []),
    ...("children" in node ? node.children.flatMap(codeElements) : []),
  ];
}

const noopMdast = defineMdastPlugin({ name: "noop-mdast", code() {} });

test.each([false, true])(
  "raw HTML lookalikes never acquire fence data (mdast plugin=%s)",
  async (withMdast) => {
    const raw = '<pre><code class="language-js" data-lang="fake">x\n</code></pre>\n\n';
    const repeated =
      "# 👋 café\n\n" + raw + source + "\n" + raw + source.replace("a.js", "b.js") + "\n" + raw;
    const seen: unknown[] = [];
    const observer = defineHastPlugin({
      name: "observe-originals",
      element: {
        filter: ["code"],
        visit(node) {
          seen.push(node.data);
        },
      },
    });
    const tree = await markdownToHast(repeated, {
      features: { rawHtml: true },
      mdastPlugins: withMdast ? [noopMdast] : [],
      hastPlugins: [observer],
    });
    const expected = [
      undefined,
      fenceData,
      undefined,
      { lang: "js", meta: 'title="b.js"' },
      undefined,
    ];
    const codes = codeElements(tree);
    expect(codes.map((node) => node.data)).toEqual(expected);
    expect(codes.filter((node) => node.data).map((node) => node.position)).toEqual(
      codeElements(await markdownToHast(repeated)).map((node) => node.position),
    );
    expect(codes.filter((node) => !node.data).every((node) => node.position === undefined)).toBe(
      true,
    );
    expect(seen).toEqual(expected);
  },
);

test("foster parenting relocates fences without swapping identical code blocks' data", async () => {
  const input =
    "<table>\n\n" +
    source +
    "\n<tr><td>\n\n" +
    source.replace("a.js", "b.js") +
    "\n</td></tr></table>\n";
  const tree = await markdownToHast(input, { features: { rawHtml: true } });
  if (tree.type !== "root") throw new Error("expected root");
  expect(tree.children[0]).toMatchObject({ type: "element", tagName: "pre" });
  const codes = codeElements(tree);
  expect(codes.map((node) => node.data)).toEqual([fenceData, { lang: "js", meta: 'title="b.js"' }]);
  expect(codes.map((node) => node.position)).toEqual(
    codeElements(await markdownToHast(input)).map((node) => node.position),
  );
  const table = tree.children.find((node) => node.type === "element" && node.tagName === "table");
  expect(table && codeElements(table).map((node) => node.data)).toEqual([
    { lang: "js", meta: 'title="b.js"' },
  ]);
});

test.each([
  "```\nx\n```\n",
  "    x\n",
  '```café title="été & <x>"\n<a>&b\n```\n',
  '```js title="a.js"\r\nx\r\n```\r\n',
])("rawHtml preserves code data and content for %j", async (input) => {
  const before = codeElements(await markdownToHast(input));
  const after = codeElements(await markdownToHast(input, { features: { rawHtml: true } }));
  expect(after.map(({ data, properties, children }) => ({ data, properties, children }))).toEqual(
    before.map(({ data, properties, children }) => ({ data, properties, children })),
  );
});

test.each([
  { name: "markdownToHtml", compile: (options: CompileOptions) => markdownToHtml(source, options) },
  { name: "markdownToJs", compile: (options: CompileOptions) => markdownToJs(source, options) },
  { name: "mdxToJs", compile: (options: CompileOptions) => mdxToJs(source, options) },
])("HAST plugins can render fence metadata through $name", async ({ compile }) => {
  const renderer = defineHastPlugin({
    name: "render-fence-meta",
    element: {
      filter: ["code"],
      visit(node) {
        const meta = node.data?.meta;
        if (typeof meta !== "string") return;
        return { ...node, properties: { ...node.properties, title: meta } };
      },
    },
  });
  for (const withMdast of [false, true]) {
    const result = await compile({
      features: { rawHtml: true },
      mdastPlugins: withMdast ? [noopMdast] : [],
      hastPlugins: [renderer],
    });
    const output = "html" in result ? result.html : result.code;
    expect(output).toContain("a.js");
    expect(output).toContain("title");
  }
});

test("MDX compilation preserves metadata and positions in nested JSX children", () => {
  const input = "<Outer>\n\n<Inner>\n\n" + source + "\n</Inner>\n\n</Outer>\n";
  const seen: unknown[] = [];
  const positions: unknown[] = [];
  const observer = defineHastPlugin({
    name: "observe-nested-fences",
    element: {
      filter: ["code"],
      visit(node) {
        seen.push(node.data);
        positions.push(node.position);
      },
    },
  });
  const baselinePositions: unknown[] = [];
  const baselineObserver = defineHastPlugin({
    name: "observe-baseline-positions",
    element: {
      filter: ["code"],
      visit(node) {
        baselinePositions.push(node.position);
      },
    },
  });
  mdxToJs(input, { hastPlugins: [baselineObserver] });
  const { code } = mdxToJs(input, {
    features: { rawHtml: true },
    hastPlugins: [observer],
  });
  expect(seen).toEqual([fenceData]);
  expect(positions).toEqual(baselinePositions);
  // MDX currently omits these nested fence positions even without reparsing;
  // preserving provenance must not invent a span that the source arena lacks.
  expect(code).toContain("Outer");
  expect(code).toContain("Inner");
  expect(code).toContain("language-js");
});
