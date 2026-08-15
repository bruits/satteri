import { describe, test, expect } from "vitest";
import { unified } from "unified";
import rehypeStringify from "rehype-stringify";
import { htmlToHast } from "../src/index.js";
import type { HastNode } from "../src/hast/hast-materializer.js";

/** Collect element tag names in document order. */
function tags(node: HastNode, out: string[] = []): string[] {
  if (node.type === "element") out.push(node.tagName);
  if ("children" in node && node.children) {
    for (const child of node.children as HastNode[]) tags(child, out);
  }
  return out;
}

/** Depth-first find the first element with the given tag name. */
function findElement(node: HastNode, tagName: string): HastNode | undefined {
  if (node.type === "element" && node.tagName === tagName) return node;
  if ("children" in node && node.children) {
    for (const child of node.children as HastNode[]) {
      const found = findElement(child, tagName);
      if (found) return found;
    }
  }
  return undefined;
}

function assertNodeType<T extends HastNode["type"]>(
  node: HastNode | undefined,
  type: T,
): asserts node is Extract<HastNode, { type: T }> {
  expect(node?.type).toBe(type);
}

const stringify = (tree: HastNode): string =>
  unified()
    .use(rehypeStringify)
    .stringify(tree as never);

describe("htmlToHast", () => {
  test("returns a hast root wrapping the parsed document", () => {
    const tree = htmlToHast("<p>hi</p>");
    expect(tree.type).toBe("root");
    expect(tags(tree)).toEqual(["html", "head", "body", "p"]);
  });

  test("materializes structured element and text nodes", () => {
    const tree = htmlToHast("<p>hi</p>");
    const p = findElement(tree, "p");
    assertNodeType(p, "element");
    expect(p.tagName).toBe("p");
    const text = p.children[0];
    assertNodeType(text, "text");
    expect(text.value).toBe("hi");
  });

  test("captures element attributes, normalized like property-information", () => {
    const tree = htmlToHast(`<a href="/x" class="y" download tabindex="2">z</a>`);
    const a = findElement(tree, "a");
    assertNodeType(a, "element");
    // `class` → `className` array, `download` → boolean, `tabindex` → number.
    expect(a.properties).toMatchObject({
      href: "/x",
      className: ["y"],
      download: true,
      tabIndex: 2,
    });
  });

  test("resolves namespaced SVG attributes to their hast properties", () => {
    const tree = htmlToHast(
      `<svg xmlns:xlink="http://www.w3.org/1999/xlink"><use xlink:href="#a" xml:lang="en" foo:bar="x"/></svg>`,
    );
    const svg = findElement(tree, "svg")!;
    if (svg.type !== "element") return;
    expect(svg.properties).toMatchObject({
      xmlnsXLink: "http://www.w3.org/1999/xlink",
    });
    const use = findElement(tree, "use")!;
    if (use.type !== "element") return;
    expect(use.properties).toEqual({
      xLinkHref: "#a",
      xmlLang: "en",
      "foo:bar": "x",
    });
  });

  test("decodes character references in text", () => {
    const tree = htmlToHast("<p>a &amp; b</p>");
    const p = findElement(tree, "p");
    assertNodeType(p, "element");
    const text = p.children[0];
    assertNodeType(text, "text");
    expect(text.value).toBe("a & b");
  });

  test("preserves comments", () => {
    const tree = htmlToHast("<div><!--note--></div>");
    const div = findElement(tree, "div");
    assertNodeType(div, "element");
    const comment = div.children[0];
    assertNodeType(comment, "comment");
    expect(comment.value).toBe("note");
  });

  test("emits a doctype node", () => {
    const tree = htmlToHast("<!doctype html><title>t</title>");
    assertNodeType(tree, "root");
    const doctype = tree.children[0];
    assertNodeType(doctype, "doctype");
  });

  test("recovers from misnested tags", () => {
    // The stray <b> is foster-parented out of the table.
    const tree = htmlToHast("<table><b>x</b><tr><td>y</td></tr></table>");
    expect(tags(tree)).toContain("tbody");
    expect(findElement(tree, "b")).toBeDefined();
    expect(findElement(tree, "td")).toBeDefined();
  });

  test("preserves <template> content", () => {
    // Template content is emitted as `children` rather than the standard hast
    // `content` root, which the arena has no field for.
    const tree = htmlToHast("<template><p>hi</p></template>");
    const template = findElement(tree, "template");
    assertNodeType(template, "element");
    const p = findElement(template, "p");
    assertNodeType(p, "element");
    const text = p.children[0];
    assertNodeType(text, "text");
    expect(text.value).toBe("hi");
  });

  test("parses <noscript> content as markup (scripting disabled)", () => {
    const tree = htmlToHast("<noscript><link><!--c--></noscript>");
    const noscript = findElement(tree, "noscript");
    assertNodeType(noscript, "element");
    expect(tags(noscript)).toEqual(["noscript", "link"]);
    const comment = noscript.children.find((c) => c.type === "comment");
    expect(comment).toBeDefined();
  });

  test("round-trips through the unified/rehype ecosystem", () => {
    const tree = htmlToHast(`<main><a href="/x" class="y">z</a><img src="a.png"></main>`);
    const html = stringify(tree);
    expect(html).toContain(`<a href="/x" class="y">z</a>`);
    expect(html).toContain(`<img src="a.png">`);
    expect(html).toContain("<main>");
  });
});
