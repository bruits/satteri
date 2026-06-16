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

// Plugins that transform the tree — the path most plugins exercise. Two
// representative shapes: keeping children (passthrough) and building a fresh
// subtree.

// Keep children: swap every <a> for a <span> carrying the href; children pass
// through by reference.
const replaceLinksHast = defineHastPlugin({
  name: "replace-links",
  element: {
    filter: ["a"],
    visit(node, ctx) {
      ctx.replaceNode(node, {
        type: "element",
        tagName: "span",
        properties: { className: ["link"], "data-href": String(node.properties.href ?? "") },
        children: node.children,
      });
    },
  },
});

// MDAST mirror of the keep-children fast path: passing `node.children` through
// compiles them to refs and skips the arena snapshot.
const replaceLinksMdast = defineMdastPlugin({
  name: "replace-links-mdast",
  link(node, ctx) {
    ctx.replaceNode(node, { type: "emphasis", children: node.children });
  },
});

// Build a fresh subtree: replace every paragraph with a blockquote it constructs.
const buildSubtreeMdast = defineMdastPlugin({
  name: "build-subtree-mdast",
  paragraph() {
    return {
      type: "blockquote",
      children: [
        { type: "heading", depth: 3, children: [{ type: "text", value: "Note" }] },
        { type: "paragraph", children: [{ type: "text", value: "Rebuilt paragraph body." }] },
      ],
    } satisfies MdastNode;
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

describe("markdownToHtml (plugin transforms)", () => {
  bench("HAST replaceNode keep-children (links)", () => {
    markdownToHtml(MARKDOWN, { hastPlugins: [replaceLinksHast] });
  });

  bench("MDAST replaceNode keep-children (links)", () => {
    markdownToHtml(MARKDOWN, { mdastPlugins: [replaceLinksMdast] });
  });

  bench("MDAST build-subtree (paragraphs)", () => {
    markdownToHtml(MARKDOWN, { mdastPlugins: [buildSubtreeMdast] });
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
