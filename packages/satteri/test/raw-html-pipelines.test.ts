import { test, expect } from "vitest";
import { markdownToHtml, defineMdastPlugin, defineHastPlugin } from "../src/index.js";

// `rawHtml` is applied during MDAST→HAST conversion, so every pipeline — the
// no-plugin fast path, the MDAST-plugin fused tail, and the full
// plugin-capable path — must reparse embedded raw HTML identically.

// The class whitespace and the missing <tbody> are only normalized when the
// reparse actually ran, so they discriminate reparsed from verbatim output.
const src = '<div class="a  b">\n\n**hi**\n\n</div>\n\n<table><tr><td>x</td></tr></table>\n';

function sync<T>(r: T | Promise<T>): T {
  if (r instanceof Promise) throw new Error("expected sync");
  return r;
}

const noopMdast = defineMdastPlugin({ name: "noop", heading() {} });
const divToSection = defineHastPlugin({
  name: "div-to-section",
  element: {
    filter: ["div"],
    visit(node) {
      return {
        type: "element",
        tagName: "section",
        properties: node.properties,
        children: node.children,
      };
    },
  },
});

function expectReparsed(html: string) {
  expect(html).toContain('<div class="a b">');
  expect(html).toContain("<tbody>");
  expect(html).toContain("<strong>hi</strong>");
}

test("fast path (no plugins) applies rawHtml", () => {
  const { html } = sync(markdownToHtml(src, { features: { rawHtml: true } }));
  expectReparsed(html);
});

test("mdast-plugin path applies rawHtml", () => {
  const { html } = sync(
    markdownToHtml(src, { features: { rawHtml: true }, mdastPlugins: [noopMdast] }),
  );
  expectReparsed(html);
});

test("hast plugins see reparsed elements on the mdast+hast plugin path", () => {
  const { html } = sync(
    markdownToHtml(src, {
      features: { rawHtml: true },
      mdastPlugins: [noopMdast],
      hastPlugins: [divToSection],
    }),
  );
  expect(html).toContain('<section class="a b">');
  expect(html).toContain("<tbody>");
});

test("rawHtml off leaves raw HTML verbatim on every path", () => {
  const { html } = sync(markdownToHtml(src, { mdastPlugins: [noopMdast] }));
  expect(html).toContain('class="a  b"');
  expect(html).not.toContain("<tbody>");
});

test("plugin-spliced raw HTML is reparsed too", () => {
  const splicer = defineMdastPlugin({
    name: "splicer",
    code(node) {
      if (node.lang !== "box") return;
      return { raw: `<aside class="n  m">${node.value}</aside>` };
    },
  });
  const { html } = sync(
    markdownToHtml("before\n\n```box\nhi\n```\n", {
      features: { rawHtml: true },
      mdastPlugins: [splicer],
    }),
  );
  expect(html).toContain('<aside class="n m">hi</aside>');
});
