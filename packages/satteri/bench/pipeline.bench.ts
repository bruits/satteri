/**
 * End-to-end pipeline benchmarks using the public API.
 *
 * Requires the native Rust module to be built:
 *   pnpm build:native
 *
 * Run with: pnpm bench
 */

import { readFileSync } from "node:fs";
import { bench, describe } from "vitest";
import {
  markdownToHtml,
  mdxToJs,
  markdownToMdast,
  mdxToMdast,
  markdownToHast,
  mdxToHast,
  defineHastPlugin,
  defineMdastPlugin,
} from "../src/index.js";
import type { HastNode } from "../src/hast/hast-materializer.js";
import type { HastVisitorContext } from "../src/hast/hast-visitor.js";
import type { MdastNode } from "../src/types.js";

const MARKDOWN = readFileSync(new URL("./fixtures/markdown.md", import.meta.url), "utf8");
const MDX = readFileSync(new URL("./fixtures/document.mdx", import.meta.url), "utf8");

const noopHastPlugin = defineHastPlugin({
  name: "noop",
  element: { filter: [], visit() {} },
});

const filteredHastPlugin = defineHastPlugin({
  name: "filtered",
  element: {
    filter: ["a"],
    visit(_node: HastNode, _ctx: HastVisitorContext) {},
  },
});

const mutatingHastPlugin = defineHastPlugin({
  name: "mutating",
  element: {
    filter: ["h1", "h2", "h3"],
    visit(node: HastNode, ctx: HastVisitorContext) {
      ctx.setProperty(node, "id", "heading");
    },
  },
});

const noopMdastPlugin = defineMdastPlugin({
  name: "noop-mdast",
  heading() {},
});

// Structural-mutation plugins. Unlike the noop / setProperty plugins above,
// these exercise the binary node payload path — insert / replace / wrap of
// plugin-built node trees — which is the path the JSON→binary work changed.

// ctx.wrapNode — wrap every heading in a fresh <div> (fresh wrapper payload).
const wrapHeadingsHast = defineHastPlugin({
  name: "wrap-headings",
  element: {
    filter: ["h1", "h2", "h3", "h4", "h5", "h6"],
    visit(node: HastNode, ctx: HastVisitorContext) {
      ctx.wrapNode(node, {
        type: "element",
        tagName: "div",
        properties: { className: ["heading-wrap"] },
        children: [],
      } as unknown as HastNode);
    },
  },
});

// ctx.replaceNode keeping children — swap every <a> for a <span> carrying the
// href; the children pass through as `_ref` placeholders.
const replaceLinksHast = defineHastPlugin({
  name: "replace-links",
  element: {
    filter: ["a"],
    visit(node: HastNode, ctx: HastVisitorContext) {
      const el = node as { properties?: Record<string, unknown>; children: HastNode[] };
      ctx.replaceNode(node, {
        type: "element",
        tagName: "span",
        properties: { className: ["link"], "data-href": String(el.properties?.href ?? "") },
        children: el.children,
      } as unknown as HastNode);
    },
  },
});

// Visitor return value — replace every <strong>/<em> with a fresh <mark>
// wrapping its (passed-through) children.
const returnReplaceHast = defineHastPlugin({
  name: "return-replace",
  element: {
    filter: ["strong", "em"],
    visit(node: HastNode): HastNode {
      return {
        type: "element",
        tagName: "mark",
        properties: {},
        children: (node as { children: HastNode[] }).children,
      } as unknown as HastNode;
    },
  },
});

// ctx.replaceNode (MDAST) keeping children — swap every link for an emphasis.
const replaceLinksMdast = defineMdastPlugin({
  name: "replace-links-mdast",
  link(node, ctx) {
    ctx.replaceNode(node, {
      type: "emphasis",
      children: node.children,
    } as unknown as MdastNode);
  },
});

// ctx.insertAfter (MDAST) — drop a thematic break after every heading.
const insertAfterMdast = defineMdastPlugin({
  name: "insert-after-mdast",
  heading(node, ctx) {
    ctx.insertAfter(node, { type: "thematicBreak" } as unknown as MdastNode);
  },
});

// Visitor return value (MDAST) building a brand-new subtree (no passthrough) —
// the heaviest encode case: every field and child is freshly serialized.
const buildSubtreeMdast = defineMdastPlugin({
  name: "build-subtree-mdast",
  paragraph() {
    return {
      type: "blockquote",
      children: [
        { type: "heading", depth: 3, children: [{ type: "text", value: "Note" }] },
        { type: "paragraph", children: [{ type: "text", value: "Rebuilt paragraph body." }] },
      ],
    } as unknown as MdastNode;
  },
});

// HAST: replace every <p> with a fresh `<div class="note"><p>Rebuilt</p></div>`.
const buildSubtreeHastDecl = defineHastPlugin({
  name: "hast-build-decl",
  element: {
    filter: ["p"],
    visit(node: HastNode, ctx: HastVisitorContext) {
      ctx.replaceNode(node, {
        type: "element",
        tagName: "div",
        properties: { className: ["note"] },
        children: [
          {
            type: "element",
            tagName: "p",
            properties: {},
            children: [{ type: "text", value: "Rebuilt" }],
          },
        ],
      } as unknown as HastNode);
    },
  },
});

describe("markdownToHtml", () => {
  bench("no plugins", () => {
    markdownToHtml(MARKDOWN);
  });

  bench("noop HAST plugin (all elements)", () => {
    markdownToHtml(MARKDOWN, { hastPlugins: [noopHastPlugin] });
  });

  bench("filtered HAST plugin ([a] only)", () => {
    markdownToHtml(MARKDOWN, { hastPlugins: [filteredHastPlugin] });
  });

  bench("mutating HAST plugin (set id on headings)", () => {
    markdownToHtml(MARKDOWN, { hastPlugins: [mutatingHastPlugin] });
  });

  bench("noop MDAST plugin", () => {
    markdownToHtml(MARKDOWN, { mdastPlugins: [noopMdastPlugin] });
  });

  bench("MDAST + HAST plugins", () => {
    markdownToHtml(MARKDOWN, {
      mdastPlugins: [noopMdastPlugin],
      hastPlugins: [mutatingHastPlugin],
    });
  });
});

describe("markdownToHtml (structural mutations)", () => {
  bench("HAST ctx.wrapNode (headings)", () => {
    markdownToHtml(MARKDOWN, { hastPlugins: [wrapHeadingsHast] });
  });

  bench("HAST ctx.replaceNode keep-children (links)", () => {
    markdownToHtml(MARKDOWN, { hastPlugins: [replaceLinksHast] });
  });

  bench("HAST return replace (emphasis→mark)", () => {
    markdownToHtml(MARKDOWN, { hastPlugins: [returnReplaceHast] });
  });

  bench("MDAST ctx.replaceNode keep-children (links)", () => {
    markdownToHtml(MARKDOWN, { mdastPlugins: [replaceLinksMdast] });
  });

  bench("MDAST ctx.insertAfter (headings)", () => {
    markdownToHtml(MARKDOWN, { mdastPlugins: [insertAfterMdast] });
  });

  bench("MDAST return build-subtree (paragraphs)", () => {
    markdownToHtml(MARKDOWN, { mdastPlugins: [buildSubtreeMdast] });
  });

  bench("HAST return build-subtree (paragraphs)", () => {
    markdownToHtml(MARKDOWN, { hastPlugins: [buildSubtreeHastDecl] });
  });
});

describe("mdxToJs", () => {
  bench("no plugins", () => {
    mdxToJs(MDX);
  });

  bench("noop HAST plugin", () => {
    mdxToJs(MDX, { hastPlugins: [noopHastPlugin] });
  });

  bench("MDAST + HAST plugins", () => {
    mdxToJs(MDX, {
      mdastPlugins: [noopMdastPlugin],
      hastPlugins: [mutatingHastPlugin],
    });
  });
});

describe("markdownToMdast", () => {
  bench("markdown", () => {
    markdownToMdast(MARKDOWN);
  });
});

describe("mdxToMdast", () => {
  bench("mdx", () => {
    mdxToMdast(MDX);
  });
});

describe("markdownToHast", () => {
  bench("markdown", () => {
    markdownToHast(MARKDOWN);
  });
});

describe("mdxToHast", () => {
  bench("mdx", () => {
    mdxToHast(MDX);
  });
});
