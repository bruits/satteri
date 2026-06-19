import { describe, test, expect } from "vitest";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkDirective from "remark-directive";
import remarkRehype from "remark-rehype";
import rehypeStringify from "rehype-stringify";
import type { Root as MdastRoot, Nodes as MdastNodes } from "mdast";
import type { ElementContent, Properties } from "hast";
import { markdownToHtml, defineMdastPlugin } from "../../src/index.js";
import type { MdastPluginInstance } from "../../src/mdast/mdast-visitor.js";
import type { MdastNode } from "../../src/types.js";

// Each test exercises an mdast plugin that mirrors the canonical remark idiom
// of mutating `node.data.hName`/`hProperties`/`hChildren`. We run the same
// transform through both pipelines and compare the resulting HTML so satteri
// stays observably identical to remark-rehype's `applyData` semantics.

type MdastPluginFactory = () => MdastPluginInstance;

interface RemarkPluginAndSatteri {
  /** Mutates the mdast tree in place — the remark idiom. */
  remark: (tree: MdastRoot) => void;
  /** Equivalent satteri plugin shape. */
  satteri: MdastPluginFactory;
}

function visitMdast(tree: MdastNodes, fn: (node: MdastNodes) => void): void {
  fn(tree);
  if ("children" in tree && Array.isArray(tree.children)) {
    for (const child of tree.children as MdastNodes[]) {
      visitMdast(child, fn);
    }
  }
}

function referenceHtml(md: string, plugin: RemarkPluginAndSatteri["remark"]): string {
  const processor = unified()
    .use(remarkParse)
    .use(remarkGfm)
    .use(remarkDirective)
    .use(() => (tree: MdastRoot) => plugin(tree))
    .use(remarkRehype, { allowDangerousHtml: true })
    .use(rehypeStringify, { allowDangerousHtml: true });
  return normalize(String(processor.processSync(md)));
}

// The plugin is built dynamically (computed visitor key), so its type is the
// wide `MdastPluginInstance` and `markdownToHtml` can't prove the run is sync.
// We `await` the maybe-async result rather than asserting it.
async function satteriHtml(md: string, plugin: MdastPluginFactory): Promise<string> {
  const { html } = await markdownToHtml(md, {
    features: { directive: true, gfm: true, frontmatter: false, math: false },
    mdastPlugins: [defineMdastPlugin({ name: "hdata-test", ...plugin() })],
  });
  return normalize(html);
}

function normalize(html: string): string {
  return html
    .replace(/<br>/g, "<br />")
    .replace(/<br\/>/g, "<br />")
    .replace(/<hr>/g, "<hr />")
    .replace(/<hr\/>/g, "<hr />")
    .trim();
}

async function assertHtmlMatches(md: string, plugin: RemarkPluginAndSatteri): Promise<void> {
  const ref = referenceHtml(md, plugin.remark);
  const got = await satteriHtml(md, plugin.satteri);
  expect(got).toBe(ref);
}

// Helpers that do the same thing on both sides for the common case where the
// mdast plugin only writes data fields.

interface DataPatch {
  hName?: string;
  hProperties?: Record<string, unknown>;
  hChildren?: unknown[];
}

function mutateOnRemark(
  predicate: (node: MdastNodes) => boolean,
  patch: DataPatch,
): RemarkPluginAndSatteri["remark"] {
  return (tree) => {
    visitMdast(tree, (node) => {
      if (predicate(node)) {
        const data = ((node as unknown as { data?: Record<string, unknown> }).data ??= {});
        if (patch.hName !== undefined) data.hName = patch.hName;
        if (patch.hProperties !== undefined) data.hProperties = patch.hProperties;
        if (patch.hChildren !== undefined) data.hChildren = patch.hChildren;
      }
    });
  };
}

function mutateOnSatteri(
  type: keyof MdastPluginInstance,
  predicate: (node: MdastNode) => boolean,
  patch: DataPatch,
): MdastPluginFactory {
  return () => ({
    [type]: ((node: MdastNode, ctx: { setProperty: Function }) => {
      if (!predicate(node)) return;
      const existing = ((node as unknown as { data?: Record<string, unknown> }).data ??
        {}) as Record<string, unknown>;
      const next = { ...existing };
      if (patch.hName !== undefined) next.hName = patch.hName;
      if (patch.hProperties !== undefined) next.hProperties = patch.hProperties;
      if (patch.hChildren !== undefined) next.hChildren = patch.hChildren;
      ctx.setProperty(node, "data", next);
    }) as MdastPluginInstance[typeof type],
  });
}

describe("data.hName / hProperties / hChildren conformance vs remark-rehype", () => {
  test("hName on paragraph swaps the tag, keeps children", () => {
    return assertHtmlMatches("Hello world", {
      remark: mutateOnRemark((n) => n.type === "paragraph", { hName: "section" }),
      satteri: mutateOnSatteri("paragraph", (n) => n.type === "paragraph", { hName: "section" }),
    });
  });

  test("hName on heading swaps h1 with div", () => {
    return assertHtmlMatches("# Title\n\nbody", {
      remark: mutateOnRemark((n) => n.type === "heading", { hName: "div" }),
      satteri: mutateOnSatteri("heading", (n) => n.type === "heading", { hName: "div" }),
    });
  });

  test("hProperties merges onto paragraph defaults", () => {
    return assertHtmlMatches("Hi", {
      remark: mutateOnRemark((n) => n.type === "paragraph", {
        hProperties: { className: ["note", "boxed"], id: "intro" },
      }),
      satteri: mutateOnSatteri("paragraph", (n) => n.type === "paragraph", {
        hProperties: { className: ["note", "boxed"], id: "intro" },
      }),
    });
  });

  test("hName + hProperties together", () => {
    return assertHtmlMatches("Body", {
      remark: mutateOnRemark((n) => n.type === "paragraph", {
        hName: "aside",
        hProperties: { className: ["note"] },
      }),
      satteri: mutateOnSatteri("paragraph", (n) => n.type === "paragraph", {
        hName: "aside",
        hProperties: { className: ["note"] },
      }),
    });
  });

  test("hChildren replaces the rendered children", () => {
    return assertHtmlMatches("original", {
      remark: mutateOnRemark((n) => n.type === "paragraph", {
        hChildren: [{ type: "text", value: "replaced" }],
      }),
      satteri: mutateOnSatteri("paragraph", (n) => n.type === "paragraph", {
        hChildren: [{ type: "text", value: "replaced" }],
      }),
    });
  });

  test("hName + hChildren emits an arbitrary subtree", () => {
    const tree = [
      {
        type: "element",
        tagName: "strong",
        properties: {},
        children: [{ type: "text", value: "Hi" }],
      },
    ];
    return assertHtmlMatches("Original body", {
      remark: mutateOnRemark((n) => n.type === "paragraph", {
        hName: "aside",
        hProperties: { className: ["note"] },
        hChildren: tree,
      }),
      satteri: mutateOnSatteri("paragraph", (n) => n.type === "paragraph", {
        hName: "aside",
        hProperties: { className: ["note"] },
        hChildren: tree,
      }),
    });
  });

  test("hProperties on heading", () => {
    return assertHtmlMatches("# Title", {
      remark: mutateOnRemark((n) => n.type === "heading", {
        hProperties: { id: "main", className: ["big"] },
      }),
      satteri: mutateOnSatteri("heading", (n) => n.type === "heading", {
        hProperties: { id: "main", className: ["big"] },
      }),
    });
  });

  test("hName on listItem keeps content", () => {
    return assertHtmlMatches("- one\n- two\n", {
      remark: mutateOnRemark((n) => n.type === "listItem", { hName: "div" }),
      satteri: mutateOnSatteri("listItem", (n) => n.type === "listItem", { hName: "div" }),
    });
  });

  test("hName on container directive (canonical use case)", () => {
    return assertHtmlMatches(":::note\nContent here\n:::", {
      remark: mutateOnRemark(
        (n) => n.type === "containerDirective" && (n as { name?: string }).name === "note",
        { hName: "aside", hProperties: { className: ["note"] } },
      ),
      satteri: mutateOnSatteri(
        "containerDirective",
        (n) => n.type === "containerDirective" && (n as { name?: string }).name === "note",
        { hName: "aside", hProperties: { className: ["note"] } },
      ),
    });
  });

  test("hName on leaf directive", () => {
    return assertHtmlMatches("::break", {
      remark: mutateOnRemark(
        (n) => n.type === "leafDirective" && (n as { name?: string }).name === "break",
        { hName: "hr" },
      ),
      satteri: mutateOnSatteri(
        "leafDirective",
        (n) => n.type === "leafDirective" && (n as { name?: string }).name === "break",
        { hName: "hr" },
      ),
    });
  });

  test("hName on emphasis", () => {
    return assertHtmlMatches("This is *italic* text.", {
      remark: mutateOnRemark((n) => n.type === "emphasis", {
        hName: "i",
        hProperties: { className: ["em"] },
      }),
      satteri: mutateOnSatteri("emphasis", (n) => n.type === "emphasis", {
        hName: "i",
        hProperties: { className: ["em"] },
      }),
    });
  });

  test("hName on strong", () => {
    return assertHtmlMatches("**hello**", {
      remark: mutateOnRemark((n) => n.type === "strong", {
        hName: "b",
      }),
      satteri: mutateOnSatteri("strong", (n) => n.type === "strong", {
        hName: "b",
      }),
    });
  });

  test("hName on link adds rel attribute", () => {
    return assertHtmlMatches("[click](https://example.com)", {
      remark: mutateOnRemark((n) => n.type === "link", {
        hProperties: { rel: ["noopener"], target: "_blank" },
      }),
      satteri: mutateOnSatteri("link", (n) => n.type === "link", {
        hProperties: { rel: ["noopener"], target: "_blank" },
      }),
    });
  });

  test("hName on blockquote", () => {
    return assertHtmlMatches("> quoted", {
      remark: mutateOnRemark((n) => n.type === "blockquote", { hName: "aside" }),
      satteri: mutateOnSatteri("blockquote", (n) => n.type === "blockquote", { hName: "aside" }),
    });
  });

  test("hName on thematicBreak (void)", () => {
    return assertHtmlMatches("---\n", {
      remark: mutateOnRemark((n) => n.type === "thematicBreak", { hName: "hr" }),
      satteri: mutateOnSatteri("thematicBreak", (n) => n.type === "thematicBreak", {
        hName: "hr",
      }),
    });
  });

  test("hProperties null strips an existing override", () => {
    // First add then remove on a paragraph: end state should match the no-op
    // case — vanilla `<p>`.
    return assertHtmlMatches("plain", {
      remark: mutateOnRemark((n) => n.type === "paragraph", {
        hProperties: { className: null as unknown as string[] },
      }),
      satteri: mutateOnSatteri("paragraph", (n) => n.type === "paragraph", {
        hProperties: { className: null as unknown as string[] },
      }),
    });
  });

  test("hChildren with empty array produces empty element", () => {
    return assertHtmlMatches("body", {
      remark: mutateOnRemark((n) => n.type === "paragraph", {
        hName: "div",
        hChildren: [],
      }),
      satteri: mutateOnSatteri("paragraph", (n) => n.type === "paragraph", {
        hName: "div",
        hChildren: [],
      }),
    });
  });

  test("nested element in hChildren", () => {
    const tree = [
      {
        type: "element",
        tagName: "span",
        properties: { className: ["wrap"] },
        children: [
          { type: "text", value: "outer " },
          {
            type: "element",
            tagName: "em",
            properties: {},
            children: [{ type: "text", value: "inner" }],
          },
        ],
      },
    ];
    return assertHtmlMatches("body", {
      remark: mutateOnRemark((n) => n.type === "paragraph", { hChildren: tree }),
      satteri: mutateOnSatteri("paragraph", (n) => n.type === "paragraph", { hChildren: tree }),
    });
  });
});

// Hints set on a *freshly emitted* node — the canonical upstream-Starlight
// `remarkAsides` idiom. The case `setProperty` can't cover at all, since the
// node didn't exist before the plugin ran.

interface DataHints {
  hName?: string;
  hProperties?: Record<string, unknown>;
  hChildren?: ElementContent[];
}

// Single laundering boundary for the asides shape: the mdast type spec
// forbids `Paragraph` as `Paragraph["children"]`, but a paragraph rendered as
// `<aside>` wrapping further paragraphs is exactly what we're testing.
function mkParagraph(data: DataHints, children: readonly MdastNodes[]): MdastNodes {
  return { type: "paragraph", data, children: [...children] } as unknown as MdastNodes;
}

function mkText(value: string): MdastNodes {
  return { type: "text", value };
}

function mkHastElement(
  tagName: string,
  properties: Properties,
  children: ElementContent[],
): ElementContent {
  return { type: "element", tagName, properties, children };
}

function mkHastText(value: string): ElementContent {
  return { type: "text", value };
}

function childrenOf(node: MdastNodes): readonly MdastNodes[] {
  if ("children" in node && Array.isArray(node.children)) return node.children;
  return [];
}

function replaceOnRemark(
  predicate: (node: MdastNodes) => boolean,
  build: (node: MdastNodes) => MdastNodes,
): RemarkPluginAndSatteri["remark"] {
  const walk = (kids: MdastNodes[]): void => {
    for (let i = 0; i < kids.length; i++) {
      const child = kids[i];
      if (!child) continue;
      if (predicate(child)) {
        kids[i] = build(child);
      } else if ("children" in child && Array.isArray(child.children)) {
        walk(child.children);
      }
    }
  };
  return (tree) => walk(tree.children as MdastNodes[]);
}

function paragraphReplacePlugin(build: (node: MdastNodes) => MdastNodes): MdastPluginFactory {
  return () => ({
    paragraph: (node, ctx) => {
      ctx.replaceNode(node, build(node));
    },
  });
}

describe("data hints on freshly emitted mdast nodes (fresh-node path)", () => {
  test("hName on a fresh paragraph from replaceNode swaps the tag", () => {
    const build = (n: MdastNodes): MdastNodes => mkParagraph({ hName: "section" }, childrenOf(n));
    return assertHtmlMatches("Hello", {
      remark: replaceOnRemark((n) => n.type === "paragraph", build),
      satteri: paragraphReplacePlugin(build),
    });
  });

  test("hName + hProperties on a fresh paragraph (asides shape)", () => {
    const build = (n: MdastNodes): MdastNodes =>
      mkParagraph({ hName: "aside", hProperties: { className: ["note"] } }, childrenOf(n));
    return assertHtmlMatches("Body text", {
      remark: replaceOnRemark((n) => n.type === "paragraph", build),
      satteri: paragraphReplacePlugin(build),
    });
  });

  test("hName + hChildren on a fresh paragraph (arbitrary hast subtree)", () => {
    const hChildren: ElementContent[] = [mkHastElement("strong", {}, [mkHastText("Replaced")])];
    const build = (): MdastNodes =>
      mkParagraph({ hName: "aside", hProperties: { className: ["note"] }, hChildren }, []);
    return assertHtmlMatches("Original", {
      remark: replaceOnRemark((n) => n.type === "paragraph", build),
      satteri: paragraphReplacePlugin(build),
    });
  });

  test("hChildren alone on a fresh node replaces rendered children", () => {
    const build = (): MdastNodes => mkParagraph({ hChildren: [mkHastText("fresh")] }, []);
    return assertHtmlMatches("original body", {
      remark: replaceOnRemark((n) => n.type === "paragraph", build),
      satteri: paragraphReplacePlugin(build),
    });
  });

  test("nested data hints: aside paragraph wrapping fresh paragraphs (asides full shape)", () => {
    const build = (): MdastNodes =>
      mkParagraph({ hName: "aside", hProperties: { className: ["note"] } }, [
        mkParagraph({ hName: "p", hProperties: { className: ["title"] } }, [mkText("Note")]),
        mkParagraph({ hName: "div", hProperties: { className: ["body"] } }, [mkText("Note body")]),
      ]);
    return assertHtmlMatches("Note body", {
      remark: replaceOnRemark((n) => n.type === "paragraph", build),
      satteri: paragraphReplacePlugin(build),
    });
  });

  test("hName on a fresh node inserted via prependChild", () => {
    const fresh = (): MdastNodes => mkParagraph({ hName: "header" }, [mkText("Title")]);
    return assertHtmlMatches("> existing\n", {
      remark: (tree) => {
        visitMdast(tree, (node) => {
          if (node.type !== "blockquote") return;
          const kids = node.children as MdastNodes[];
          kids.unshift(fresh());
        });
      },
      satteri: () => ({
        blockquote: (node, ctx) => {
          ctx.prependChild(node, fresh());
        },
      }),
    });
  });

  test("hName on a fresh node inserted via insertBefore", () => {
    const fresh = (): MdastNodes =>
      mkParagraph({ hName: "nav", hProperties: { className: ["toc"] } }, [mkText("TOC")]);
    return assertHtmlMatches("# Heading\n\nBody\n", {
      remark: (tree) => {
        const kids = tree.children as MdastNodes[];
        for (let i = 0; i < kids.length; i++) {
          if (kids[i]?.type === "heading") {
            kids.splice(i, 0, fresh());
            break;
          }
        }
      },
      satteri: () => ({
        heading: (node, ctx) => {
          ctx.insertBefore(node, fresh());
        },
      }),
    });
  });

  test("hProperties on a fresh paragraph (no hName) merges class + id", () => {
    const build = (n: MdastNodes): MdastNodes =>
      mkParagraph({ hProperties: { className: ["lead"], id: "intro" } }, childrenOf(n));
    return assertHtmlMatches("Hi", {
      remark: replaceOnRemark((n) => n.type === "paragraph", build),
      satteri: paragraphReplacePlugin(build),
    });
  });
});
