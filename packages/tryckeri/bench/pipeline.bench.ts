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
  compileMarkdownToHtml,
  compileMdxToJs,
  defineHastPlugin,
  defineMdastPlugin,
} from "../src/index.js";
import type { HastNode } from "../src/hast/hast-materializer.js";
import type { HastVisitorContext } from "../src/hast/hast-visitor.js";

const MARKDOWN = readFileSync(new URL("./fixtures/markdown.md", import.meta.url), "utf8");
const MDX = readFileSync(new URL("./fixtures/document.mdx", import.meta.url), "utf8");

// -- Plugins ----------------------------------------------------------------

const noopHastPlugin = defineHastPlugin({
  name: "noop",
  createOnce: () => ({
    element: { filter: [], visit() {} },
  }),
});

const filteredHastPlugin = defineHastPlugin({
  name: "filtered",
  createOnce: () => ({
    element: {
      filter: ["a"],
      visit(_node: HastNode, _ctx: HastVisitorContext) {},
    },
  }),
});

const mutatingHastPlugin = defineHastPlugin({
  name: "mutating",
  createOnce: () => ({
    element: {
      filter: ["h1", "h2", "h3"],
      visit(node: HastNode, ctx: HastVisitorContext) {
        ctx.setProperty(node, "id", "heading");
      },
    },
  }),
});

const noopMdastPlugin = defineMdastPlugin({
  name: "noop-mdast",
  createOnce: () => ({ heading() {} }),
});

// -- Benchmarks -------------------------------------------------------------

describe("compileMarkdownToHtml", () => {
  bench("no plugins", () => {
    compileMarkdownToHtml(MARKDOWN);
  });

  bench("noop HAST plugin (all elements)", () => {
    compileMarkdownToHtml(MARKDOWN, { hastPlugins: [noopHastPlugin] });
  });

  bench("filtered HAST plugin ([a] only)", () => {
    compileMarkdownToHtml(MARKDOWN, { hastPlugins: [filteredHastPlugin] });
  });

  bench("mutating HAST plugin (set id on headings)", () => {
    compileMarkdownToHtml(MARKDOWN, { hastPlugins: [mutatingHastPlugin] });
  });

  bench("noop MDAST plugin", () => {
    compileMarkdownToHtml(MARKDOWN, { mdastPlugins: [noopMdastPlugin] });
  });

  bench("MDAST + HAST plugins", () => {
    compileMarkdownToHtml(MARKDOWN, {
      mdastPlugins: [noopMdastPlugin],
      hastPlugins: [mutatingHastPlugin],
    });
  });
});

describe("compileMdxToJs", () => {
  bench("no plugins", () => {
    compileMdxToJs(MDX);
  });

  bench("noop HAST plugin", () => {
    compileMdxToJs(MDX, { hastPlugins: [noopHastPlugin] });
  });

  bench("MDAST + HAST plugins", () => {
    compileMdxToJs(MDX, {
      mdastPlugins: [noopMdastPlugin],
      hastPlugins: [mutatingHastPlugin],
    });
  });
});
