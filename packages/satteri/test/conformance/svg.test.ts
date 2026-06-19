import { describe, test, expect } from "vitest";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkRehype from "remark-rehype";
import rehypeStringify from "rehype-stringify";
import type { Root as MdastRoot, Nodes as MdastNodes } from "mdast";
import type { Root as HastRoot, ElementContent } from "hast";
import { markdownToHtml, defineMdastPlugin, defineHastPlugin } from "../../src/index.js";
import type { MdastNode, HastNode } from "../../src/types.js";

// Compare satteri's HTML output to remark-rehype + hast-util-to-html for
// SVG attribute serialization and numeric properties, on both the mdast
// `data.hProperties` path and the direct `_hast: true` HAST emit path.

function visitMdast(tree: MdastNodes, fn: (node: MdastNodes) => void): void {
  fn(tree);
  if ("children" in tree && Array.isArray(tree.children)) {
    for (const child of tree.children as MdastNodes[]) {
      visitMdast(child, fn);
    }
  }
}

interface DataPatch {
  hName?: string;
  hProperties?: Record<string, unknown>;
  hChildren?: ElementContent[];
}

function referenceHtmlWithMdastPatch(
  md: string,
  predicate: (node: MdastNodes) => boolean,
  patch: DataPatch,
): string {
  const processor = unified()
    .use(remarkParse)
    .use(remarkGfm)
    .use(() => (tree: MdastRoot) => {
      visitMdast(tree, (node) => {
        if (!predicate(node)) return;
        const data = ((node as unknown as { data?: Record<string, unknown> }).data ??= {});
        if (patch.hName !== undefined) data.hName = patch.hName;
        if (patch.hProperties !== undefined) data.hProperties = patch.hProperties;
        if (patch.hChildren !== undefined) data.hChildren = patch.hChildren;
      });
    })
    .use(remarkRehype, { allowDangerousHtml: true })
    .use(rehypeStringify, { allowDangerousHtml: true });
  return String(processor.processSync(md)).trim();
}

function satteriHtmlWithMdastPatch(
  md: string,
  predicate: (node: MdastNode) => boolean,
  patch: DataPatch,
): string {
  const plugin = defineMdastPlugin({
    name: "svg-test",
    paragraph(node, ctx) {
      if (!predicate(node)) return;
      const next: Record<string, unknown> = { ...node.data };
      if (patch.hName !== undefined) next.hName = patch.hName;
      if (patch.hProperties !== undefined) next.hProperties = patch.hProperties;
      if (patch.hChildren !== undefined) next.hChildren = patch.hChildren;
      ctx.setProperty(node, "data", next);
    },
  });
  const { html } = markdownToHtml(md, {
    features: { gfm: true, frontmatter: false, math: false },
    mdastPlugins: [plugin],
  });
  return html.trim();
}

function path(properties: Record<string, unknown>): ElementContent {
  return { type: "element", tagName: "path", properties, children: [] } as ElementContent;
}

describe("SVG attribute conformance vs remark-rehype", () => {
  test("kebab-cased SVG attributes (fillRule, strokeWidth, etc.)", () => {
    const patch: DataPatch = {
      hName: "svg",
      hProperties: { viewBox: "0 0 24 24" },
      hChildren: [
        path({
          fillRule: "evenodd",
          clipRule: "evenodd",
          strokeWidth: "2",
          strokeLineCap: "round",
          strokeLineJoin: "round",
          d: "M12 0L0 12h24z",
        }),
      ],
    };
    const ref = referenceHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    const got = satteriHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    expect(got).toBe(ref);
  });

  test("case-preserved SVG attributes (viewBox, preserveAspectRatio, gradientUnits)", () => {
    const patch: DataPatch = {
      hName: "svg",
      hProperties: { viewBox: "0 0 100 100", preserveAspectRatio: "xMidYMid meet" },
      hChildren: [
        {
          type: "element",
          tagName: "linearGradient",
          properties: {
            id: "g",
            gradientUnits: "userSpaceOnUse",
            gradientTransform: "translate(10 20)",
          },
          children: [],
        } as ElementContent,
      ],
    };
    const ref = referenceHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    const got = satteriHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    expect(got).toBe(ref);
  });

  test("namespaced xLinkHref → xlink:href on <use>", () => {
    const patch: DataPatch = {
      hName: "svg",
      hProperties: { viewBox: "0 0 10 10" },
      hChildren: [
        {
          type: "element",
          tagName: "use",
          properties: { xLinkHref: "#sym" },
          children: [],
        } as ElementContent,
      ],
    };
    const ref = referenceHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    const got = satteriHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    expect(got).toBe(ref);
  });

  test("numeric SVG properties on hProperties path (width, height)", () => {
    const patch: DataPatch = {
      hName: "svg",
      hProperties: { viewBox: "0 0 24 24", width: 16, height: 16 },
      hChildren: [path({ d: "M0 0L24 24" })],
    };
    const ref = referenceHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    const got = satteriHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    expect(got).toBe(ref);
  });

  test("numeric HTML property on hProperties path (start on <ol>)", () => {
    // Same numeric-handling code path, exercised on a plain HTML element.
    const patch: DataPatch = {
      hName: "ol",
      hProperties: { start: 5 },
      hChildren: [
        { type: "element", tagName: "li", properties: {}, children: [] } as ElementContent,
      ],
    };
    const ref = referenceHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    const got = satteriHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    expect(got).toBe(ref);
  });

  test("HTML schema unchanged: className stays `class`, srcSet → srcset", () => {
    // Wrapper is non-void on purpose — void-element handling between satteri
    // and hast-util-to-html diverges and would mask the attribute-name check.
    const patch: DataPatch = {
      hName: "section",
      hProperties: { className: "hero", srcSet: "a.png 1x, a@2x.png 2x", tabIndex: 0 },
      hChildren: [],
    };
    const ref = referenceHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    const got = satteriHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    expect(got).toBe(ref);
  });

  test("foreignObject inside SVG keeps SVG schema (matches hast-util-to-html)", () => {
    // hast-util-to-html does not re-enter HTML at <foreignObject>; we mirror.
    const patch: DataPatch = {
      hName: "svg",
      hProperties: { viewBox: "0 0 100 100" },
      hChildren: [
        {
          type: "element",
          tagName: "foreignObject",
          properties: { width: 100, height: 100 },
          children: [
            {
              type: "element",
              tagName: "div",
              properties: { className: "embedded", tabIndex: 0 },
              children: [{ type: "text", value: "hi" }],
            } as ElementContent,
          ],
        } as ElementContent,
      ],
    };
    const ref = referenceHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    const got = satteriHtmlWithMdastPatch("Hi", (n) => n.type === "paragraph", patch);
    expect(got).toBe(ref);
  });
});

describe("SVG attribute conformance via _hast plugin emit path", () => {
  function svgElement(properties: Record<string, unknown>, children: HastNode[] = []): HastNode {
    return {
      type: "element",
      tagName: "svg",
      properties,
      children,
    } as HastNode;
  }

  function pathElement(properties: Record<string, unknown>): HastNode {
    return {
      type: "element",
      tagName: "path",
      properties,
      children: [],
    } as HastNode;
  }

  function expectedHtmlForSubtree(node: ElementContent): string {
    const root: HastRoot = { type: "root", children: [node] };
    const stringifier = unified().use(rehypeStringify, { allowDangerousHtml: true });
    return String(stringifier.stringify(root));
  }

  test("kebab-cased + numeric attrs round-trip through replaceNode", () => {
    const svgNode: ElementContent = {
      type: "element",
      tagName: "svg",
      properties: {
        viewBox: "0 0 24 24",
        width: 16,
        height: 16,
        fill: "currentColor",
      },
      children: [
        {
          type: "element",
          tagName: "path",
          properties: { fillRule: "evenodd", d: "M0 0L24 24" },
          children: [],
        },
      ],
    };

    const replace = defineHastPlugin({
      name: "replace-with-svg",
      element: {
        filter: ["h1"],
        visit() {
          return svgElement({ viewBox: "0 0 24 24", width: 16, height: 16, fill: "currentColor" }, [
            pathElement({ fillRule: "evenodd", d: "M0 0L24 24" }),
          ]);
        },
      },
    });
    const { html } = markdownToHtml("# Hi", { hastPlugins: [replace] });
    expect(html).toContain(expectedHtmlForSubtree(svgNode));
  });

  test("numeric-only properties survive on a fresh HAST element", () => {
    const { html } = markdownToHtml("# Hi", {
      hastPlugins: [
        defineHastPlugin({
          name: "replace-with-numeric",
          element: {
            filter: ["h1"],
            visit() {
              return {
                type: "element",
                tagName: "svg",
                properties: { width: 16, height: 16 },
                children: [],
              } as HastNode;
            },
          },
        }),
      ],
    });
    expect(html).toContain('width="16"');
    expect(html).toContain('height="16"');
  });

  test("xLinkHref namespaced through _hast emit path", () => {
    const { html } = markdownToHtml("# Hi", {
      hastPlugins: [
        defineHastPlugin({
          name: "replace-with-use",
          element: {
            filter: ["h1"],
            visit() {
              return {
                type: "element",
                tagName: "svg",
                properties: { viewBox: "0 0 10 10" },
                children: [
                  {
                    type: "element",
                    tagName: "use",
                    properties: { xLinkHref: "#sym" },
                    children: [],
                  },
                ],
              } as HastNode;
            },
          },
        }),
      ],
    });
    expect(html).toContain('xlink:href="#sym"');
  });
});
