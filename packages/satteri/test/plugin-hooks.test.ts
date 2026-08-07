import { describe, test, expect } from "vitest";
import { markdownToHtml, mdxToJs } from "../src/compile.js";
import { defineMdastPlugin, defineHastPlugin } from "../src/plugin.js";
import type { HastNode, MdastNode } from "../src/types.js";
import type { Root as MdastRoot } from "mdast";
import type { Root as HastRoot, Element } from "hast";

describe("mdast lifecycle hooks", () => {
  test("after fires exactly once on an empty document", () => {
    let calls = 0;
    let seen: MdastRoot | undefined;
    markdownToHtml("", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "after-counter",
          after(root) {
            calls++;
            seen = root;
          },
        }),
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
        defineMdastPlugin({
          name: "after-counter",
          after(root) {
            calls++;
            childTypes = root.children.map((c) => c.type);
          },
        }),
      ],
    });
    expect(calls).toBe(1);
    expect(childTypes).toEqual(["heading", "paragraph"]);
  });

  test("before and after bracket the plugin's visitors", () => {
    const order: string[] = [];
    markdownToHtml("# One\n\n## Two", {
      mdastPlugins: [
        defineMdastPlugin({
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
        }),
      ],
    });
    expect(order).toEqual(["before", "heading", "heading", "after"]);
  });

  test("hooks fire on an empty document even with no visitors registered", () => {
    const order: string[] = [];
    markdownToHtml("", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "hooks-only",
          before: () => void order.push("before"),
          after: () => void order.push("after"),
        }),
      ],
    });
    expect(order).toEqual(["before", "after"]);
  });

  test("before seeds state that visitors read", () => {
    let seen: unknown;
    markdownToHtml("# Hi", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "seed",
          before(_root, ctx) {
            ctx.data.flag = "seeded";
          },
          heading(_node, ctx) {
            seen = ctx.data.flag;
          },
        }),
      ],
    });
    expect(seen).toBe("seeded");
  });

  test("async before settles before visitors dispatch", async () => {
    let seen: unknown;
    await markdownToHtml("# Hi", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "async-seed",
          async before(_root, ctx) {
            await Promise.resolve();
            ctx.data.flag = "seeded";
          },
          heading(_node, ctx) {
            seen = ctx.data.flag;
          },
        }),
      ],
    });
    expect(seen).toBe("seeded");
  });

  test("after fires after async visitors settle", async () => {
    const headings: string[] = [];
    let seenAtAfter: string[] = [];
    await markdownToHtml("# One\n\n## Two", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "async-order",
          async heading(node, ctx) {
            await Promise.resolve();
            headings.push(ctx.textContent(node));
          },
          after() {
            seenAtAfter = [...headings];
          },
        }),
      ],
    });
    expect(seenAtAfter).toEqual(["One", "Two"]);
  });

  test("after injects an ESM export on an empty MDX document", () => {
    const { code } = mdxToJs("", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "toc",
          after(root, ctx) {
            ctx.appendChild(root, { type: "mdxjsEsm", value: "export const toc = [];" });
          },
        }),
      ],
    });
    expect(code).toContain("const toc = []");
  });

  test("after injects a TOC export built from headings visited in the same pass", () => {
    const headings: string[] = [];
    const { code } = mdxToJs("# One\n\n## Two", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "toc",
          heading(node, ctx) {
            headings.push(ctx.textContent(node));
          },
          after(root, ctx) {
            ctx.appendChild(root, {
              type: "mdxjsEsm",
              value: `export const toc = ${JSON.stringify(headings)};`,
            });
          },
        }),
      ],
    });
    expect(code).toContain('"One"');
    expect(code).toContain('"Two"');
  });

  test("hooks prepend an import and append an export on an empty document", () => {
    const { code } = mdxToJs("", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "inject-esm-empty",
          before(root, ctx) {
            ctx.prependChild(root, {
              type: "mdxjsEsm",
              value: 'import { config } from "./config.js";',
            });
          },
          after(root, ctx) {
            ctx.appendChild(root, { type: "mdxjsEsm", value: "export const toc = [];" });
          },
        }),
      ],
    });
    expect(code).toContain('import { config } from "./config.js"');
    expect(code).toContain("export const toc = []");
    expect(code.indexOf("import { config }")).toBeLessThan(code.indexOf("export const toc"));
  });

  test("hooks inject an import and an export into an MDX document", () => {
    const { code } = mdxToJs("# Hi\n\n<Aside>note</Aside>", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "inject-esm",
          before(root, ctx) {
            ctx.prependChild(root, {
              type: "mdxjsEsm",
              value: 'import { Aside } from "./aside.js";',
            });
          },
          after(root, ctx) {
            ctx.appendChild(root, {
              type: "mdxjsEsm",
              value: 'export const meta = { layout: "docs" };',
            });
          },
        }),
      ],
    });
    expect(code).toContain('import { Aside } from "./aside.js"');
    expect(code).toContain('export const meta = { layout: "docs" }');
    // The injected import participates in compilation, so <Aside> is a real binding.
    expect(code).toContain("_jsx(Aside,");
    expect(code).not.toContain("_missingMdxReference");
    expect(code.indexOf("import { Aside }")).toBeLessThan(code.indexOf("export const meta"));
  });

  test("async after mutations apply", async () => {
    const { html } = await markdownToHtml("", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "async-after",
          async after(root, ctx) {
            await Promise.resolve();
            ctx.appendChild(root, {
              type: "paragraph",
              children: [{ type: "text", value: "late" }],
            });
          },
        }),
      ],
    });
    expect(html).toContain("<p>late</p>");
  });

  test("sync visitor replacements apply when hooks are present", () => {
    const { html } = markdownToHtml("# Old", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "sync-replace",
          before() {},
          heading() {
            return { type: "paragraph", children: [{ type: "text", value: "New" }] };
          },
        }),
      ],
    });
    expect(html).toContain("<p>New</p>");
    expect(html).not.toContain("<h1>");
  });

  test("async visitor replacements apply when hooks are present", async () => {
    const { html } = await markdownToHtml("# Old", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "async-replace",
          after() {},
          async heading() {
            await Promise.resolve();
            return { type: "paragraph", children: [{ type: "text", value: "New" }] };
          },
        }),
      ],
    });
    expect(html).toContain("<p>New</p>");
    expect(html).not.toContain("<h1>");
  });

  test("{ raw } visitor returns apply when hooks are present", () => {
    const { html } = markdownToHtml("# Old", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "raw-return",
          after() {},
          heading() {
            return { raw: "**bold**" };
          },
        }),
      ],
    });
    expect(html).toContain("<strong>bold</strong>");
    expect(html).not.toContain("<h1>");
  });

  test("visitor and after mutations both land in the same pass", () => {
    const { html } = markdownToHtml("# Hi", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "both",
          heading(node, ctx) {
            ctx.setProperty(node, "depth", 3);
          },
          after(root, ctx) {
            ctx.appendChild(root, {
              type: "paragraph",
              children: [{ type: "text", value: "tail" }],
            });
          },
        }),
      ],
    });
    expect(html).toContain("<h3>Hi</h3>");
    expect(html).toContain("<p>tail</p>");
  });

  test("each plugin gets its own hook invocations, in plugin order", () => {
    const calls: string[] = [];
    markdownToHtml("", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "a",
          before: () => void calls.push("a:before"),
          after: () => void calls.push("a:after"),
        }),
        defineMdastPlugin({
          name: "b",
          after: () => void calls.push("b:after"),
        }),
      ],
    });
    expect(calls).toEqual(["a:before", "a:after", "b:after"]);
  });

  test("a later plugin sees the root a previous plugin's after appended to", () => {
    let seen: string[] = [];
    markdownToHtml("# Hi", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "a",
          after(root, ctx) {
            ctx.appendChild(root, {
              type: "paragraph",
              children: [{ type: "text", value: "A" }],
            });
          },
        }),
        defineMdastPlugin({
          name: "b",
          after(root) {
            seen = root.children.map((c) => c.type);
          },
        }),
      ],
    });
    expect(seen).toEqual(["heading", "paragraph"]);
  });

  test("after swaps the whole document for a new root", () => {
    const { html } = markdownToHtml("# Hi\n\nWorld", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "swap",
          after(root, ctx) {
            ctx.replaceNode(root, {
              type: "root",
              children: [{ type: "paragraph", children: [{ type: "text", value: "swapped" }] }],
            });
          },
        }),
      ],
    });
    expect(html).toBe("<p>swapped</p>\n");
  });

  test("before swaps the whole document for a new root", () => {
    const { html } = markdownToHtml("# Hi", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "swap",
          before(root, ctx) {
            ctx.replaceNode(root, { type: "root", children: [{ type: "thematicBreak" }] });
          },
        }),
      ],
    });
    expect(html).toBe("<hr>\n");
  });

  test("the root can be replaced on an empty document", () => {
    const { html } = markdownToHtml("", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "swap",
          after(root, ctx) {
            ctx.replaceNode(root, {
              type: "root",
              children: [{ type: "paragraph", children: [{ type: "text", value: "from empty" }] }],
            });
          },
        }),
      ],
    });
    expect(html).toBe("<p>from empty</p>\n");
  });

  test("a replacement root keeps the original children it reuses", () => {
    const { html } = markdownToHtml("# Hi\n\nWorld", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "swap",
          after(root, ctx) {
            ctx.replaceNode(root, {
              type: "root",
              children: [{ type: "thematicBreak" }, ...root.children],
            });
          },
        }),
      ],
    });
    expect(html).toBe("<hr>\n<h1>Hi</h1>\n<p>World</p>\n");
  });

  test("replacing the root with an empty root empties the document", () => {
    const { html } = markdownToHtml("# Hi", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "swap",
          after(root, ctx) {
            ctx.replaceNode(root, { type: "root", children: [] });
          },
        }),
      ],
    });
    expect(html).toBe("");
  });

  test("a later plugin sees the root a previous plugin's after replaced", () => {
    let seen: string[] = [];
    markdownToHtml("# Hi", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "swap",
          after(root, ctx) {
            ctx.replaceNode(root, { type: "root", children: [{ type: "thematicBreak" }] });
          },
        }),
        defineMdastPlugin({
          name: "observe",
          after(root) {
            seen = root.children.map((c) => c.type);
          },
        }),
      ],
    });
    expect(seen).toEqual(["thematicBreak"]);
  });

  test("a replacement root can prepend an ESM export ahead of the document", () => {
    const { code } = mdxToJs("# Hi", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "toc",
          after(root, ctx) {
            ctx.replaceNode(root, {
              type: "root",
              children: [
                { type: "mdxjsEsm", value: "export const toc = [];" } as unknown as MdastNode,
                ...root.children,
              ],
            });
          },
        }),
      ],
    });
    expect(code).toContain("export const toc = []");
    expect(code).toContain('"h1"');
  });

  test("a replacement root with _keepChildren keeps the document's children", () => {
    const { html } = markdownToHtml("# Hi", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "swap",
          after(root, ctx) {
            ctx.replaceNode(root, { type: "root", _keepChildren: true } as unknown as MdastRoot);
          },
        }),
      ],
    });
    expect(html).toBe("<h1>Hi</h1>\n");
  });

  test("a root is still unencodable as content for any other node", () => {
    expect(() =>
      markdownToHtml("# Hi", {
        mdastPlugins: [
          defineMdastPlugin({
            name: "bad",
            heading(node, ctx) {
              ctx.replaceNode(node, { type: "root", children: [] });
            },
          }),
        ],
      }),
    ).toThrow(/cannot encode replacement content of type "root"/);
  });

  test("replacing the root with a non-root node is rejected", () => {
    expect(() =>
      markdownToHtml("# Hi", {
        mdastPlugins: [
          defineMdastPlugin({
            name: "bad",
            after(root, ctx) {
              ctx.replaceNode(root, {
                type: "paragraph",
                children: [{ type: "text", value: "p" }],
              });
            },
          }),
        ],
      }),
    ).toThrow(/replaceNode on the document root takes a `root`, not "paragraph"/);
  });

  test("replacing the root with a non-root array tail is rejected", () => {
    expect(() =>
      markdownToHtml("# Hi", {
        mdastPlugins: [
          defineMdastPlugin({
            name: "bad",
            after(root, ctx) {
              ctx.replaceNode(root, [{ type: "thematicBreak" }]);
            },
          }),
        ],
      }),
    ).toThrow(/replaceNode on the document root takes a `root`/);
  });

  test("replacing the root with a list of nodes is rejected before anything is queued", () => {
    expect(() =>
      markdownToHtml("# Hi", {
        mdastPlugins: [
          defineMdastPlugin({
            name: "bad",
            after(root, ctx) {
              ctx.replaceNode(root, [
                { type: "thematicBreak" },
                { type: "root", children: [] } as unknown as MdastNode,
              ]);
            },
          }),
        ],
      }),
    ).toThrow(/replaceNode on the document root takes a `root`/);
  });

  test("a raw string replaces the whole document", () => {
    const fromMarkdown = markdownToHtml("# Hi\n\nWorld", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "raw-swap",
          after: (root, ctx) => ctx.replaceNode(root, { raw: "# New\n\nfrom raw markdown" }),
        }),
      ],
    });
    expect(fromMarkdown.html).toBe("<h1>New</h1>\n<p>from raw markdown</p>\n");

    const fromHtml = markdownToHtml("# Hi", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "raw-html-swap",
          after: (root, ctx) => ctx.replaceNode(root, { rawHtml: "<section>raw html</section>" }),
        }),
      ],
    });
    expect(fromHtml.html).toBe("<section>raw html</section>\n");
  });

  test("a raw root swap still leaves a root for a later plugin's hooks", () => {
    const seen: string[] = [];
    const { html } = markdownToHtml("# Hi", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "raw-swap",
          after: (root, ctx) => ctx.replaceNode(root, { raw: "*replaced*" }),
        }),
        defineMdastPlugin({ name: "observe", after: (root) => void seen.push(root.type) }),
      ],
    });
    expect(seen).toEqual(["root"]);
    expect(html).toBe("<p><em>replaced</em></p>\n");
  });

  test("a visitor reaching the root via parent() can replace it with a raw string", () => {
    const { html } = markdownToHtml("# Hi\n\nWorld", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "raw-via-parent",
          heading(node, ctx) {
            ctx.replaceNode(ctx.parent(node), { raw: "**swapped**" });
          },
        }),
      ],
    });
    expect(html).toBe("<p><strong>swapped</strong></p>\n");
  });

  test("a hook that throws surfaces the error", () => {
    expect(() =>
      markdownToHtml("# Hi", {
        mdastPlugins: [
          defineMdastPlugin({
            name: "boom",
            before() {
              throw new Error("before exploded");
            },
          }),
        ],
      }),
    ).toThrow("before exploded");
  });

  test("an async hook that rejects surfaces the rejection", async () => {
    await expect(
      markdownToHtml("# Hi", {
        mdastPlugins: [
          defineMdastPlugin({
            name: "boom",
            async after() {
              await Promise.resolve();
              throw new Error("after exploded");
            },
          }),
        ],
      }),
    ).rejects.toThrow("after exploded");
  });

  test("a factory plugin gets fresh hook state for each document", () => {
    const collected: string[][] = [];
    const toc = () => {
      const headings: string[] = [];
      return defineMdastPlugin({
        name: "toc",
        heading(node, ctx) {
          headings.push(ctx.textContent(node));
        },
        after() {
          collected.push([...headings]);
        },
      });
    };

    markdownToHtml("# One\n\n## Two", { mdastPlugins: [toc] });
    markdownToHtml("# Three", { mdastPlugins: [toc] });
    expect(collected).toEqual([["One", "Two"], ["Three"]]);
  });

  test("a visitor reaching the root via parent() cannot replace it with a non-root", () => {
    expect(() =>
      markdownToHtml("# Hi", {
        mdastPlugins: [
          defineMdastPlugin({
            name: "bad",
            heading(node, ctx) {
              ctx.replaceNode(ctx.parent(node), { type: "thematicBreak" });
            },
          }),
        ],
      }),
    ).toThrow(/replaceNode on the document root takes a `root`/);
  });

  test("a later plugin's hooks survive every root mutation", () => {
    const mutations = [
      defineMdastPlugin({
        name: "replace",
        after: (root, ctx) =>
          ctx.replaceNode(root, { type: "root", children: [{ type: "thematicBreak" }] }),
      }),
      defineMdastPlugin({
        name: "wrap",
        after: (root, ctx) => ctx.wrapNode(root, { type: "blockquote", children: [] }),
      }),
      defineMdastPlugin({ name: "remove", after: (root, ctx) => ctx.removeNode(root) }),
      defineMdastPlugin({
        name: "setChildren",
        after: (root, ctx) => ctx.setProperty(root, "children", [{ type: "thematicBreak" }]),
      }),
      defineMdastPlugin({
        name: "append",
        after: (root, ctx) => ctx.appendChild(root, { type: "thematicBreak" }),
      }),
    ];
    for (const mutate of mutations) {
      const seen: string[] = [];
      markdownToHtml("# Hi", {
        mdastPlugins: [
          mutate,
          defineMdastPlugin({ name: "observe", after: (root) => void seen.push(root.type) }),
        ],
      });
      expect(seen, mutate.name).toEqual(["root"]);
    }
  });

  test("wrapNode wraps the root, and still rejects a leaf wrapper", () => {
    const { html } = markdownToHtml("# Hi", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "wrap",
          after(root, ctx) {
            ctx.wrapNode(root, { type: "blockquote", children: [] });
          },
        }),
      ],
    });
    expect(html).toBe("<blockquote>\n<h1>Hi</h1>\n</blockquote>\n");

    expect(() =>
      markdownToHtml("# Hi", {
        mdastPlugins: [
          defineMdastPlugin({
            name: "wrap-leaf",
            after(root, ctx) {
              ctx.wrapNode(root, { type: "thematicBreak" } as never);
            },
          }),
        ],
      }),
    ).toThrow(/"thematicBreak" nodes cannot hold children/);
  });

  test("the sibling operations throw on the root", () => {
    for (const op of ["insertBefore", "insertAfter"] as const) {
      expect(() =>
        markdownToHtml("# Hi", {
          mdastPlugins: [
            defineMdastPlugin({
              name: op,
              after(root, ctx) {
                ctx[op](root, { type: "thematicBreak" });
              },
            }),
          ],
        }),
      ).toThrow(/sibling insert on root/);
    }
  });

  test("the child operations work on the root", () => {
    const prepended = markdownToHtml("# Hi\n\nWorld", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "prepend",
          after: (root, ctx) => ctx.prependChild(root, { type: "thematicBreak" }),
        }),
      ],
    });
    expect(prepended.html).toBe("<hr>\n<h1>Hi</h1>\n<p>World</p>\n");

    const inserted = markdownToHtml("# Hi\n\nWorld", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "insert-at",
          after: (root, ctx) => ctx.insertChildAt(root, 1, { type: "thematicBreak" }),
        }),
      ],
    });
    expect(inserted.html).toBe("<h1>Hi</h1>\n<hr>\n<p>World</p>\n");

    const removedChild = markdownToHtml("# Hi\n\nWorld", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "remove-child",
          after: (root, ctx) => ctx.removeChildAt(root, 0),
        }),
      ],
    });
    expect(removedChild.html).toBe("<p>World</p>\n");

    const removedRoot = markdownToHtml("# Hi\n\nWorld", {
      mdastPlugins: [
        defineMdastPlugin({ name: "remove-root", after: (root, ctx) => ctx.removeNode(root) }),
      ],
    });
    expect(removedRoot.html).toBe("");
  });
});

describe("hast lifecycle hooks", () => {
  test("after fires exactly once on an empty document", () => {
    let calls = 0;
    let seen: HastRoot | undefined;
    markdownToHtml("", {
      hastPlugins: [
        defineHastPlugin({
          name: "after-counter",
          after(root) {
            calls++;
            seen = root;
          },
        }),
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
        defineHastPlugin({
          name: "order",
          before() {
            order.push("before");
          },
          element: { filter: ["h1"], visit: () => void order.push("h1") },
          after() {
            order.push("after");
          },
        }),
      ],
    });
    expect(order).toEqual(["before", "h1", "after"]);
  });

  test("after appends an element to an empty document", () => {
    const { html } = markdownToHtml("", {
      hastPlugins: [
        defineHastPlugin({
          name: "footer",
          after(root, ctx) {
            ctx.appendChild(root, {
              type: "element",
              tagName: "footer",
              properties: {},
              children: [{ type: "text", value: "generated" }],
            });
          },
        }),
      ],
    });
    expect(html).toContain("<footer>generated</footer>");
  });

  test("after injects an ESM export on an empty MDX document", () => {
    const { code } = mdxToJs("", {
      hastPlugins: [
        defineHastPlugin({
          name: "toc",
          after(root, ctx) {
            ctx.appendChild(root, {
              type: "mdxjsEsm",
              value: "export const toc = [];",
            } as unknown as HastNode);
          },
        }),
      ],
    });
    expect(code).toContain("const toc = []");
  });

  test("before seeds state that element visitors read", () => {
    let seen: unknown;
    markdownToHtml("# Hi", {
      hastPlugins: [
        defineHastPlugin({
          name: "seed",
          before(_root, ctx) {
            ctx.data.flag = "seeded";
          },
          element: {
            filter: ["h1"],
            visit(_node, ctx) {
              seen = ctx.data.flag;
            },
          },
        }),
      ],
    });
    expect(seen).toBe("seeded");
  });

  test("after fires exactly once on a non-empty document, with children", () => {
    let calls = 0;
    let childTags: string[] = [];
    markdownToHtml("# Hi\n\nWorld", {
      hastPlugins: [
        defineHastPlugin({
          name: "after-counter",
          after(root) {
            calls++;
            childTags = root.children
              .filter((c): c is Element => c.type === "element")
              .map((c) => c.tagName);
          },
        }),
      ],
    });
    expect(calls).toBe(1);
    expect(childTags).toEqual(["h1", "p"]);
  });

  test("async before settles before element visitors dispatch", async () => {
    let seen: unknown;
    await markdownToHtml("# Hi", {
      hastPlugins: [
        defineHastPlugin({
          name: "async-seed",
          async before(_root, ctx) {
            await Promise.resolve();
            ctx.data.flag = "seeded";
          },
          element: {
            filter: ["h1"],
            visit(_node, ctx) {
              seen = ctx.data.flag;
            },
          },
        }),
      ],
    });
    expect(seen).toBe("seeded");
  });

  test("after fires after async element visitors settle", async () => {
    const visited: string[] = [];
    let seenAtAfter: string[] = [];
    await markdownToHtml("# One\n\n## Two", {
      hastPlugins: [
        defineHastPlugin({
          name: "async-order",
          element: {
            filter: ["h1", "h2"],
            async visit(node) {
              await Promise.resolve();
              visited.push(node.tagName);
            },
          },
          after() {
            seenAtAfter = [...visited];
          },
        }),
      ],
    });
    expect(seenAtAfter).toEqual(["h1", "h2"]);
  });

  test("async after mutations apply", async () => {
    const { html } = await markdownToHtml("", {
      hastPlugins: [
        defineHastPlugin({
          name: "async-after",
          async after(root, ctx) {
            await Promise.resolve();
            ctx.appendChild(root, {
              type: "element",
              tagName: "footer",
              properties: {},
              children: [{ type: "text", value: "late" }],
            });
          },
        }),
      ],
    });
    expect(html).toContain("<footer>late</footer>");
  });

  test("async visitor replacements apply when hooks are present", async () => {
    const { html } = await markdownToHtml("# Old", {
      hastPlugins: [
        defineHastPlugin({
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
              } satisfies Element;
            },
          },
        }),
      ],
    });
    expect(html).toContain("<h2>New</h2>");
    expect(html).not.toContain("<h1>");
  });

  test("after swaps the whole document for a new root", () => {
    const { html } = markdownToHtml("# Hi\n\nWorld", {
      hastPlugins: [
        defineHastPlugin({
          name: "swap",
          after(root, ctx) {
            ctx.replaceNode(root, {
              type: "root",
              children: [
                {
                  type: "element",
                  tagName: "main",
                  properties: {},
                  children: [{ type: "text", value: "swapped" }],
                },
              ],
            });
          },
        }),
      ],
    });
    expect(html).toBe("<main>swapped</main>\n");
  });

  test("before replaces the root on an empty document", () => {
    const { html } = markdownToHtml("", {
      hastPlugins: [
        defineHastPlugin({
          name: "swap",
          before(root, ctx) {
            ctx.replaceNode(root, {
              type: "root",
              children: [
                {
                  type: "element",
                  tagName: "p",
                  properties: {},
                  children: [{ type: "text", value: "from empty" }],
                },
              ],
            });
          },
        }),
      ],
    });
    expect(html).toBe("<p>from empty</p>\n");
  });

  test("a replacement root keeps the original children it reuses", () => {
    const { html } = markdownToHtml("# Hi", {
      hastPlugins: [
        defineHastPlugin({
          name: "swap",
          after(root, ctx) {
            ctx.replaceNode(root, {
              type: "root",
              children: [
                ...root.children,
                {
                  type: "element",
                  tagName: "footer",
                  properties: {},
                  children: [{ type: "text", value: "tail" }],
                },
              ],
            });
          },
        }),
      ],
    });
    expect(html).toContain("<h1>Hi</h1>");
    expect(html).toContain("<footer>tail</footer>");
  });

  test("replacing the root with an empty root empties the document", () => {
    const { html } = markdownToHtml("# Hi", {
      hastPlugins: [
        defineHastPlugin({
          name: "swap",
          after(root, ctx) {
            ctx.replaceNode(root, { type: "root", children: [] });
          },
        }),
      ],
    });
    expect(html).toBe("");
  });

  test("a root is still unencodable as content for any other node", () => {
    expect(() =>
      markdownToHtml("# Hi", {
        hastPlugins: [
          defineHastPlugin({
            name: "bad",
            element: {
              filter: ["h1"],
              visit(node, ctx) {
                ctx.replaceNode(node, { type: "root", children: [] });
              },
            },
          }),
        ],
      }),
    ).toThrow(/cannot encode replacement content of type "root"/);
  });

  test("replacing the root with a non-root node is rejected", () => {
    expect(() =>
      markdownToHtml("# Hi", {
        hastPlugins: [
          defineHastPlugin({
            name: "bad",
            after(root, ctx) {
              ctx.replaceNode(root, {
                type: "element",
                tagName: "section",
                properties: {},
                children: [],
              });
            },
          }),
        ],
      }),
    ).toThrow(/replaceNode on the document root takes a `root`, not "element"/);
  });

  test("replacing the root with a list of nodes is rejected before anything is queued", () => {
    expect(() =>
      markdownToHtml("# Hi", {
        hastPlugins: [
          defineHastPlugin({
            name: "bad",
            after(root, ctx) {
              ctx.replaceNode(root, [
                { type: "element", tagName: "hr", properties: {}, children: [] },
                { type: "root", children: [] },
              ]);
            },
          }),
        ],
      }),
    ).toThrow(/replaceNode on the document root takes a `root`/);
  });

  test("hooks fire for every hast plugin, not just the last", () => {
    const order: string[] = [];
    const { html } = markdownToHtml("# Hi", {
      hastPlugins: [
        defineHastPlugin({
          name: "first",
          before: (root) => void order.push(`first:before:${root.children.length}`),
          after(root, ctx) {
            order.push(`first:after:${root.children.length}`);
            ctx.appendChild(root, {
              type: "element",
              tagName: "footer",
              properties: {},
              children: [],
            });
          },
        }),
        defineHastPlugin({
          name: "second",
          before: (root) => void order.push(`second:before:${root.children.length}`),
          after: () => void order.push("second:after"),
        }),
      ],
    });
    expect(order).toEqual(["first:before:1", "first:after:1", "second:before:2", "second:after"]);
    expect(html).toContain("<footer></footer>");
  });

  test("an mdast root swap does not strand the hast hooks", () => {
    const seen: string[] = [];
    const { html } = markdownToHtml("# Hi", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "swap",
          after(root, ctx) {
            ctx.replaceNode(root, { type: "root", children: [{ type: "thematicBreak" }] });
          },
        }),
      ],
      hastPlugins: [
        defineHastPlugin({ name: "observe", after: (root) => void seen.push(root.type) }),
      ],
    });
    expect(seen).toEqual(["root"]);
    expect(html).toBe("<hr>\n");
  });

  test("wrapNode wraps the root, and still rejects a void wrapper", () => {
    const wrap = (tagName: string) =>
      markdownToHtml("# Hi", {
        hastPlugins: [
          defineHastPlugin({
            name: "wrap",
            after(root, ctx) {
              ctx.wrapNode(root, { type: "element", tagName, properties: {}, children: [] });
            },
          }),
        ],
      }).html;

    expect(wrap("div")).toBe("<div><h1>Hi</h1></div>\n");
    expect(() => wrap("br")).toThrow(/<br> is a void element/);
  });

  test("the sibling operations throw on the root", () => {
    for (const op of ["insertBefore", "insertAfter"] as const) {
      expect(() =>
        markdownToHtml("# Hi", {
          hastPlugins: [
            defineHastPlugin({
              name: op,
              after(root, ctx) {
                ctx[op](root, { type: "element", tagName: "hr", properties: {}, children: [] });
              },
            }),
          ],
        }),
      ).toThrow(/sibling insert on root/);
    }
  });
});
