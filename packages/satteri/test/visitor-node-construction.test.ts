import { expect, test } from "vitest";
import {
  defineHastPlugin,
  defineMdastPlugin,
  markdownToHtml,
  type MdastNode,
} from "../src/index.js";

type Text = Extract<MdastNode, { type: "text" }>;
type TextVisit = (node: Text, edit: (target: Text, value: string) => void) => void;

function compile(phase: "mdast" | "hast", source: string, visitors: TextVisit[]) {
  return phase === "mdast"
    ? markdownToHtml(source, {
        mdastPlugins: visitors.map((visit, i) =>
          defineMdastPlugin({
            name: `identity-${i}`,
            options: { position: true },
            text(node, context) {
              visit(node, (target, value) => context.setProperty(target, "value", value));
            },
          }),
        ),
      })
    : markdownToHtml(source, {
        hastPlugins: visitors.map((visit, i) =>
          defineHastPlugin({
            name: `identity-${i}`,
            options: { position: true },
            text(node, context) {
              visit(node, (target, value) => context.setProperty(target, "value", value));
            },
          }),
        ),
      });
}

test.each(["mdast", "hast"] as const)(
  "%s matched nodes stay plain; only original objects retain document identity",
  async (phase) => {
    let saved: Text | undefined;
    const result = await compile(phase, "original 雪", [
      (node, edit) => {
        expect(Object.getPrototypeOf(node)).toBe(Object.prototype);
        expect(node.constructor).toBe(Object);
        expect(Reflect.ownKeys(node)).toEqual(
          phase === "mdast" ? ["type", "position", "value"] : ["type", "value", "position"],
        );
        expect(node.position?.start.offset).toBe(0);
        for (const copy of [
          { ...node },
          Object.assign({}, node),
          structuredClone(node),
          JSON.parse(JSON.stringify(node)) as Text,
        ]) {
          expect(copy).toStrictEqual(node);
          expect(() => edit(copy, "invalid")).toThrow("invalid node id");
        }
        saved = node;
      },
      (node, edit) => {
        if (!saved) throw new Error("Expected first visitor to save the node");
        expect(node).not.toBe(saved);
        expect(node).toStrictEqual(saved);
        edit(saved, "edited");
      },
    ]);
    expect(result.html).toBe("<p>edited</p>\n");
    expect(saved?.value).toBe("original 雪");
    for (const nextPhase of ["mdast", "hast"] as const) {
      const other = await compile(nextPhase, "other", [
        (node, edit) => {
          const retained = saved;
          if (!retained) throw new Error("Expected retained node");
          expect(() => edit(retained, "wrong document")).toThrow("invalid node id");
          edit(node, "correct document");
        },
      ]);
      expect(other.html).toBe("<p>correct document</p>\n");
    }
    expect(saved?.value).toBe("original 雪");
  },
);
