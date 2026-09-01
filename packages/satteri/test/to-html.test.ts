import { describe, test, expect } from "vitest";
import { unified } from "unified";
import rehypeStringify from "rehype-stringify";
import { h, s } from "hastscript";
import { html as htmlSchema, svg as svgSchema } from "property-information";
import {
  defineHastPlugin,
  hastToHtml,
  htmlToHast,
  markdownToHast,
  markdownToHtml,
  mdxToHast,
} from "../src/index.js";
import type { HastNode } from "../src/hast/hast-materializer.js";

const stringify = (tree: HastNode | HastNode[]): string =>
  unified()
    .use(rehypeStringify, {
      allowDangerousHtml: true,
      characterReferences: { useNamedReferences: true },
    })
    .stringify({ type: "root", children: Array.isArray(tree) ? tree : [tree] } as never);

/** `hast-util-to-html` leaves `>`, `'` and `` ` `` unescaped where the renderer,
 *  following cmark, escapes them; both are valid HTML for the same text. */
const foldEscapes = (html: string): string =>
  html.replaceAll("&gt;", ">").replaceAll("&#x27;", "'").replaceAll("&#x60;", "`");

const matchesOracle = (tree: HastNode | HastNode[]): void => {
  expect(foldEscapes(hastToHtml(tree))).toBe(foldEscapes(stringify(tree)));
};

describe("hastToHtml", () => {
  test("serializes an element and escapes its text", () => {
    const tree = h("p", ["hi & <bye>"]) as HastNode;
    expect(hastToHtml(tree)).toBe("<p>hi &amp; &lt;bye&gt;</p>");
    matchesOracle(tree);
  });

  test("renders a root's children, not the root", () => {
    const tree = { type: "root", children: [h("p", "a"), h("p", "b")] } as HastNode;
    expect(hastToHtml(tree)).toBe("<p>a</p><p>b</p>");
  });

  test("renders a list of nodes in order", () => {
    expect(hastToHtml([{ type: "text", value: "a" }, h("br") as HastNode])).toBe("a<br>");
  });

  test("renders a lone text node", () => {
    expect(hastToHtml({ type: "text", value: "a & b" })).toBe("a &amp; b");
  });

  test("returns an empty string for an empty root", () => {
    expect(hastToHtml({ type: "root", children: [] })).toBe("");
  });

  test("adds no trailing newline, unlike a rendered document", () => {
    const source = "# Hello *world*";
    expect(hastToHtml(markdownToHast(source))).toBe(`${markdownToHtml(source).html.trimEnd()}`);
    expect(hastToHtml(markdownToHast(source)).endsWith("\n")).toBe(false);
  });

  test("closes void elements without a slash and drops their children", () => {
    const tree = { type: "root", children: [h("img", { src: "a.png" }), h("hr")] } as HastNode;
    expect(hastToHtml(tree)).toBe(`<img src="a.png"><hr>`);
    matchesOracle(tree);
  });

  test("serializes the property kinds", () => {
    const tree = h("input", {
      className: ["a", "b"],
      disabled: true,
      hidden: false,
      tabIndex: 2,
      value: "",
    }) as HastNode;
    expect(hastToHtml(tree)).toBe(`<input class="a b" disabled tabindex="2" value="">`);
    matchesOracle(tree);
  });

  test("joins list properties by name, comma-separated ones included", () => {
    const el = (tagName: string, properties: Record<string, unknown>): HastNode =>
      ({ type: "element", tagName, properties, children: [] }) as unknown as HastNode;
    const cases: HastNode[] = [
      el("p", { className: ["x", "y"] }),
      el("img", { srcSet: ["a 1x", "b 2x"] }),
      el("input", { accept: ["a/b", "c/d"] }),
      el("area", { coords: [1, 2, 3] }),
      el("div", { exportParts: ["a", "b"] }),
    ];
    expect(cases.map((node) => hastToHtml(node))).toEqual([
      `<p class="x y"></p>`,
      `<img srcset="a 1x b 2x">`,
      `<input accept="a/b, c/d">`,
      `<area coords="1, 2, 3">`,
      `<div exportparts="a, b"></div>`,
    ]);
    for (const node of cases) matchesOracle(node);
  });

  test("joins list edge cases like the token stringifiers", () => {
    const div = (properties: Record<string, unknown>): HastNode =>
      ({ type: "element", tagName: "div", properties, children: [] }) as unknown as HastNode;
    const cases = [
      div({ exportParts: [""] }),
      div({ exportParts: ["a", ""] }),
      div({ exportParts: [" a "] }),
      div({ className: ["", "a"] }),
      div({ className: [" a "] }),
      div({ className: [] }),
    ];
    expect(cases.map((node) => hastToHtml(node))).toEqual([
      `<div exportparts=","></div>`,
      `<div exportparts="a, ,"></div>`,
      `<div exportparts="a"></div>`,
      `<div class="a"></div>`,
      `<div class="a"></div>`,
      `<div class=""></div>`,
    ]);
    for (const node of cases) matchesOracle(node);
  });

  test("separates list properties the way property-information classifies them", () => {
    const cases: [property: string, node: HastNode][] = [];
    for (const [property, info] of Object.entries(htmlSchema.property)) {
      if (!info.spaceSeparated && !info.commaSeparated) continue;
      cases.push([
        property,
        {
          type: "element",
          tagName: "div",
          properties: { [property]: ["a", "b"] },
          children: [],
        } as unknown as HastNode,
      ]);
    }
    // Inside `<svg>` the SVG schema applies, and it separates a different set:
    // `coords` turns plain there, `glyphName` turns comma-separated.
    for (const [property, info] of Object.entries(svgSchema.property)) {
      if (!info.spaceSeparated && !info.commaSeparated) continue;
      cases.push([
        `svg ${property}`,
        {
          type: "element",
          tagName: "svg",
          properties: {},
          children: [
            {
              type: "element",
              tagName: "circle",
              properties: { [property]: ["a", "b"] },
              children: [],
            },
          ],
        } as unknown as HastNode,
      ]);
    }
    expect(cases.length).toBeGreaterThan(30);
    for (const [label, node] of cases) {
      expect(hastToHtml(node), label).toBe(stringify(node));
    }
  });

  test("separates a list by the schema of the element it sits in", () => {
    const inSvg = (properties: Record<string, unknown>): HastNode =>
      ({
        type: "element",
        tagName: "svg",
        properties: {},
        children: [{ type: "element", tagName: "circle", properties, children: [] }],
      }) as unknown as HastNode;
    const inHtml = (tagName: string, properties: Record<string, unknown>): HastNode =>
      ({ type: "element", tagName, properties, children: [] }) as unknown as HastNode;

    expect(hastToHtml(inSvg({ coords: [1, 2] }))).toBe(`<svg><circle coords="1 2"></circle></svg>`);
    expect(hastToHtml(inHtml("area", { coords: [1, 2] }))).toBe(`<area coords="1, 2">`);
    expect(hastToHtml(inSvg({ glyphName: ["a", "b"] }))).toBe(
      `<svg><circle glyph-name="a, b"></circle></svg>`,
    );
    expect(hastToHtml(inHtml("div", { glyphName: ["a", "b"] }))).toBe(
      `<div glyphName="a b"></div>`,
    );
  });

  test("keeps a token holding the other schema's separator intact", () => {
    const node = (properties: Record<string, unknown>): HastNode =>
      ({
        type: "element",
        tagName: "svg",
        properties: {},
        children: [{ type: "element", tagName: "circle", properties, children: [] }],
      }) as unknown as HastNode;
    const cases = [node({ accept: [","] }), node({ glyphName: [""] }), node({ accept: ["a b"] })];
    expect(cases.map((tree) => hastToHtml(tree))).toEqual([
      `<svg><circle accept=","></circle></svg>`,
      `<svg><circle glyph-name=","></circle></svg>`,
      `<svg><circle accept="a b"></circle></svg>`,
    ]);
    for (const tree of cases) matchesOracle(tree);
  });

  test("escapes text below a raw-text element, not just inside it", () => {
    const tree = {
      type: "element",
      tagName: "script",
      properties: {},
      children: [
        { type: "text", value: "a & b" },
        {
          type: "element",
          tagName: "div",
          properties: {},
          children: [{ type: "text", value: "c & d" }],
        },
      ],
    } as unknown as HastNode;
    expect(hastToHtml(tree)).toBe("<script>a & b<div>c &amp; d</div></script>");
    matchesOracle(tree);
  });

  test("round-trips a comma-separated attribute through htmlToHast", () => {
    const tree = htmlToHast(`<input accept="a,b"><input accept=", ,">`, { fragment: true });
    expect(tree.type).toBe("root");
    const accepts = (tree.children as HastNode[]).map((child) =>
      child.type === "element" ? child.properties.accept : undefined,
    );
    expect(accepts).toEqual([
      ["a", "b"],
      ["", ""],
    ]);
    expect(hastToHtml(tree)).toBe(`<input accept="a, b"><input accept=", ,">`);
    matchesOracle(tree);
  });

  test("treats an inherited property name as an ordinary list", () => {
    const node = {
      type: "element",
      tagName: "div",
      properties: { constructor: ["a", "b"] },
      children: [],
    } as unknown as HastNode;
    expect(hastToHtml(node)).toBe(`<div constructor="a b"></div>`);
  });

  test("a plugin's setProperty joins a list the same way", () => {
    const plugin = defineHastPlugin({
      name: "set-coords",
      element: {
        filter: ["a"],
        visit(node, ctx) {
          ctx.setProperty(node, "coords", [1, 2]);
          ctx.setProperty(node, "className", ["x", "y"]);
        },
      },
    });
    const { html } = markdownToHtml("[a](/x)", { hastPlugins: [plugin] });
    expect(html.trim()).toBe(`<p><a href="/x" coords="1, 2" class="x y">a</a></p>`);
  });

  test("escapes attribute values", () => {
    expect(hastToHtml(h("a", { title: `a "b" & c` }) as HastNode)).toBe(
      `<a title="a &quot;b&quot; &amp; c"></a>`,
    );
  });

  test("keeps comments verbatim", () => {
    const tree = h("div", [{ type: "comment", value: " note " }]) as HastNode;
    expect(hastToHtml(tree)).toBe("<div><!-- note --></div>");
    matchesOracle(tree);
  });

  test("emits raw nodes verbatim", () => {
    const tree = { type: "root", children: [{ type: "raw", value: "<b>x</b>" }] } as HastNode;
    expect(hastToHtml(tree)).toBe("<b>x</b>");
    matchesOracle(tree);
  });

  test("does not escape text inside script and style", () => {
    const tree = {
      type: "root",
      children: [h("script", "a < b && c"), h("style", "a > b")],
    } as HastNode;
    expect(hastToHtml(tree)).toBe("<script>a < b && c</script><style>a > b</style>");
    matchesOracle(tree);
  });

  test("uses the SVG attribute schema inside <svg>", () => {
    const tree = s("svg", { viewBox: "0 0 1 1" }, [s("circle", { strokeWidth: 2 })]) as HastNode;
    expect(hastToHtml(tree)).toBe(
      `<svg viewBox="0 0 1 1"><circle stroke-width="2"></circle></svg>`,
    );
    matchesOracle(tree);
  });

  test("round-trips a document parsed from HTML, doctype included", () => {
    const html = `<!doctype html><html><head><title>t</title></head><body><p class="a">hi</p></body></html>`;
    expect(hastToHtml(htmlToHast(html))).toBe(html);
  });

  test("round-trips a fragment parsed from HTML", () => {
    const html = `<p>a</p><ul><li>b</li></ul><img src="c.png">`;
    expect(hastToHtml(htmlToHast(html, { fragment: true }))).toBe(html);
  });

  test("matches hast-util-to-html on a rendered Markdown document", () => {
    const source = [
      "# Title",
      "",
      "A [link](/x 'the x') with `code`, **bold**, and an ![img](a.png).",
      "",
      "> quoted",
      "",
      "```js",
      "const a = 1 < 2;",
      "```",
      "",
      "| a | b |",
      "| - | - |",
      "| 1 | 2 |",
      "",
      '<div class="raw">kept</div>',
    ].join("\n");
    const tree = markdownToHast(source, { features: { gfm: true } });
    matchesOracle(tree);
    expect(hastToHtml(tree)).toBe(
      markdownToHtml(source, { features: { gfm: true } }).html.trimEnd(),
    );
  });

  test("renders a spread copy of a parsed tree", () => {
    const tree = markdownToHast("*a*");
    if (tree.type !== "root") throw new Error("markdownToHast returned a non-root");
    const copy: HastNode = { ...tree, children: [...tree.children] };
    expect(hastToHtml(copy)).toBe("<p><em>a</em></p>");
  });

  test("serializes a node handed to a plugin visitor", () => {
    let captured: string | undefined;
    const plugin = defineHastPlugin({
      name: "capture-em",
      element: {
        filter: ["em"],
        visit(node) {
          captured = hastToHtml(node);
        },
      },
    });
    markdownToHtml("*a* and **b**", { hastPlugins: [plugin] });
    expect(captured).toBe("<em>a</em>");
  });

  test("skips MDX nodes, which have no HTML representation", () => {
    const tree = mdxToHast("<Foo bar />\n\nplain\n\n{1 + 1}");
    expect(hastToHtml(tree)).toBe("\n<p>plain</p>\n");
  });

  test("serializes a deeply nested tree", () => {
    let node = h("div", "x") as HastNode;
    for (let i = 0; i < 500; i++) node = h("div", [node]) as HastNode;
    const html = hastToHtml(node);
    expect(html.split("<div>").length - 1).toBe(501);
    matchesOracle(node);
  });

  test("rejects a root nested inside the tree", () => {
    const tree = {
      type: "element",
      tagName: "div",
      properties: {},
      children: [{ type: "root", children: [] }],
    } as unknown as HastNode;
    expect(() => hastToHtml(tree)).toThrow(/cannot encode/);
  });

  test("rejects a node type it cannot encode", () => {
    expect(() => hastToHtml({ type: "nope" } as unknown as HastNode)).toThrow(/cannot encode/);
  });
});
