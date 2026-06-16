import { describe, it, expect } from "vitest";
import { markdownToHtml, mdxToJs } from "../src/compile.js";
import { defineMdastPlugin, defineHastPlugin } from "../src/plugin.js";

describe("result.data", () => {
  it("is an empty object when no plugin writes to ctx.data", () => {
    const out = markdownToHtml("# hi");
    expect(out.data).toEqual({});
  });

  it("preserves non-serializable values (functions, class instances)", () => {
    class Box {
      n: number;
      constructor(n: number) {
        this.n = n;
      }
    }
    const fn = () => 42;
    const plugin = defineMdastPlugin({
      name: "writer",
      paragraph(_node, ctx) {
        ctx.data.fn = fn;
        ctx.data.box = new Box(7);
      },
    });
    const out = markdownToHtml("text", { mdastPlugins: [plugin] });
    expect(out.data.fn).toBe(fn);
    expect(out.data.box).toBeInstanceOf(Box);
    expect((out.data.box as Box).n).toBe(7);
  });

  it("reflects mdast plugin writes", () => {
    const plugin = defineMdastPlugin({
      name: "writer",
      heading(node, ctx) {
        const list = (ctx.data.headings as string[]) ?? [];
        const first = node.children[0];
        if (first && "value" in first) list.push(first.value as string);
        ctx.data.headings = list;
      },
    });
    const out = markdownToHtml("# Alpha\n\n# Beta", { mdastPlugins: [plugin] });
    expect(out.data).toEqual({ headings: ["Alpha", "Beta"] });
  });

  it("reflects hast plugin writes when there are no mdast plugins", () => {
    const plugin = defineHastPlugin({
      name: "writer",
      element: {
        filter: ["h1"],
        visit(_node, ctx) {
          ctx.data.touched = "h1";
        },
      },
    });
    const out = markdownToHtml("# hi", { hastPlugins: [plugin] });
    expect(out.data).toEqual({ touched: "h1" });
  });

  it("is also present on mdxToJs result", () => {
    const plugin = defineMdastPlugin({
      name: "writer",
      paragraph(_node, ctx) {
        ctx.data.fromMdx = true;
      },
    });
    const out = mdxToJs("hello", { mdastPlugins: [plugin] });
    expect(out.data).toEqual({ fromMdx: true });
  });

  it("is an empty object when ctx.data is touched but no key is written", () => {
    const plugin = defineMdastPlugin({
      name: "reader",
      paragraph(_node, ctx) {
        void ctx.data;
      },
    });
    const out = markdownToHtml("hi", { mdastPlugins: [plugin] });
    expect(out.data).toEqual({});
  });
});
