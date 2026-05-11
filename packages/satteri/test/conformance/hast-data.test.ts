import { describe, test, expect } from "vitest";
import { markdownToHtml, defineHastPlugin } from "../../src/index.js";
import type { HastNode } from "../../src/types.js";

// Plugin-to-plugin signaling on hast nodes via the free-form `data` field.
// All cases must round-trip identically whether `data` was set on an existing
// node via `ctx.setProperty` or carried in on a freshly emitted node.

interface SignalData {
  origin?: string;
  level?: number;
  flags?: string[];
  meta?: { kind: string; payload: { count: number } };
}

function freshElement(
  tagName: string,
  data: SignalData,
  children: HastNode["type"] extends "element" ? HastNode[] : never,
): HastNode;
function freshElement(tagName: string, data: SignalData, children: HastNode[]): HastNode {
  return {
    type: "element",
    tagName,
    properties: {},
    children,
    data,
  } as HastNode;
}

function freshText(value: string): HastNode {
  return { type: "text", value } as HastNode;
}

describe("HAST plugin data round-trip (existing-node path via setProperty)", () => {
  test("data set by a plugin is visible to the next plugin", () => {
    const tag = defineHastPlugin({
      name: "tag",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.setProperty(node, "data", { origin: "h1-tag" });
        },
      },
    });
    const consume = defineHastPlugin({
      name: "consume",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          const origin = (node.data as SignalData | undefined)?.origin;
          if (origin) ctx.setProperty(node, "data-origin", origin);
        },
      },
    });
    const { html } = markdownToHtml("# Hi", { hastPlugins: [tag, consume] });
    expect(html).toContain('data-origin="h1-tag"');
  });

  test("nested data values (objects, arrays, numbers) round-trip intact", () => {
    let received: SignalData | undefined;
    const tag = defineHastPlugin({
      name: "tag",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.setProperty(node, "data", {
            level: 42,
            flags: ["bold", "highlighted"],
            meta: { kind: "title", payload: { count: 3 } },
          });
        },
      },
    });
    const consume = defineHastPlugin({
      name: "consume",
      element: {
        filter: ["h1"],
        visit(node) {
          received = node.data as SignalData | undefined;
        },
      },
    });
    markdownToHtml("# Hi", { hastPlugins: [tag, consume] });
    expect(received).toEqual({
      level: 42,
      flags: ["bold", "highlighted"],
      meta: { kind: "title", payload: { count: 3 } },
    });
  });

  test("setting data to null clears it", () => {
    let after: unknown;
    const setThenClear = defineHastPlugin({
      name: "set-then-clear",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.setProperty(node, "data", { origin: "doomed" });
          ctx.setProperty(node, "data", null);
        },
      },
    });
    const inspect = defineHastPlugin({
      name: "inspect",
      element: {
        filter: ["h1"],
        visit(node) {
          after = node.data;
        },
      },
    });
    markdownToHtml("# Hi", { hastPlugins: [setThenClear, inspect] });
    expect(after == null).toBe(true);
  });
});

describe("HAST plugin data round-trip (fresh-node path)", () => {
  test("data on a fresh element from replaceNode is readable", () => {
    let received: SignalData | undefined;
    const replace = defineHastPlugin({
      name: "replace",
      element: {
        filter: ["h1"],
        visit(node) {
          return freshElement("section", { origin: "replaced" }, node.children as HastNode[]);
        },
      },
    });
    const inspect = defineHastPlugin({
      name: "inspect",
      element: {
        filter: ["section"],
        visit(node) {
          received = node.data as SignalData | undefined;
        },
      },
    });
    markdownToHtml("# Hi", { hastPlugins: [replace, inspect] });
    expect(received?.origin).toBe("replaced");
  });

  test("data on a fresh element from insertBefore is readable", () => {
    let received: SignalData | undefined;
    const insert = defineHastPlugin({
      name: "insert",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.insertBefore(node, freshElement("nav", { origin: "toc" }, [freshText("TOC")]));
        },
      },
    });
    const inspect = defineHastPlugin({
      name: "inspect",
      element: {
        filter: ["nav"],
        visit(node) {
          received = node.data as SignalData | undefined;
        },
      },
    });
    markdownToHtml("# Hi", { hastPlugins: [insert, inspect] });
    expect(received?.origin).toBe("toc");
  });

  test("data on a fresh element from prependChild is readable", () => {
    let received: SignalData | undefined;
    const prepend = defineHastPlugin({
      name: "prepend",
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.prependChild(node, freshElement("span", { origin: "anchor" }, [freshText("§")]));
        },
      },
    });
    const inspect = defineHastPlugin({
      name: "inspect",
      element: {
        filter: ["span"],
        visit(node) {
          received = node.data as SignalData | undefined;
        },
      },
    });
    markdownToHtml("# Hi", { hastPlugins: [prepend, inspect] });
    expect(received?.origin).toBe("anchor");
  });

  test("nested fresh subtree carries data on every level", () => {
    const seen: Record<string, SignalData | undefined> = {};
    const replace = defineHastPlugin({
      name: "replace",
      element: {
        filter: ["h1"],
        visit() {
          return freshElement("section", { origin: "outer" }, [
            freshElement("header", { origin: "header" }, [freshText("Title")]),
            freshElement("article", { origin: "article" }, [freshText("Body")]),
          ]);
        },
      },
    });
    const inspect = defineHastPlugin({
      name: "inspect",
      element: {
        filter: [],
        visit(node) {
          if (node.type !== "element") return;
          const data = node.data as SignalData | undefined;
          if (data?.origin) seen[node.tagName] = data;
        },
      },
    });
    markdownToHtml("# Hi", { hastPlugins: [replace, inspect] });
    expect(seen.section?.origin).toBe("outer");
    expect(seen.header?.origin).toBe("header");
    expect(seen.article?.origin).toBe("article");
  });

  test("data with deeply nested values on a fresh element round-trips intact", () => {
    let received: SignalData | undefined;
    const replace = defineHastPlugin({
      name: "replace",
      element: {
        filter: ["h1"],
        visit(node) {
          return freshElement(
            "section",
            {
              origin: "complex",
              level: 7,
              flags: ["a", "b", "c"],
              meta: { kind: "note", payload: { count: 99 } },
            },
            node.children as HastNode[],
          );
        },
      },
    });
    const inspect = defineHastPlugin({
      name: "inspect",
      element: {
        filter: ["section"],
        visit(node) {
          received = node.data as SignalData | undefined;
        },
      },
    });
    markdownToHtml("# Hi", { hastPlugins: [replace, inspect] });
    expect(received).toEqual({
      origin: "complex",
      level: 7,
      flags: ["a", "b", "c"],
      meta: { kind: "note", payload: { count: 99 } },
    });
  });
});
