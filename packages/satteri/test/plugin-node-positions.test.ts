import { test, expect } from "vitest";
import type { Position } from "unist";
import { defineHastPlugin, defineMdastPlugin, markdownToHtml } from "../src/index.js";

// The emoji and accent put byte offsets ahead of UTF-16 offsets for every later
// node, so an offset written back without conversion would come back shifted.
const source = "# T😀tle\n\ncorps é un\n\nlast *one*";

function paragraphPositions(edit?: Parameters<typeof defineMdastPlugin>[0]["paragraph"]) {
  const seen: (Position | undefined)[] = [];
  const plugins = [
    edit && defineMdastPlugin({ name: "edit", options: { position: true }, paragraph: edit }),
    defineMdastPlugin({
      name: "read",
      options: { position: true },
      paragraph(node) {
        seen.push(node.position);
      },
      custom(node) {
        seen.push(node.position);
      },
    }),
  ];
  markdownToHtml(source, { mdastPlugins: plugins });
  return seen;
}

test("a node a plugin copies keeps its position for later plugins", () => {
  const untouched = paragraphPositions();
  expect(untouched).toHaveLength(2);
  expect(paragraphPositions((node, ctx) => ctx.replaceNode(node, { ...node }))).toEqual(untouched);
});

test("a custom node keeps the position it is given", () => {
  const untouched = paragraphPositions();
  const wrapped = paragraphPositions((node, ctx) =>
    ctx.replaceNode(node, {
      type: "section",
      ...(node.position && { position: node.position }),
      data: { hName: "section" },
      children: node.children,
    }),
  );
  expect(wrapped).toEqual(untouched);
});

test("a position that does not fit the source is dropped", () => {
  const outside = { start: { line: 99, column: 1 }, end: { line: 99, column: 2 } };
  expect(
    paragraphPositions((node, ctx) => ctx.replaceNode(node, { ...node, position: outside })),
  ).toEqual([undefined, undefined]);
});

test("a hast node a plugin copies keeps its position for later plugins", () => {
  const run = (copy: boolean) => {
    const seen: (Position | undefined)[] = [];
    markdownToHtml(source, {
      hastPlugins: [
        defineHastPlugin({
          name: "edit",
          options: { position: true },
          element: {
            filter: ["p"],
            visit(node, ctx) {
              if (copy) ctx.replaceNode(node, { ...node });
            },
          },
        }),
        defineHastPlugin({
          name: "read",
          options: { position: true },
          element: {
            filter: ["p"],
            visit(node) {
              seen.push(node.position);
            },
          },
        }),
      ],
    });
    return seen;
  };
  const untouched = run(false);
  expect(untouched).toHaveLength(2);
  expect(run(true)).toEqual(untouched);
});

test("copies of several nodes on one line keep their positions", () => {
  const run = (copy: boolean) => {
    const seen: (Position | undefined)[] = [];
    markdownToHtml("é *a* 😀 *b* *c*\n\nplain *d* *e*", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "edit",
          options: { position: true },
          emphasis(node, ctx) {
            if (copy) ctx.replaceNode(node, { ...node });
          },
        }),
        defineMdastPlugin({
          name: "read",
          options: { position: true },
          emphasis(node) {
            seen.push(node.position);
          },
        }),
      ],
    });
    return seen;
  };
  const untouched = run(false);
  expect(untouched).toHaveLength(5);
  expect(untouched).not.toContain(undefined);
  expect(run(true)).toEqual(untouched);
});
