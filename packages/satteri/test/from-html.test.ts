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
    const p = findElement(tree, "p")!;
    expect(p.type).toBe("element");
    if (p.type !== "element") return;
    expect(p.tagName).toBe("p");
    const text = p.children[0]!;
    expect(text.type).toBe("text");
    if (text.type !== "text") return;
    expect(text.value).toBe("hi");
  });

  test("captures element attributes, normalized like property-information", () => {
    const tree = htmlToHast(`<a href="/x" class="y" download tabindex="2">z</a>`);
    const a = findElement(tree, "a")!;
    if (a.type !== "element") return;
    // `class` → `className` array, `download` → boolean, `tabindex` → number.
    expect(a.properties).toMatchObject({
      href: "/x",
      className: ["y"],
      download: true,
      tabIndex: 2,
    });
  });

  test("decodes character references in text", () => {
    const tree = htmlToHast("<p>a &amp; b</p>");
    const p = findElement(tree, "p")!;
    if (p.type !== "element") return;
    const text = p.children[0]!;
    if (text.type !== "text") return;
    expect(text.value).toBe("a & b");
  });

  test("preserves comments", () => {
    const tree = htmlToHast("<div><!--note--></div>");
    const div = findElement(tree, "div")!;
    if (div.type !== "element") return;
    const comment = div.children[0]!;
    expect(comment.type).toBe("comment");
    if (comment.type !== "comment") return;
    expect(comment.value).toBe("note");
  });

  test("emits a doctype node", () => {
    const tree = htmlToHast("<!doctype html><title>t</title>");
    if (tree.type !== "root") return;
    expect(tree.children[0]!.type).toBe("doctype");
  });

  test("recovers from misnested tags", () => {
    // The stray <b> is foster-parented out of the table.
    const tree = htmlToHast("<table><b>x</b><tr><td>y</td></tr></table>");
    expect(tags(tree)).toContain("tbody");
    expect(findElement(tree, "b")).toBeDefined();
    expect(findElement(tree, "td")).toBeDefined();
  });

  test("preserves <template> content", () => {
    // Template children are parsed into a detached content document by the tree
    // builder. Sätteri's hast has no `content` root, so the content is emitted
    // as the template's `children` instead of being dropped. Note this diverges
    // from standard hast (`content` root), so third-party `hast-util-to-html`
    // won't re-serialize it; Sätteri's own renderer round-trips it correctly.
    const tree = htmlToHast("<template><p>hi</p></template>");
    const template = findElement(tree, "template")!;
    expect(template).toBeDefined();
    if (template.type !== "element") return;
    const p = findElement(template, "p")!;
    expect(p).toBeDefined();
    if (p.type !== "element") return;
    const text = p.children[0]!;
    expect(text.type).toBe("text");
    if (text.type !== "text") return;
    expect(text.value).toBe("hi");
  });

  test("parses <noscript> content as markup (scripting disabled)", () => {
    // `hast-util-from-html` parses with scripting disabled, so `<noscript>`
    // content is a real element tree rather than a single raw-text node.
    const tree = htmlToHast("<noscript><link><!--c--></noscript>");
    const noscript = findElement(tree, "noscript")!;
    expect(noscript).toBeDefined();
    if (noscript.type !== "element") return;
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
