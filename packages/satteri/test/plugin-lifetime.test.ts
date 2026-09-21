import { expect, test } from "vitest";
import {
  defineHastPlugin,
  defineMdastPlugin,
  markdownToHtml,
  type HastNode,
  type MdastNode,
} from "../src/index.js";

test.each(["mdast", "hast"] as const)(
  "%s pinned snapshots survive async edits, handle release, and subsequent compiles",
  async (phase) => {
    let readSnapshot: (() => string) | undefined;
    let originalSnapshot: string | undefined;
    const retain = (node: MdastNode | HastNode) => {
      if (readSnapshot) return;
      expect(Object.isFrozen(node)).toBe(true);
      if (!("children" in node) || !node.children) throw new Error("Expected a parent");
      expect(Object.isFrozen(node.children)).toBe(true);
      expect(Object.isFrozen(node.children[0])).toBe(true);
      expect(node.position).toBeDefined();
      expect(Object.isFrozen(node.position?.start)).toBe(true);
      readSnapshot = () => JSON.stringify(node);
      originalSnapshot = readSnapshot();
    };
    const mdast = defineMdastPlugin({
      name: "retain-mdast",
      options: { position: true },
      async text(node, context) {
        const parent = context.parent(node);
        if (!parent) throw new Error("Expected a parent");
        retain(parent);
        await Promise.resolve();
        context.setProperty(node, "value", `${node.value}!`);
      },
    });
    const hast = defineHastPlugin({
      name: "retain-hast",
      options: { position: true },
      async text(node, context) {
        const parent = context.parent(node);
        if (!parent) throw new Error("Expected a parent");
        retain(parent);
        await Promise.resolve();
        context.setProperty(node, "value", `${node.value}!`);
      },
    });
    const compile = (source: string) =>
      phase === "mdast"
        ? markdownToHtml(source, { mdastPlugins: [mdast] })
        : markdownToHtml(source, { hastPlugins: [hast] });
    expect((await compile("original 雪")).html).toBe("<p>original 雪!</p>\n");
    expect(readSnapshot?.()).toBe(originalSnapshot);
    for (const source of ["replacement", "different 😀", "third"])
      expect((await compile(source)).html).toBe(`<p>${source}!</p>\n`);
    expect(readSnapshot?.()).toBe(originalSnapshot);
    expect(originalSnapshot).toContain("original 雪");
    expect(originalSnapshot).not.toContain("original 雪!");
  },
);
