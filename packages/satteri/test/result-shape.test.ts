import { describe, it, expect } from "vitest";
import { markdownToHtml, mdxToJs } from "../src/compile.js";
import { defineMdastPlugin, defineHastPlugin } from "../src/plugin.js";

describe("result.data", () => {
  it("is null when no plugin writes to ctx.data", () => {
    const out = markdownToHtml("# hi");
    expect(out.data).toBeNull();
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

describe("result.diagnostics", () => {
  it("is empty when no plugin reports", () => {
    const out = markdownToHtml("# hi");
    expect(out.diagnostics).toEqual([]);
  });

  it("captures mdast plugin diagnostics with phase 'mdast'", () => {
    const plugin = defineMdastPlugin({
      name: "reporter",
      heading(node, ctx) {
        ctx.report({ message: "heading seen", node, severity: "info" });
      },
    });
    const out = markdownToHtml("# Title", { mdastPlugins: [plugin] });
    expect(out.diagnostics).toHaveLength(1);
    expect(out.diagnostics[0]!.message).toBe("heading seen");
    expect(out.diagnostics[0]!.severity).toBe("info");
    expect(out.diagnostics[0]!.phase).toBe("mdast");
    expect(out.diagnostics[0]!.position).toBeDefined();
  });

  it("captures hast plugin diagnostics with phase 'hast'", () => {
    const plugin = defineHastPlugin({
      name: "reporter",
      element: {
        filter: ["p"],
        visit(node, ctx) {
          ctx.report({ message: "paragraph seen", node, severity: "warning" });
        },
      },
    });
    const out = markdownToHtml("text", { hastPlugins: [plugin] });
    expect(out.diagnostics).toHaveLength(1);
    expect(out.diagnostics[0]).toMatchObject({
      message: "paragraph seen",
      severity: "warning",
      phase: "hast",
    });
    expect(out.diagnostics[0]!.position).toBeDefined();
  });

  it("omits position when report is called without a node", () => {
    const plugin = defineMdastPlugin({
      name: "reporter",
      paragraph(_node, ctx) {
        ctx.report({ message: "nodeless", severity: "info" });
      },
    });
    const out = markdownToHtml("text", { mdastPlugins: [plugin] });
    expect(out.diagnostics[0]).not.toHaveProperty("position");
  });

  it("combines diagnostics from both phases in order", () => {
    const mdastPlugin = defineMdastPlugin({
      name: "mdast-reporter",
      heading(_node, ctx) {
        ctx.report({ message: "from mdast", severity: "info" });
      },
    });
    const hastPlugin = defineHastPlugin({
      name: "hast-reporter",
      element: {
        filter: ["p"],
        visit(_node, ctx) {
          ctx.report({ message: "from hast", severity: "info" });
        },
      },
    });
    const out = markdownToHtml("# h\n\ntext", {
      mdastPlugins: [mdastPlugin],
      hastPlugins: [hastPlugin],
    });
    expect(out.diagnostics.map((d) => [d.phase, d.message])).toEqual([
      ["mdast", "from mdast"],
      ["hast", "from hast"],
    ]);
  });

  it("surfaces diagnostics from mdxToJs too", () => {
    const plugin = defineHastPlugin({
      name: "x",
      element: {
        filter: ["p"],
        visit(_node, ctx) {
          ctx.report({ message: "mdx-side", severity: "error" });
        },
      },
    });
    const out = mdxToJs("hello", { hastPlugins: [plugin] });
    expect(out.diagnostics[0]).toMatchObject({
      phase: "hast",
      severity: "error",
      message: "mdx-side",
    });
  });
});
