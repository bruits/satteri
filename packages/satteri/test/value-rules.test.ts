import { describe, expect, test } from "vitest";
import { defineHastPlugin, defineMdastPlugin, markdownToHtml, mdxToJs } from "../src/index.js";

const mdx = '<X a="1" />\n';
const directive = ":::note{a=1}\nx\n:::\n";
const table = "| a | b |\n| - | - |\n| 1 | 2 |\n";

const expression = { type: "mdxJsxAttributeValueExpression", value: "1 + 1" } as const;

function mdxRoutes(value: unknown) {
  return [
    defineMdastPlugin({
      name: "set-attribute",
      mdxJsxFlowElement(node, ctx) {
        ctx.setAttribute(node, "b", value as never);
      },
    }),
    defineMdastPlugin({
      name: "set-field-attributes",
      mdxJsxFlowElement(node, ctx) {
        ctx.setField(node, "attributes", [{ type: "mdxJsxAttribute", name: "b", value }] as never);
      },
    }),
    defineMdastPlugin({
      name: "built",
      mdxJsxFlowElement: (node) => ({
        ...node,
        attributes: [{ type: "mdxJsxAttribute", name: "b", value }] as never,
      }),
    }),
    defineHastPlugin({
      name: "hast-set-attribute",
      mdxJsxFlowElement: {
        filter: [],
        visit(node, ctx) {
          ctx.setAttribute(node, "b", value);
        },
      },
    }),
  ];
}

describe("an MDX JSX attribute value", () => {
  test.each([
    ["a string", "s", 'b: "s"'],
    ["null", null, "b: true"],
    ["true", true, "b: true"],
    ["an expression", expression, "b: 1 + 1"],
  ])("%s is taken the same way by every route", (_label, value, expected) => {
    for (const plugin of mdxRoutes(value)) {
      const options = plugin.name.startsWith("hast")
        ? { hastPlugins: [plugin] }
        : { mdastPlugins: [plugin] };
      expect(mdxToJs(mdx, options as never).code, plugin.name).toContain(expected);
    }
  });

  test.each([
    ["a number", 5, "not 5"],
    ["false", false, "not false"],
    ["another object", { k: 1 }, "not an object"],
  ])("%s is rejected the same way by every route", (_label, value, expected) => {
    for (const plugin of mdxRoutes(value)) {
      const options = plugin.name.startsWith("hast")
        ? { hastPlugins: [plugin] }
        : { mdastPlugins: [plugin] };
      expect(() => mdxToJs(mdx, options as never), plugin.name).toThrow(
        `satteri: plugin "${plugin.name}": MDX attribute "b" takes a string`,
      );
      expect(() => mdxToJs(mdx, options as never), plugin.name).toThrow(expected);
    }
  });
});

describe("a directive attribute value", () => {
  const routes = (value: unknown) => [
    defineMdastPlugin({
      name: "set-attribute",
      containerDirective(node, ctx) {
        ctx.setAttribute(node, "a", value as never);
      },
    }),
    defineMdastPlugin({
      name: "set-field-attributes",
      containerDirective(node, ctx) {
        ctx.setField(node, "attributes", { a: value } as never);
      },
    }),
    defineMdastPlugin({
      name: "built",
      containerDirective: (node) => ({ ...node, attributes: { a: value } as never }),
    }),
  ];

  test("a number is rejected by every route", () => {
    for (const plugin of routes(5)) {
      expect(
        () => markdownToHtml(directive, { features: { directive: true }, mdastPlugins: [plugin] }),
        plugin.name,
      ).toThrow(`satteri: plugin "${plugin.name}": directive attribute "a" takes a string, not 5`);
    }
  });
});

describe("a hast property value", () => {
  const routes = (value: unknown) => [
    defineHastPlugin({
      name: "set-property",
      element: {
        filter: ["p"],
        visit(node, ctx) {
          ctx.setProperty(node, "x", value);
        },
      },
    }),
    defineHastPlugin({
      name: "set-field-properties",
      element: {
        filter: ["p"],
        visit(node, ctx) {
          ctx.setField(node, "properties", { x: value } as never);
        },
      },
    }),
    defineHastPlugin({
      name: "built",
      element: {
        filter: ["p"],
        visit: (node) => ({ ...node, properties: { x: value } as never }),
      },
    }),
  ];

  test.each([
    ["an object", { color: "red" }, "not an object"],
    ["a function", () => 1, "not a function"],
  ])("%s is rejected by every route", (_label, value, expected) => {
    for (const plugin of routes(value)) {
      expect(() => markdownToHtml("a\n", { hastPlugins: [plugin] }), plugin.name).toThrow(
        `satteri: plugin "${plugin.name}": property "x" takes a string`,
      );
      expect(() => markdownToHtml("a\n", { hastPlugins: [plugin] }), plugin.name).toThrow(expected);
    }
  });

  test("data.hProperties takes the same values as properties", () => {
    const plugin = defineMdastPlugin({
      name: "h-properties",
      paragraph(node, ctx) {
        ctx.setField(node, "data", { hProperties: { style: { color: "red" } } });
      },
    });
    expect(() => markdownToHtml("a\n", { mdastPlugins: [plugin] })).toThrow(
      'satteri: plugin "h-properties": property "style" takes a string',
    );
  });
});

describe("fields with a fixed range", () => {
  test("a heading depth outside 1 to 6 is rejected whether set or built", () => {
    const set = defineMdastPlugin({
      name: "set-depth",
      heading(node, ctx) {
        ctx.setField(node, "depth", 7 as never);
      },
    });
    const built = defineMdastPlugin({
      name: "built-depth",
      paragraph: (node) => ({ type: "heading", depth: 0 as never, children: [...node.children] }),
    });
    expect(() => markdownToHtml("# a\n", { mdastPlugins: [set] })).toThrow(
      `a heading's "depth" is an integer from 1 to 6, not 7`,
    );
    expect(() => markdownToHtml("a\n", { mdastPlugins: [built] })).toThrow(
      `a heading's "depth" is an integer from 1 to 6, not 0`,
    );
  });

  test("an align entry other than left, right, center or null is rejected", () => {
    const plugin = defineMdastPlugin({
      name: "align",
      table(node, ctx) {
        ctx.setField(node, "align", ["middle", null] as never);
      },
    });
    expect(() =>
      markdownToHtml(table, { features: { gfm: true }, mdastPlugins: [plugin] }),
    ).toThrow(`a table's "align" holds "left", "right", "center" or null, not "middle"`);
  });

  test("a tag name that would break the HTML is rejected whether set or built", () => {
    const set = defineHastPlugin({
      name: "set-tag",
      element: {
        filter: ["p"],
        visit(node, ctx) {
          ctx.setField(node, "tagName", "my el");
        },
      },
    });
    const built = defineHastPlugin({
      name: "built-tag",
      element: {
        filter: ["p"],
        visit: (node) => ({ ...node, tagName: undefined as never }),
      },
    });
    expect(() => markdownToHtml("a\n", { hastPlugins: [set] })).toThrow(
      `an element's "tagName" is a name without spaces`,
    );
    expect(() => markdownToHtml("a\n", { hastPlugins: [built] })).toThrow(
      `an element's "tagName" is a name without spaces`,
    );
  });
});

describe("arguments and returns", () => {
  test("content that is not a node, or a nested array, names the method and the plugin", () => {
    const cases = [
      defineMdastPlugin({
        name: "insert-null",
        paragraph(node, ctx) {
          ctx.insertAfter(node, null as never);
        },
      }),
      defineMdastPlugin({
        name: "replace-nested",
        paragraph(node, ctx) {
          ctx.replaceNode(node, [[{ type: "thematicBreak" }]] as never);
        },
      }),
    ];
    expect(() => markdownToHtml("a\n", { mdastPlugins: [cases[0]!] })).toThrow(
      'satteri: plugin "insert-null": insertAfter: expected a node or an array of nodes, not null',
    );
    expect(() => markdownToHtml("a\n", { mdastPlugins: [cases[1]!] })).toThrow(
      'satteri: plugin "replace-nested": replaceNode: item 0 of the array is an array',
    );
  });

  test("a visitor returning something other than nodes names the plugin", () => {
    const plugin = defineMdastPlugin({ name: "return-string", paragraph: () => "x" as never });
    expect(() => markdownToHtml("a\n", { mdastPlugins: [plugin] })).toThrow(
      'satteri: plugin "return-string": the "paragraph" visitor returned "x"',
    );
  });

  test("setAttribute on a node without attributes, and setField on type, explain what to use", () => {
    const attribute = defineHastPlugin({
      name: "attribute-on-element",
      element: {
        filter: ["p"],
        visit(node, ctx) {
          ctx.setAttribute(node, "id", "x");
        },
      },
    });
    const type = defineMdastPlugin({
      name: "set-type",
      paragraph(node, ctx) {
        ctx.setField(node, "type" as never, "heading" as never);
      },
    });
    expect(() => markdownToHtml("a\n", { hastPlugins: [attribute] })).toThrow(
      "set an element's properties with setProperty",
    );
    expect(() => markdownToHtml("a\n", { mdastPlugins: [type] })).toThrow(
      `a node's "type" cannot change in place; replace the node with replaceNode`,
    );
  });
});
