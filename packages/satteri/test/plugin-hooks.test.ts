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

  test("hooks prepend an import and append an export on an empty document", () => {
    const { code } = mdxToJs("", {
      mdastPlugins: [
        {
          name: "inject-esm-empty",
          before(root: MdastRoot, ctx: MdastVisitorContext) {
            ctx.prependChild(root, {
              type: "mdxjsEsm",
              value: 'import { config } from "./config.js";',
            });
          },
          after(root: MdastRoot, ctx: MdastVisitorContext) {
            ctx.appendChild(root, { type: "mdxjsEsm", value: "export const toc = [];" });
          },
        },
      ],
    }) as { code: string };
    expect(code).toContain('import { config } from "./config.js"');
    expect(code).toContain("export const toc = []");
    expect(code.indexOf("import { config }")).toBeLessThan(code.indexOf("export const toc"));
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

  test("sync visitor replacements apply when hooks are present", () => {
    const { html } = markdownToHtml("# Old", {
      mdastPlugins: [
        {
          name: "sync-replace",
          before() {},
          heading(): MdastNode {
            return { type: "paragraph", children: [{ type: "text", value: "New" }] } as MdastNode;
          },
        },
      ],
    }) as { html: string };
    expect(html).toContain("<p>New</p>");
    expect(html).not.toContain("<h1>");
  });

  test("async visitor replacements apply when hooks are present", async () => {
    const { html } = await markdownToHtml("# Old", {
      mdastPlugins: [
        {
          name: "async-replace",
          after() {},
          async heading(): Promise<MdastNode> {
            await Promise.resolve();
            return { type: "paragraph", children: [{ type: "text", value: "New" }] } as MdastNode;
          },
        },
      ],
    });
    expect(html).toContain("<p>New</p>");
    expect(html).not.toContain("<h1>");
  });

  test("{ raw } visitor returns apply when hooks are present", () => {
    const { html } = markdownToHtml("# Old", {
      mdastPlugins: [
        {
          name: "raw-return",
          after() {},
          heading() {
            return { raw: "**bold**" };
          },
        },
      ],
    }) as { html: string };
    expect(html).toContain("<strong>bold</strong>");
    expect(html).not.toContain("<h1>");
  });

  test("visitor and after mutations both land in the same pass", () => {
    const { html } = markdownToHtml("# Hi", {
      mdastPlugins: [
        {
          name: "both",
          heading(node: MdastNode, ctx: MdastVisitorContext) {
            ctx.setProperty(node, "depth", 3);
          },
          after(root: MdastRoot, ctx: MdastVisitorContext) {
            ctx.appendChild(root, {
              type: "paragraph",
              children: [{ type: "text", value: "tail" }],
            });
          },
        },
      ],
    }) as { html: string };
    expect(html).toContain("<h3>Hi</h3>");
    expect(html).toContain("<p>tail</p>");
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

  test("a later plugin sees the root a previous plugin's after appended to", () => {
    let seen: string[] = [];
    markdownToHtml("# Hi", {
      mdastPlugins: [
        {
          name: "a",
          after(root: MdastRoot, ctx: MdastVisitorContext) {
            ctx.appendChild(root, {
              type: "paragraph",
              children: [{ type: "text", value: "A" }],
            });
          },
        },
        {
          name: "b",
          after(root: MdastRoot) {
            seen = root.children.map((c) => c.type);
          },
        },
      ],
    });
    expect(seen).toEqual(["heading", "paragraph"]);
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

  test("after fires exactly once on a non-empty document, with children", () => {
    let calls = 0;
    let childTags: string[] = [];
    markdownToHtml("# Hi\n\nWorld", {
      hastPlugins: [
        {
          name: "after-counter",
          after(root: HastRoot) {
            calls++;
            childTags = root.children
              .filter((c): c is Element => c.type === "element")
              .map((c) => c.tagName);
          },
        },
      ],
    });
    expect(calls).toBe(1);
    expect(childTags).toEqual(["h1", "p"]);
  });

  test("async before settles before element visitors dispatch", async () => {
    let seen: unknown;
    await markdownToHtml("# Hi", {
      hastPlugins: [
        {
          name: "async-seed",
          async before(_root: HastRoot, ctx: HastVisitorContext) {
            await Promise.resolve();
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

  test("after fires after async element visitors settle", async () => {
    const visited: string[] = [];
    let seenAtAfter: string[] = [];
    await markdownToHtml("# One\n\n## Two", {
      hastPlugins: [
        {
          name: "async-order",
          element: {
            filter: ["h1", "h2"],
            async visit(node: Element) {
              await Promise.resolve();
              visited.push(node.tagName);
            },
          },
          after() {
            seenAtAfter = [...visited];
          },
        },
      ],
    });
    expect(seenAtAfter).toEqual(["h1", "h2"]);
  });

  test("async after mutations apply", async () => {
    const { html } = await markdownToHtml("", {
      hastPlugins: [
        {
          name: "async-after",
          async after(root: HastRoot, ctx: HastVisitorContext) {
            await Promise.resolve();
            ctx.appendChild(root, {
              type: "element",
              tagName: "footer",
              properties: {},
              children: [{ type: "text", value: "late" }],
            } as Element);
          },
        },
      ],
    });
    expect(html).toContain("<footer>late</footer>");
  });

  test("async visitor replacements apply when hooks are present", async () => {
    const { html } = await markdownToHtml("# Old", {
      hastPlugins: [
        {
          name: "async-replace",
          before() {},
          element: {
            filter: ["h1"],
            async visit() {
              await Promise.resolve();
              return {
                type: "element",
                tagName: "h2",
                properties: {},
                children: [{ type: "text", value: "New" }],
              } as Element;
            },
          },
        },
      ],
    });
    expect(html).toContain("<h2>New</h2>");
    expect(html).not.toContain("<h1>");
  });
});
