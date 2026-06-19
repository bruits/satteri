import { describe, it, expect } from "vitest";
import { markdownToHtml, mdxToJs } from "../src/compile.js";
import { defineMdastPlugin, defineHastPlugin } from "../src/plugin.js";

describe("ctx.data", () => {
  it("defaults to an empty object when nothing is written", () => {
    let seen: Record<string, unknown> | undefined;
    const inspect = defineMdastPlugin({
      name: "inspect",
      paragraph(_node, ctx) {
        seen = ctx.data;
      },
    });

    markdownToHtml("hi", { mdastPlugins: [inspect] });
    expect(seen).toEqual({});
  });

  it("shares state across mdast plugins on the same document", () => {
    let observed: unknown;
    const writer = defineMdastPlugin({
      name: "writer",
      heading(_node, ctx) {
        ctx.data.headingCount = ((ctx.data.headingCount as number) ?? 0) + 1;
      },
    });
    const reader = defineMdastPlugin({
      name: "reader",
      paragraph(_node, ctx) {
        observed = ctx.data.headingCount;
      },
    });

    markdownToHtml("# A\n\n# B\n\ntext", { mdastPlugins: [writer, reader] });
    expect(observed).toBe(2);
  });

  it("survives the mdast→hast phase boundary", () => {
    let observed: unknown;
    const mdastWriter = defineMdastPlugin({
      name: "mdast-writer",
      heading(node, ctx) {
        const list = (ctx.data.headings as string[]) ?? [];
        const child = node.children[0];
        if (child && "value" in child) list.push(child.value as string);
        ctx.data.headings = list;
      },
    });
    const hastReader = defineHastPlugin({
      name: "hast-reader",
      element: {
        filter: ["p"],
        visit(_node, ctx) {
          observed = ctx.data.headings;
        },
      },
    });

    markdownToHtml("# Alpha\n\n# Beta\n\ntext", {
      mdastPlugins: [mdastWriter],
      hastPlugins: [hastReader],
    });
    expect(observed).toEqual(["Alpha", "Beta"]);
  });

  it("preserves reference identity across the mdast→hast boundary", () => {
    class Collector {
      readonly items: string[] = [];
    }
    let mdastInstance: Collector | undefined;
    let hastInstance: unknown;
    const mdastWriter = defineMdastPlugin({
      name: "mdast-writer",
      heading(_node, ctx) {
        const collector = new Collector();
        ctx.data.collector = collector;
        mdastInstance = collector;
      },
    });
    const hastReader = defineHastPlugin({
      name: "hast-reader",
      element: {
        filter: ["p"],
        visit(_node, ctx) {
          hastInstance = ctx.data.collector;
        },
      },
    });

    markdownToHtml("# A\n\ntext", { mdastPlugins: [mdastWriter], hastPlugins: [hastReader] });
    expect(hastInstance).toBeInstanceOf(Collector);
    expect(hastInstance).toBe(mdastInstance);
  });

  it("hast plugin can write data even without an mdast pass", () => {
    let observed: unknown;
    const hastA = defineHastPlugin({
      name: "hast-a",
      element: {
        filter: ["h1"],
        visit(_node, ctx) {
          ctx.data.touched = true;
        },
      },
    });
    const hastB = defineHastPlugin({
      name: "hast-b",
      element: {
        filter: ["p"],
        visit(_node, ctx) {
          observed = ctx.data.touched;
        },
      },
    });

    markdownToHtml("# Hi\n\ntext", { hastPlugins: [hastA, hastB] });
    expect(observed).toBe(true);
  });

  it("isolates data across separate compile calls", () => {
    let first: unknown;
    let second: unknown;
    let runIdx = 0;
    const factory = () => {
      const myRun = ++runIdx;
      return defineMdastPlugin({
        name: "fresh",
        paragraph(_node, ctx) {
          if (myRun === 1) {
            ctx.data.tag = "first";
            first = ctx.data.tag;
          } else {
            second = ctx.data.tag;
          }
        },
      });
    };

    markdownToHtml("first run", { mdastPlugins: [factory] });
    markdownToHtml("second run", { mdastPlugins: [factory] });
    expect(first).toBe("first");
    expect(second).toBeUndefined();
  });

  it("seeds ctx.data from options.data before plugins run", () => {
    let seen: unknown;
    const reader = defineMdastPlugin({
      name: "reader",
      paragraph(_node, ctx) {
        seen = ctx.data.seeded;
      },
    });

    markdownToHtml("hi", { mdastPlugins: [reader], data: { seeded: "value" } });
    expect(seen).toBe("value");
  });

  it("returns the seeded object so plugin mutations read back on result.data", () => {
    const data: Record<string, unknown> = { astro: { frontmatter: { title: "Original" } } };
    const mutator = defineMdastPlugin({
      name: "mutator",
      paragraph(_node, ctx) {
        (ctx.data.astro as { frontmatter: Record<string, unknown> }).frontmatter.title = "Updated";
      },
    });

    const result = markdownToHtml("body", { mdastPlugins: [mutator], data });
    expect(result.data).toBe(data);
    expect(data.astro).toEqual({ frontmatter: { title: "Updated" } });
  });

  it("seeds ctx.data for mdxToJs as well", () => {
    let seen: unknown;
    const data: Record<string, unknown> = { tag: "in" };
    const reader = defineHastPlugin({
      name: "reader",
      element: {
        filter: ["p"],
        visit(_node, ctx) {
          seen = ctx.data.tag;
          ctx.data.tag = "out";
        },
      },
    });

    const result = mdxToJs("text", { hastPlugins: [reader], data });
    expect(seen).toBe("in");
    expect(result.data).toBe(data);
    expect(data.tag).toBe("out");
  });

  it("supports nested objects and arrays", () => {
    let observed: unknown;
    const writer = defineMdastPlugin({
      name: "writer",
      paragraph(_node, ctx) {
        ctx.data.toc = { items: [{ depth: 1, text: "Intro" }] };
      },
    });
    const reader = defineHastPlugin({
      name: "reader",
      element: {
        filter: ["p"],
        visit(_node, ctx) {
          observed = ctx.data.toc;
        },
      },
    });

    markdownToHtml("para", { mdastPlugins: [writer], hastPlugins: [reader] });
    expect(observed).toEqual({ items: [{ depth: 1, text: "Intro" }] });
  });
});
