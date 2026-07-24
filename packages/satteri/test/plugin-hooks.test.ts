import { describe, test, expect } from "vitest";
import { markdownToHtml, mdxToJs } from "../src/compile.js";
import type { MdastVisitorContext } from "../src/mdast/mdast-visitor.js";
import type { HastVisitorContext } from "../src/hast/hast-visitor.js";
import type { MdastNode } from "../src/types.js";
import type { Root as MdastRoot } from "mdast";
import type { Root as HastRoot, Element } from "hast";

describe("mdast lifecycle hooks", () => {
  test("after fires exactly once on an empty document", () => {
    let calls = 0;
    let seen: MdastRoot | undefined;
    markdownToHtml("", {
      mdastPlugins: [
        {
          name: "after-counter",
          after(root: MdastRoot) {
            calls++;
            seen = root;
          },
        },
      ],
    });
    expect(calls).toBe(1);
    expect(seen?.type).toBe("root");
    expect(seen?.children).toEqual([]);
  });

  test("after fires exactly once on a non-empty document, with children", () => {
    let calls = 0;
    let childTypes: string[] = [];
    markdownToHtml("# Hi\n\nWorld", {
      mdastPlugins: [
        {
          name: "after-counter",
          after(root: MdastRoot) {
            calls++;
            childTypes = root.children.map((c) => c.type);
          },
        },
      ],
    });
    expect(calls).toBe(1);
    expect(childTypes).toEqual(["heading", "paragraph"]);
  });

  test("before and after bracket the plugin's visitors", () => {
    const order: string[] = [];
    markdownToHtml("# One\n\n## Two", {
      mdastPlugins: [
        {
          name: "order",
          before() {
            order.push("before");
          },
          heading() {
            order.push("heading");
          },
          after() {
            order.push("after");
          },
        },
      ],
    });
    expect(order).toEqual(["before", "heading", "heading", "after"]);
  });

  test("hooks fire on an empty document even with no visitors registered", () => {
    const order: string[] = [];
    markdownToHtml("", {
      mdastPlugins: [
        {
          name: "hooks-only",
          before: () => void order.push("before"),
          after: () => void order.push("after"),
        },
      ],
    });
    expect(order).toEqual(["before", "after"]);
  });

  test("before seeds state that visitors read", () => {
    let seen: unknown;
    markdownToHtml("# Hi", {
      mdastPlugins: [
        {
          name: "seed",
          before(_root: MdastRoot, ctx: MdastVisitorContext) {
            ctx.data.flag = "seeded";
          },
          heading(_node: MdastNode, ctx: MdastVisitorContext) {
            seen = ctx.data.flag;
          },
        },
      ],
    });
    expect(seen).toBe("seeded");
  });

  test("async before settles before visitors dispatch", async () => {
    let seen: unknown;
    await markdownToHtml("# Hi", {
      mdastPlugins: [
        {
          name: "async-seed",
          async before(_root: MdastRoot, ctx: MdastVisitorContext) {
            await Promise.resolve();
            ctx.data.flag = "seeded";
          },
          heading(_node: MdastNode, ctx: MdastVisitorContext) {
            seen = ctx.data.flag;
          },
        },
      ],
    });
    expect(seen).toBe("seeded");
  });

  test("after fires after async visitors settle", async () => {
    const headings: string[] = [];
    let seenAtAfter: string[] = [];
    await markdownToHtml("# One\n\n## Two", {
      mdastPlugins: [
        {
          name: "async-order",
          async heading(node: MdastNode, ctx: MdastVisitorContext) {
            await Promise.resolve();
            headings.push(ctx.textContent(node));
          },
          after() {
            seenAtAfter = [...headings];
          },
        },
      ],
    });
    expect(seenAtAfter).toEqual(["One", "Two"]);
  });

  test("after injects an ESM export on an empty MDX document", () => {
    const { code } = mdxToJs("", {
      mdastPlugins: [
        {
          name: "toc",
          after(root: MdastRoot, ctx: MdastVisitorContext) {
            ctx.appendChild(root, { type: "mdxjsEsm", value: "export const toc = [];" });
          },
        },
      ],
    }) as { code: string };
    expect(code).toContain("const toc = []");
  });

  test("after injects a TOC export built from headings visited in the same pass", () => {
    const headings: string[] = [];
    const { code } = mdxToJs("# One\n\n## Two", {
      mdastPlugins: [
        {
          name: "toc",
          heading(node: MdastNode, ctx: MdastVisitorContext) {
            headings.push(ctx.textContent(node));
          },
          after(root: MdastRoot, ctx: MdastVisitorContext) {
            ctx.appendChild(root, {
              type: "mdxjsEsm",
              value: `export const toc = ${JSON.stringify(headings)};`,
            });
          },
        },
      ],
    }) as { code: string };
    expect(code).toContain('"One"');
    expect(code).toContain('"Two"');
  });

  test("hooks inject an import and an export into an MDX document", () => {
    const { code } = mdxToJs("# Hi\n\n<Aside>note</Aside>", {
      mdastPlugins: [
        {
          name: "inject-esm",
          before(root: MdastRoot, ctx: MdastVisitorContext) {
            ctx.prependChild(root, {
              type: "mdxjsEsm",
              value: 'import { Aside } from "./aside.js";',
            });
          },
          after(root: MdastRoot, ctx: MdastVisitorContext) {
            ctx.appendChild(root, {
              type: "mdxjsEsm",
              value: 'export const meta = { layout: "docs" };',
            });
          },
        },
      ],
    }) as { code: string };
    expect(code).toContain('import { Aside } from "./aside.js"');
    expect(code).toContain('export const meta = { layout: "docs" }');
    // The import participates in compilation: <Aside> resolves to the imported
    // binding instead of the missing-component fallback.
    expect(code).toContain("_jsx(Aside,");
    expect(code).not.toContain("_missingMdxReference");
    expect(code.indexOf("import { Aside }")).toBeLessThan(code.indexOf("export const meta"));
  });

  test("async after mutations apply", async () => {
    const { html } = await markdownToHtml("", {
      mdastPlugins: [
        {
          name: "async-after",
          async after(root: MdastRoot, ctx: MdastVisitorContext) {
            await Promise.resolve();
            ctx.appendChild(root, {
              type: "paragraph",
              children: [{ type: "text", value: "late" }],
            });
          },
        },
      ],
    });
    expect(html).toContain("<p>late</p>");
  });

  test("each plugin gets its own hook invocations, in plugin order", () => {
    const calls: string[] = [];
    markdownToHtml("", {
      mdastPlugins: [
        {
          name: "a",
          before: () => void calls.push("a:before"),
          after: () => void calls.push("a:after"),
        },
        {
          name: "b",
          after: () => void calls.push("b:after"),
        },
      ],
    });
    expect(calls).toEqual(["a:before", "a:after", "b:after"]);
  });
});

describe("hast lifecycle hooks", () => {
  test("after fires exactly once on an empty document", () => {
    let calls = 0;
    let seen: HastRoot | undefined;
    markdownToHtml("", {
      hastPlugins: [
        {
          name: "after-counter",
          after(root: HastRoot) {
            calls++;
            seen = root;
          },
        },
      ],
    });
    expect(calls).toBe(1);
    expect(seen?.type).toBe("root");
    expect(seen?.children).toEqual([]);
  });

  test("before and after bracket the plugin's visitors", () => {
    const order: string[] = [];
    markdownToHtml("# Hi", {
      hastPlugins: [
        {
          name: "order",
          before() {
            order.push("before");
          },
          element: { filter: ["h1"], visit: () => void order.push("h1") },
          after() {
            order.push("after");
          },
        },
      ],
    });
    expect(order).toEqual(["before", "h1", "after"]);
  });

  test("after appends an element to an empty document", () => {
    const { html } = markdownToHtml("", {
      hastPlugins: [
        {
          name: "footer",
          after(root: HastRoot, ctx: HastVisitorContext) {
            ctx.appendChild(root, {
              type: "element",
              tagName: "footer",
              properties: {},
              children: [{ type: "text", value: "generated" }],
            } as Element);
          },
        },
      ],
    }) as { html: string };
    expect(html).toContain("<footer>generated</footer>");
  });

  test("after injects an ESM export on an empty MDX document", () => {
    const { code } = mdxToJs("", {
      hastPlugins: [
        {
          name: "toc",
          after(root: HastRoot, ctx: HastVisitorContext) {
            ctx.appendChild(root, {
              type: "mdxjsEsm",
              value: "export const toc = [];",
            } as unknown as Element);
          },
        },
      ],
    }) as { code: string };
    expect(code).toContain("const toc = []");
  });

  test("before seeds state that element visitors read", () => {
    let seen: unknown;
    markdownToHtml("# Hi", {
      hastPlugins: [
        {
          name: "seed",
          before(_root: HastRoot, ctx: HastVisitorContext) {
            ctx.data.flag = "seeded";
          },
          element: {
            filter: ["h1"],
            visit(_node: Element, ctx: HastVisitorContext) {
              seen = ctx.data.flag;
            },
          },
        },
      ],
    });
    expect(seen).toBe("seeded");
  });
});
