import { describe, test, expect } from "vitest";
import { markdownToHtml, mdxToJs, defineHastPlugin, defineMdastPlugin } from "../src/index.js";
import type { LeafDirective } from "../src/types.js";
import type { MdastVisitorContext } from "../src/mdast/mdast-visitor.js";

describe("setField", () => {
  test("renames a hast element", () => {
    const rename = defineHastPlugin({
      name: "rename-anchor",
      element: {
        filter: ["a"],
        visit(node, ctx) {
          ctx.setField(node, "tagName", "span");
        },
      },
    });

    const { html } = markdownToHtml("[label](/target)", { hastPlugins: [rename] });
    expect(html).toContain("<span");
    expect(html).toContain("label");
    expect(html).not.toContain("<a ");
  });

  test("renaming keeps the element's existing properties", () => {
    const rename = defineHastPlugin({
      name: "rename-keeping-props",
      element: {
        filter: ["a"],
        visit(node, ctx) {
          ctx.setField(node, "tagName", "span");
        },
      },
    });

    const { html } = markdownToHtml("[label](/target)", { hastPlugins: [rename] });
    expect(html).toContain('href="/target"');
  });

  test("does not leak the field name into the element's properties", () => {
    const rename = defineHastPlugin({
      name: "rename-no-leak",
      element: {
        filter: ["a"],
        visit(node, ctx) {
          ctx.setField(node, "tagName", "span");
        },
      },
    });

    const { html } = markdownToHtml("[label](/target)", { hastPlugins: [rename] });
    expect(html).not.toContain("tagName");
  });

  test("sets a text node's value", () => {
    const shout = defineHastPlugin({
      name: "shout",
      text(node, ctx) {
        ctx.setField(node, "value", node.value.toUpperCase());
      },
    });

    const { html } = markdownToHtml("hello", { hastPlugins: [shout] });
    expect(html).toContain("HELLO");
  });

  test("renames an mdast directive", () => {
    const after = editDirective("::note\n", (node, ctx) => {
      ctx.setField(node, "name", "warning");
    });
    expect(after?.name).toBe("warning");
  });
});

describe("setAttribute", () => {
  test("adds an attribute to an mdast directive", () => {
    const after = editDirective("::note\n", (node, ctx) => {
      ctx.setAttribute(node, "id", "intro");
    });
    expect(after?.attributes?.id).toBe("intro");
  });

  test("replaces an existing directive attribute rather than duplicating it", () => {
    const after = editDirective("::note{.tip}\n", (node, ctx) => {
      ctx.setAttribute(node, "class", "warn");
    });
    const attributes = after?.attributes ?? {};
    expect(attributes.class).toBe("warn");
    expect(Object.keys(attributes).filter((k) => k === "class")).toHaveLength(1);
  });

  test("leaves the directive's name alone", () => {
    const after = editDirective("::note\n", (node, ctx) => {
      ctx.setAttribute(node, "id", "intro");
    });
    expect(after?.name).toBe("note");
  });

  test("keeps an existing attribute when adding another", () => {
    const after = editDirective("::note{.tip}\n", (node, ctx) => {
      ctx.setAttribute(node, "id", "intro");
    });
    expect(after?.attributes?.class).toBe("tip");
    expect(after?.attributes?.id).toBe("intro");
  });

  test("sets an attribute on an MDX JSX element", () => {
    const annotate = defineHastPlugin({
      name: "annotate-jsx",
      mdxJsxFlowElement: {
        filter: ["Box"],
        visit(node, ctx) {
          ctx.setAttribute(node, "className", "card");
        },
      },
    });

    const { code } = mdxToJs("<Box>hi</Box>", { hastPlugins: [annotate] });
    expect(code).toContain("card");
    expect(code).toContain("className");
  });
});

describe("setField on MDX JSX", () => {
  test("renames the element", () => {
    const rename = defineHastPlugin({
      name: "rename-jsx",
      mdxJsxFlowElement: {
        filter: ["Box"],
        visit(node, ctx) {
          ctx.setField(node, "name", "Card");
        },
      },
    });

    const { code } = mdxToJs("<Box>hi</Box>", { hastPlugins: [rename] });
    expect(code).toContain("Card");
    expect(code).not.toContain("Box");
  });

  test("keeps the element's attributes", () => {
    const rename = defineHastPlugin({
      name: "rename-jsx-keep-attrs",
      mdxJsxFlowElement: {
        filter: ["Box"],
        visit(node, ctx) {
          ctx.setField(node, "name", "Card");
        },
      },
    });

    const { code } = mdxToJs('<Box id="x">hi</Box>', { hastPlugins: [rename] });
    expect(code).toContain("Card");
    expect(code).toContain("x");
  });
});

type DirectiveEdit = (node: Readonly<LeafDirective>, ctx: MdastVisitorContext) => void;

/** A later plugin pass is the only place the applied mutation is observable. */
function editDirective(source: string, edit: DirectiveEdit): LeafDirective | undefined {
  const seen: LeafDirective[] = [];
  const mutate = defineMdastPlugin({ name: "mutate-directive", leafDirective: edit });
  const observe = defineMdastPlugin({
    name: "observe-directive",
    leafDirective(node) {
      seen.push({ ...node, attributes: { ...(node.attributes ?? {}) } });
    },
  });

  markdownToHtml(source, {
    features: { directive: true },
    mdastPlugins: [mutate, observe],
  });
  return seen[0];
}
