import { describe, test, expect } from "vitest";
import { markdownToHtml, mdxToJs, defineMdastPlugin, defineHastPlugin } from "../src/index.js";
import type { MarkdownToHtmlResult } from "../src/index.js";

/** Records its name on each heading, making run order observable. */
function recordMdast(order: string[], name: string) {
  return defineMdastPlugin({
    name,
    heading() {
      order.push(name);
    },
  });
}

/** HAST counterpart of `recordMdast`. */
function recordHast(order: string[], name: string) {
  return defineHastPlugin({
    name,
    element: {
      filter: ["h1", "p"],
      visit() {
        order.push(name);
      },
    },
  });
}

const asyncBundled = defineMdastPlugin({
  name: "async-bundled",
  async code() {
    await new Promise((r) => setTimeout(r, 1));
    return { rawHtml: "<pre>done</pre>" };
  },
});

describe("nested plugin lists", () => {
  test("a bundle runs in its own order at the bundle's position (mdast)", () => {
    const order: string[] = [];
    const bundle = [recordMdast(order, "b"), recordMdast(order, "c")];

    markdownToHtml("# Title", {
      mdastPlugins: [recordMdast(order, "a"), bundle, recordMdast(order, "d")],
    });

    expect(order).toEqual(["a", "b", "c", "d"]);
  });

  test("a bundle runs in its own order at the bundle's position (hast)", () => {
    const order: string[] = [];
    const bundle = [recordHast(order, "b"), recordHast(order, "c")];

    markdownToHtml("# Title", {
      hastPlugins: [recordHast(order, "a"), bundle, recordHast(order, "d")],
    });

    expect(order).toEqual(["a", "b", "c", "d"]);
  });

  test("bundles nest to arbitrary depth", () => {
    const order: string[] = [];

    markdownToHtml("# Title", {
      mdastPlugins: [
        recordMdast(order, "a"),
        [recordMdast(order, "b"), [recordMdast(order, "c"), [recordMdast(order, "d")]]],
        recordMdast(order, "e"),
      ],
    });

    expect(order).toEqual(["a", "b", "c", "d", "e"]);
  });

  test("a readonly bundle is accepted", () => {
    const order: string[] = [];
    const bundle = [recordMdast(order, "a"), recordMdast(order, "b")] as const;

    markdownToHtml("# Title", { mdastPlugins: [bundle] });

    expect(order).toEqual(["a", "b"]);
  });

  test("empty and empty-nested lists compile as if no plugins were passed", () => {
    const result = markdownToHtml("# Title", { mdastPlugins: [[], [[]]], hastPlugins: [[]] });
    expect(result.html).toContain("<h1>Title</h1>");
  });

  test("factories inside a bundle are invoked once per compile", () => {
    let calls = 0;
    const factory = () => {
      calls++;
      return defineMdastPlugin({ name: "counted", heading() {} });
    };

    markdownToHtml("# A", { mdastPlugins: [[factory]] });
    markdownToHtml("# B", { mdastPlugins: [[factory]] });

    expect(calls).toBe(2);
  });

  test("a factory may return a bundle, flattened in place", () => {
    const order: string[] = [];

    markdownToHtml("# Title", {
      mdastPlugins: [
        recordMdast(order, "a"),
        () => [recordMdast(order, "b"), [recordMdast(order, "c")]],
        recordMdast(order, "d"),
      ],
    });

    expect(order).toEqual(["a", "b", "c", "d"]);
  });

  // The point of allowing it: plugins in one bundle can share state that resets
  // per document, which an array of separate factories cannot express.
  test("a factory returning a bundle gives its plugins shared per-compile state", () => {
    const snapshots: string[][] = [];
    const preset = () => {
      const headings: string[] = [];
      return [
        defineMdastPlugin({
          name: "collect",
          heading(node) {
            const child = node.children[0];
            if (child?.type === "text") headings.push(child.value);
          },
        }),
        defineMdastPlugin({
          name: "report",
          paragraph() {
            snapshots.push([...headings]);
          },
        }),
      ];
    };

    markdownToHtml("# One\n\npara", { mdastPlugins: [preset] });
    markdownToHtml("# Two\n\npara", { mdastPlugins: [preset] });

    expect(snapshots).toEqual([["One"], ["Two"]]);
  });

  test("a factory that returns itself is rejected, naming the option", () => {
    const cyclic = (): unknown[] => [cyclic];

    expect(() => markdownToHtml("# T", { mdastPlugins: [cyclic as never] })).toThrowError(
      /^mdastPlugins: plugin factory nesting is too deep/,
    );
    expect(() => markdownToHtml("# T", { hastPlugins: [cyclic as never] })).toThrowError(
      /^hastPlugins: plugin factory nesting is too deep/,
    );
  });

  test("mutations from a bundled plugin are applied", () => {
    const removeHeadings = defineMdastPlugin({
      name: "remove-headings",
      heading(node, ctx) {
        ctx.removeNode(node);
      },
    });
    const addId = defineHastPlugin({
      name: "add-id",
      element: {
        filter: ["p"],
        visit(node, ctx) {
          ctx.setProperty(node, "id", "bundled");
        },
      },
    });

    const { html } = markdownToHtml("# Gone\n\nKept", {
      mdastPlugins: [[removeHeadings]],
      hastPlugins: [[addId]],
    });

    expect(html).not.toContain("Gone");
    expect(html).toContain('id="bundled"');
  });

  test("bundles work in mdxToJs", () => {
    const order: string[] = [];
    const removeHeadings = defineMdastPlugin({
      name: "remove-headings",
      heading(node, ctx) {
        order.push("remove");
        ctx.removeNode(node);
      },
    });

    const { code } = mdxToJs("# Gone\n\nKept", {
      mdastPlugins: [[recordMdast(order, "first"), removeHeadings]],
      hastPlugins: [[recordHast(order, "hast")]],
    });

    expect(code).not.toContain("Gone");
    expect(code).toContain("Kept");
    expect(order).toEqual(["first", "remove", "hast"]);
  });

  test("an async plugin inside a bundle still narrows the result to a Promise", async () => {
    const result: Promise<MarkdownToHtmlResult> = markdownToHtml("```\nhi\n```", {
      mdastPlugins: [[asyncBundled]],
    });

    expect((await result).html).toContain("done");
  });

  test("a bundle mixing sync and async plugins returns a Promise", async () => {
    const order: string[] = [];
    const result: Promise<MarkdownToHtmlResult> = markdownToHtml("# T\n\n```\nhi\n```", {
      mdastPlugins: [[recordMdast(order, "sync"), asyncBundled]],
    });

    expect(result).toBeInstanceOf(Promise);
    expect((await result).html).toContain("done");
    expect(order).toEqual(["sync"]);
  });
});
