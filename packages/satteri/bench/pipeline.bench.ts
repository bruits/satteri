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

const touchAllHastPlugin = () => {
  let count = 0;
  return defineHastPlugin({
    name: "hast-touch-all",
    element: {
      filter: [],
      visit(node, ctx) {
        ctx.setProperty(node, "data-count", String(++count));
      },
    },
    text(node) {
      return { type: "text", value: node.value.toUpperCase() };
    },
  });
};

const touchAllElementsOnly = () => {
  let count = 0;
  return defineHastPlugin({
    name: "hast-elements-only",
    element: {
      filter: [],
      visit(node, ctx) {
        ctx.setProperty(node, "data-count", String(++count));
      },
    },
  });
};

const touchAllTextOnly = defineHastPlugin({
  name: "hast-text-only",
  text(node) {
    return { type: "text", value: node.value.toUpperCase() };
  },
});

const touchAllTextNoop = defineHastPlugin({
  name: "hast-text-noop",
  text() {
    // visit every text node, return nothing
  },
});

const noopMdastPlugin = defineMdastPlugin({
  name: "noop-mdast",
  heading() {},
});

const touchAllMdastPlugin = () => {
  let count = 0;
  return defineMdastPlugin({
    name: "mdast-touch-all",
    heading(node, ctx) {
      ctx.setProperty(node, "depth", ((count++ % 6) + 1) as 1 | 2 | 3 | 4 | 5 | 6);
    },
    text(node) {
      return { type: "text", value: node.value.toUpperCase() };
    },
  });
};

const touchAllMdastTextOnly = defineMdastPlugin({
  name: "mdast-text-only",
  text(node) {
    return { type: "text", value: node.value.toUpperCase() };
  },
});

const touchAllMdastTextNoop = defineMdastPlugin({
  name: "mdast-text-noop",
  text() {},
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

  bench("hast-touch-all (worst case: setProperty + text replace)", () => {
    markdownToHtml(MARKDOWN, { hastPlugins: [touchAllHastPlugin] });
  });

  bench("hast-elements-only (setProperty per element)", () => {
    markdownToHtml(MARKDOWN, { hastPlugins: [touchAllElementsOnly] });
  });

  bench("hast-text-only (text replace per text node)", () => {
    markdownToHtml(MARKDOWN, { hastPlugins: [touchAllTextOnly] });
  });

  bench("hast-text-noop (visit text nodes, no return)", () => {
    markdownToHtml(MARKDOWN, { hastPlugins: [touchAllTextNoop] });
  });

  bench("noop MDAST plugin", () => {
    markdownToHtml(MARKDOWN, { mdastPlugins: [noopMdastPlugin] });
  });

  bench("mdast-touch-all (setProperty + text replace)", () => {
    markdownToHtml(MARKDOWN, { mdastPlugins: [touchAllMdastPlugin] });
  });

  bench("mdast-text-only (text replace per text node)", () => {
    markdownToHtml(MARKDOWN, { mdastPlugins: [touchAllMdastTextOnly] });
  });

  bench("mdast-text-noop (visit text nodes, no return)", () => {
    markdownToHtml(MARKDOWN, { mdastPlugins: [touchAllMdastTextNoop] });
  });

  bench("MDAST + HAST plugins", () => {
    markdownToHtml(MARKDOWN, {
      mdastPlugins: [noopMdastPlugin],
      hastPlugins: [mutatingHastPlugin],
    });
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
