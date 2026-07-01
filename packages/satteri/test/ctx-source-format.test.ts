import { describe, it, expect } from "vitest";
import { markdownToHtml, mdxToJs } from "../src/compile.js";
import { defineMdastPlugin, defineHastPlugin } from "../src/plugin.js";
import type { SourceFormat } from "../src/types.js";

describe("ctx.sourceFormat", () => {
  it('is "markdown" for an mdast plugin under markdownToHtml', () => {
    let seen: SourceFormat | undefined;
    const inspect = defineMdastPlugin({
      name: "inspect",
      paragraph(_node, ctx) {
        seen = ctx.sourceFormat;
      },
    });

    markdownToHtml("hi", { mdastPlugins: [inspect] });
    expect(seen).toBe("markdown");
  });

  it('is "markdown" for a hast plugin under markdownToHtml', () => {
    let seen: SourceFormat | undefined;
    const inspect = defineHastPlugin({
      name: "inspect",
      element: {
        filter: ["p"],
        visit(_node, ctx) {
          seen = ctx.sourceFormat;
        },
      },
    });

    markdownToHtml("hi", { hastPlugins: [inspect] });
    expect(seen).toBe("markdown");
  });

  it('is "mdx" for an mdast plugin under mdxToJs', () => {
    let seen: SourceFormat | undefined;
    const inspect = defineMdastPlugin({
      name: "inspect",
      paragraph(_node, ctx) {
        seen = ctx.sourceFormat;
      },
    });

    mdxToJs("hi", { mdastPlugins: [inspect] });
    expect(seen).toBe("mdx");
  });

  it('is "mdx" for a hast plugin under mdxToJs', () => {
    let seen: SourceFormat | undefined;
    const inspect = defineHastPlugin({
      name: "inspect",
      element: {
        filter: ["p"],
        visit(_node, ctx) {
          seen = ctx.sourceFormat;
        },
      },
    });

    mdxToJs("hi", { hastPlugins: [inspect] });
    expect(seen).toBe("mdx");
  });
});
