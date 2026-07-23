import { describe, test, expect } from "vitest";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkRehype from "remark-rehype";
import rehypeStringify from "rehype-stringify";
import type { Root as MdastRoot, Nodes as MdastNodes } from "mdast";
import { markdownToHtml, defineMdastPlugin } from "../../src/index.js";
import type { MdastPluginInstance } from "../../src/mdast/mdast-visitor.js";
import type { MdastNode } from "../../src/types.js";

// Custom (user-defined) mdast nodes are modeled on mdast-util-to-hast's
// `defaultUnknownHandler`: a `value` leaf with no children/`data.h*` becomes a
// hast text node, anything else becomes a `<div>` (renamed via `data.hName`,
// merged with `data.hProperties`) whose children are recursed. These tests
// inject the *same* custom node into a remark-rehype reference pipeline and
// into satteri, and assert the HTML matches — so the mainline paths stay
// observably identical to remark rather than to hand-written expectations.
//
// Satteri-only behaviors with no remark equivalent (the reserved `"custom"`
// type string, dropping an empty `value`) stay as unit tests in the sibling
// `test/custom-node.test.ts`.

function referenceHtml(md: string, transform: (tree: MdastRoot) => void): string {
  const processor = unified()
    .use(remarkParse)
    .use(remarkGfm)
    .use(() => (tree: MdastRoot) => transform(tree))
    .use(remarkRehype, { allowDangerousHtml: true })
    .use(rehypeStringify, { allowDangerousHtml: true });
  return normalize(String(processor.processSync(md)));
}

async function satteriHtml(md: string, plugin: MdastPluginInstance): Promise<string> {
  const { html } = await markdownToHtml(md, {
    features: { gfm: true, frontmatter: false, math: false },
    mdastPlugins: [defineMdastPlugin({ name: "custom-conformance", ...plugin })],
  });
  return normalize(html);
}

function normalize(html: string): string {
  // Canonicalize entity encoding style — remark+rehype favours hex (`&#x26;`)
  // while satteri uses named entities — then collapse `&gt;`/`&quot;` to their
  // raw forms (rehype-stringify doesn't encode `>` or `"` outside contexts
  // that require it). All produce semantically identical HTML. Same
  // normalization as `commonmark-spec-json.test.ts`.
  return html
    .replace(/<br>/g, "<br />")
    .replace(/<hr>/g, "<hr />")
    .replace(/&#x3C;/g, "&lt;")
    .replace(/&#x3E;/g, "&gt;")
    .replace(/&#x26;/g, "&amp;")
    .replace(/&#x22;/g, "&quot;")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .trim();
}

type Build = (node: MdastNodes) => MdastNodes;

/** Splice `build(node)` in place of every node matching `predicate` — the
 *  remark idiom of mutating the shared tree. */
function replaceOnRemark(
  predicate: (node: MdastNodes) => boolean,
  build: Build,
): (tree: MdastRoot) => void {
  const walk = (kids: MdastNodes[]): void => {
    for (let i = 0; i < kids.length; i++) {
      const child = kids[i];
      if (!child) continue;
      if (predicate(child)) {
        kids[i] = build(child);
      } else if ("children" in child && Array.isArray(child.children)) {
        walk(child.children as MdastNodes[]);
      }
    }
  };
  return (tree) => walk(tree.children as MdastNodes[]);
}

/** The satteri equivalent: one visitor that returns `build(node)`. */
function replacePlugin(type: keyof MdastPluginInstance, build: Build): MdastPluginInstance {
  return {
    [type]: ((node: MdastNode, ctx: { replaceNode: (n: MdastNode, r: unknown) => void }) => {
      ctx.replaceNode(node, build(node) as unknown as MdastNode);
    }) as MdastPluginInstance[typeof type],
  };
}

function childrenOf(node: MdastNodes): MdastNodes[] {
  if ("children" in node && Array.isArray(node.children)) return node.children as MdastNodes[];
  return [];
}

/** A custom parent node carrying the original node's children. */
function parent(data: Record<string, unknown> | undefined, node: MdastNodes): MdastNodes {
  const n: Record<string, unknown> = { type: "section", children: childrenOf(node) };
  if (data !== undefined) n.data = data;
  return n as unknown as MdastNodes;
}

/** A custom leaf node carrying a text value. */
function leaf(value: string): MdastNodes {
  return { type: "token", value } as unknown as MdastNodes;
}

async function assertMatches(
  md: string,
  type: keyof MdastPluginInstance,
  build: Build,
): Promise<void> {
  const ref = referenceHtml(
    md,
    replaceOnRemark((n) => n.type === type, build),
  );
  const got = await satteriHtml(md, replacePlugin(type, build));
  expect(got).toBe(ref);
}

describe("custom node rendering conformance vs remark-rehype", () => {
  test("parent with hName renders as that element, children recursed", () => {
    return assertMatches("Hello **bold** world", "paragraph", (n) =>
      parent({ hName: "section" }, n),
    );
  });

  test("parent without hName defaults to <div>", () => {
    return assertMatches("Hello **bold** world", "paragraph", (n) => parent(undefined, n));
  });

  test("parent with hName + hProperties merges attributes", () => {
    return assertMatches("Hi there", "paragraph", (n) =>
      parent({ hName: "section", hProperties: { className: ["note"], id: "s1" } }, n),
    );
  });

  test("leaf (value, no children/data) renders as a text node", () => {
    return assertMatches("placeholder", "paragraph", () => leaf("just some text"));
  });

  test("leaf value with special characters is escaped identically", () => {
    return assertMatches("placeholder", "paragraph", () => leaf("a < b & c > d \" e"));
  });

  test("custom parent preserves GFM content inside it (table)", () => {
    const md = "> | a | b |\n> | - | - |\n> | 1 | 2 |\n";
    return assertMatches(md, "blockquote", (n) => parent({ hName: "section" }, n));
  });
});
