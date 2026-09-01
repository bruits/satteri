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
import type { Element, ElementContent, Properties } from "hast";

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

const el = (tagName: string, properties: Properties, children: ElementContent[] = []): Element => ({
  type: "element",
  tagName,
  properties,
  children,
});

/** The SVG schema applies to an element's own attributes too, so the property
 *  under test goes on a child of `<svg>`, not on the `<svg>`. */
const inSvg = (properties: Properties): Element => el("svg", {}, [el("circle", properties)]);

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
    const cases: Element[] = [
      el("p", { className: ["x", "y"] }),
      // hast types `srcSet` as a string; an array is still valid input, and
      // it separates with spaces because the schema does not mark it comma-separated.
      el("img", { srcSet: ["a 1x", "b 2x"] } as unknown as Properties),
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
    const cases = [
      el("div", { exportParts: [""] }),
      el("div", { exportParts: ["a", ""] }),
      el("div", { exportParts: [" a "] }),
      el("div", { className: ["", "a"] }),
      el("div", { className: [" a "] }),
      el("div", { className: [] }),
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
    const cases: [property: string, node: Element][] = [];
    for (const [property, info] of Object.entries(htmlSchema.property)) {
      if (!info.spaceSeparated && !info.commaSeparated) continue;
      cases.push([property, el("div", { [property]: ["a", "b"] })]);
    }
    // Inside `<svg>` the SVG schema applies, and it separates a different set:
    // `coords` turns plain there, `glyphName` turns comma-separated.
    for (const [property, info] of Object.entries(svgSchema.property)) {
      if (!info.spaceSeparated && !info.commaSeparated) continue;
      cases.push([`svg ${property}`, inSvg({ [property]: ["a", "b"] })]);
    }
    expect(cases.length).toBeGreaterThan(30);
    for (const [label, node] of cases) {
      expect(hastToHtml(node), label).toBe(stringify(node));
    }
  });

  test("separates a list by the schema of the element it sits in", () => {
    expect(hastToHtml(inSvg({ coords: [1, 2] }))).toBe(`<svg><circle coords="1 2"></circle></svg>`);
    expect(hastToHtml(el("area", { coords: [1, 2] }))).toBe(`<area coords="1, 2">`);
    expect(hastToHtml(inSvg({ glyphName: ["a", "b"] }))).toBe(
      `<svg><circle glyph-name="a, b"></circle></svg>`,
    );
    expect(hastToHtml(el("div", { glyphName: ["a", "b"] }))).toBe(`<div glyphName="a b"></div>`);
  });

  test("keeps a token holding the other schema's separator intact", () => {
    const cases = [
      inSvg({ accept: [","] }),
      inSvg({ glyphName: [""] }),
      inSvg({ accept: ["a b"] }),
    ];
    expect(cases.map((tree) => hastToHtml(tree))).toEqual([
      `<svg><circle accept=","></circle></svg>`,
      `<svg><circle glyph-name=","></circle></svg>`,
      `<svg><circle accept="a b"></circle></svg>`,
    ]);
    for (const tree of cases) matchesOracle(tree);
  });

  test("escapes text below a raw-text element, not just inside it", () => {
    const tree = el("script", {}, [
      { type: "text", value: "a & b" },
      el("div", {}, [{ type: "text", value: "c & d" }]),
    ]);
    expect(hastToHtml(tree)).toBe("<script>a & b<div>c &amp; d</div></script>");
    matchesOracle(tree);
  });

  test("round-trips a comma-separated attribute through htmlToHast", () => {
    const tree = htmlToHast(`<input accept="a,b"><input accept=", ,">`, { fragment: true });
    if (tree.type !== "root") throw new Error("htmlToHast returned a non-root");
    const accepts = tree.children.map((child) =>
      child.type === "element" ? child.properties.accept : undefined,
    );
    expect(accepts).toEqual([
      ["a", "b"],
      ["", ""],
    ]);
    expect(hastToHtml(tree)).toBe(`<input accept="a, b"><input accept=", ,">`);
    matchesOracle(tree);
  });

  test("drops a NaN property instead of rendering it", () => {
    const built = h("div", { tabIndex: Number.NaN }) as HastNode;
    expect(hastToHtml(built)).toBe("<div></div>");
    matchesOracle(built);

    const plugin = defineHastPlugin({
      name: "set-nan",
      element: {
        filter: ["a"],
        visit(node, ctx) {
          ctx.setProperty(node, "tabIndex", Number.NaN);
        },
      },
    });
    expect(markdownToHtml("[a](/x)", { hastPlugins: [plugin] }).html.trim()).toBe(
      `<p><a href="/x">a</a></p>`,
    );
  });

  test("pads a trailing empty item but not a trailing null", () => {
    // `null` is not a hast list item, but a plugin can hand one over.
    const cases = [
      el("div", { exportParts: ["a", ""] }),
      el("div", { exportParts: ["a", null] } as unknown as Properties),
    ];
    expect(cases.map((node) => hastToHtml(node))).toEqual([
      `<div exportparts="a, ,"></div>`,
      `<div exportparts="a,"></div>`,
    ]);
    for (const node of cases) matchesOracle(node);
  });

  test("renders a root nested in a list of nodes", () => {
    const nodes = [
      h("p", "a") as HastNode,
      { type: "root", children: [h("p", "b") as HastNode] } as HastNode,
    ];
    expect(hastToHtml(nodes)).toBe("<p>a</p><p>b</p>");
  });

  test("renders a childless root as an empty string", () => {
    expect(hastToHtml({ type: "root" } as unknown as HastNode)).toBe("");
  });

  test("treats an inherited property name as an ordinary list", () => {
    const node = el("div", { constructor: ["a", "b"] });
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
