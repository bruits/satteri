// The binary node codec (encodeNodeTree + read_binary_node in Rust) replaces
// JSON for structural-mutation payloads. The broad suite already exercises it
// end-to-end via every insert/replace/wrap test; this file pins the trickier
// sub-formats (hast property value kinds, mdast typed fields) and the size win.

import { test, expect } from "vitest";
import { encodeNodeTree } from "../src/command-buffer.js";
import { markdownToHtml, defineHastPlugin, defineMdastPlugin } from "../src/index.js";
import type { HastNode } from "../src/hast/hast-materializer.js";
import type { MdastNode } from "../src/types.js";

function render(md: string, opts: Parameters<typeof markdownToHtml>[1]): string {
  return (markdownToHtml(md, opts) as { html: string }).html;
}

test("hast element properties of every value kind round-trip through the binary codec", () => {
  const plugin = defineHastPlugin({
    name: "wrap-h1",
    element: {
      filter: ["h1"],
      visit(node, ctx) {
        ctx.replaceNode(node, {
          type: "element",
          tagName: "section",
          properties: {
            id: "main", // string
            className: ["a", "b"], // space-separated array
            tabIndex: 2, // number
            hidden: true, // boolean true
            draggable: false, // boolean false
            "data-x": null, // null → stripped
          },
          children: node.children, // reused → _ref passthrough
        } as unknown as HastNode);
      },
    },
  });
  const html = render("# Hi there", { hastPlugins: [plugin] });
  expect(html).toContain("<section");
  expect(html).toContain('id="main"');
  expect(html).toContain('class="a b"');
  expect(html).toContain('tabindex="2"');
  expect(html).toMatch(/\bhidden\b/);
  expect(html).toContain("Hi there"); // passed-through children preserved
  expect(html).not.toContain("data-x"); // null property stripped
});

test("mdast typed string fields (url/title) round-trip through the binary codec", () => {
  const plugin = defineMdastPlugin({
    name: "linkify",
    text(node): MdastNode {
      return {
        type: "link",
        url: "https://example.com",
        title: "T",
        children: [node], // reused text → _ref
      } as unknown as MdastNode;
    },
  });
  const html = render("hello", { mdastPlugins: [plugin] });
  expect(html).toContain('href="https://example.com"');
  expect(html).toContain('title="T"');
  expect(html).toContain("hello");
});

test("mdast code node fields (lang/value) round-trip through the binary codec", () => {
  const plugin = defineMdastPlugin({
    name: "to-code",
    paragraph(): MdastNode {
      return { type: "code", lang: "ts", value: "const x = 1;" } as unknown as MdastNode;
    },
  });
  const html = render("placeholder", { mdastPlugins: [plugin] });
  expect(html).toContain("language-ts");
  expect(html).toContain("const x = 1;");
});

test("binary node encoding is smaller than the equivalent JSON", () => {
  const node = {
    type: "element",
    tagName: "div",
    properties: { className: ["note", "box"], id: "callout" },
    children: [
      {
        type: "element",
        tagName: "p",
        properties: {},
        children: [{ type: "text", value: "Hello, world!" }],
      },
    ],
  };
  const binary = encodeNodeTree(node, () => undefined);
  const json = new TextEncoder().encode(JSON.stringify(node));
  expect(binary.length).toBeLessThan(json.length);
});

test("a passed-through reused subtree encodes as a tiny _ref placeholder", () => {
  // A reused node (id 42) nested in a fresh wrapper encodes as just the wrapper
  // skeleton + a varint ref — far smaller than serializing the subtree.
  const reused = { type: "element", tagName: "article", properties: {}, children: [] };
  const reusedId = (n: unknown) => (n === reused ? 42 : undefined);
  const wrapper = { type: "element", tagName: "div", properties: {}, children: [reused] };
  const withRef = encodeNodeTree(wrapper, reusedId);
  const inlined = encodeNodeTree(wrapper, () => undefined);
  expect(withRef.length).toBeLessThan(inlined.length);
});
