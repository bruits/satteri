import { describe, test, expect } from "vitest";
import {
  parseToHtml,
  createHastHandle,
  createMdastHandle,
  serializeHandle,
  renderHandle,
  dropHandle,
  convertMdastToHastHandle,
  getHandleSource,
} from "../index.js";
import { HastReader } from "../src/hast/hast-reader.js";
import { visitHastHandle, resolveSubscriptions } from "../src/hast/hast-visitor.js";
import { markdownToHtml, defineMdastPlugin } from "../src/index.js";
import type { MdastNode } from "../src/types.js";
import type { HastNode } from "../src/hast/hast-materializer.js";
import type { HastVisitorContext, HastVisitorInstance } from "../src/hast/hast-visitor.js";

// Helpers

/** Create a HAST reader from source (handle-based) */
function makeHastReader(source: string): {
  reader: HastReader;
  handle: ReturnType<typeof createHastHandle>;
} {
  const handle = createHastHandle(source);
  const buf = serializeHandle(handle);
  return { reader: new HastReader(buf), handle };
}

// PART 1: MDAST plugins that affect the Markdown → HTML result

describe("MDAST plugins affecting HTML output", () => {
  test("no plugins: simple markdown renders correct HTML", () => {
    const { html } = markdownToHtml("# Hello\n\nWorld");
    expect(html).toContain("<h1>");
    expect(html).toContain("Hello");
    expect(html).toContain("<p>");
    expect(html).toContain("World");
  });

  test("MDAST plugin that removes headings - heading disappears from HTML", () => {
    const removeHeadings = defineMdastPlugin({
      name: "remove-headings",
      heading(node, ctx) {
        ctx.removeNode(node);
      },
    });
    const { html } = markdownToHtml("# Title\n\nKeep this paragraph", {
      mdastPlugins: [removeHeadings],
    });
    expect(html).not.toContain("<h1>");
    expect(html).not.toContain("Title");
    expect(html).toContain("<p>");
    expect(html).toContain("Keep this paragraph");
  });

  test("MDAST plugin that replaces heading with paragraph - h1 becomes p in HTML", () => {
    const replaceHeading = defineMdastPlugin({
      name: "heading-to-paragraph",
      heading(node) {
        if (node.type === "heading") {
          return { type: "paragraph", children: node.children };
        }
      },
    });
    const { html } = markdownToHtml("# Hello\n\nWorld", { mdastPlugins: [replaceHeading] });
    expect(html).not.toContain("<h1>");
    // "Hello" should be in a <p> now
    expect(html).toContain("<p>");
    expect(html).toContain("Hello");
  });

  test("MDAST plugin chain: plugin 1 sets data, plugin 2 reads it (data persists)", () => {
    let seenIdInPlugin2: string | null = null;
    const setId = defineMdastPlugin({
      name: "set-id",
      heading(node, ctx) {
        ctx.setProperty(node, "data", { id: "custom-id" });
      },
    });
    const readId = defineMdastPlugin({
      name: "read-id",
      heading(node) {
        seenIdInPlugin2 = (node.data as { id?: string } | null)?.id ?? null;
      },
    });
    markdownToHtml("# Test\n\nBody", { mdastPlugins: [setId, readId] });
    expect(seenIdInPlugin2).toBe("custom-id");
  });

  test("MDAST plugin chain: data survives rebuild when another node is mutated", () => {
    let seenIdInPlugin2: string | null = null;
    // Plugin 1: sets data on heading AND mutates a different node (text → bold)
    const setDataAndMutate = defineMdastPlugin({
      name: "set-and-mutate",
      heading(node, ctx) {
        ctx.setProperty(node, "data", { id: "survives-rebuild" });
      },
      text(node, ctx) {
        // Mutating text forces a rebuild, node IDs change
        ctx.setProperty(node, "value", "mutated");
      },
    });
    // Plugin 2: reads the data set by plugin 1 (after rebuild)
    const readData = defineMdastPlugin({
      name: "read-data",
      heading(node) {
        seenIdInPlugin2 = (node.data as { id?: string } | null)?.id ?? null;
      },
    });
    markdownToHtml("# Title\n\nBody text", { mdastPlugins: [setDataAndMutate, readData] });
    expect(seenIdInPlugin2).toBe("survives-rebuild");
  });

  test("MDAST plugin removing a link - anchor disappears from HTML", () => {
    const removeLinks = defineMdastPlugin({
      name: "remove-links",
      link(node, ctx) {
        ctx.removeNode(node);
      },
    });
    const { html } = markdownToHtml("Visit [example](https://example.com) today", {
      mdastPlugins: [removeLinks],
    });
    expect(html).not.toContain("<a");
    expect(html).not.toContain("href");
    expect(html).not.toContain("example.com");
  });
});

// PART 2: HAST plugins that affect the HTML result

describe("HAST plugins affecting HTML output", () => {
  test("no HAST plugin: basic rendering is correct", () => {
    const { html } = markdownToHtml("**bold** and *italic*");
    expect(html).toContain("<strong>");
    expect(html).toContain("bold");
    expect(html).toContain("<em>");
    expect(html).toContain("italic");
  });

  test("HAST visitor sees all element nodes", () => {
    const handle = createHastHandle("# Title\n\n- one\n- two\n\n> quote");
    const source = getHandleSource(handle);
    const tags: string[] = [];
    const plugin: HastVisitorInstance = {
      element: {
        filter: [] as string[],
        visit(node) {
          if (node.type === "element") tags.push(node.tagName);
        },
      },
    };
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    dropHandle(handle);
    expect(tags).toContain("h1");
    expect(tags).toContain("ul");
    expect(tags).toContain("li");
    expect(tags).toContain("blockquote");
  });

  test("HAST visitor can inspect link properties (href)", () => {
    const handle = createHastHandle("[click](https://example.com)");
    const source = getHandleSource(handle);
    const hrefs: string[] = [];
    const plugin: HastVisitorInstance = {
      element: {
        filter: ["a"],
        visit(node) {
          if (node.type === "element" && node.tagName === "a" && node.properties?.href) {
            hrefs.push(node.properties.href as string);
          }
        },
      },
    };
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    dropHandle(handle);
    expect(hrefs).toContain("https://example.com");
  });

  test("HAST visitor can identify images and their attributes", () => {
    const handle = createHastHandle('![alt text](image.png "my title")');
    const source = getHandleSource(handle);
    let imgNode: HastNode | null = null;
    const plugin: HastVisitorInstance = {
      element: {
        filter: ["img"],
        visit(node) {
          if (node.type === "element" && node.tagName === "img") {
            imgNode = node;
          }
        },
      },
    };
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    dropHandle(handle);
    expect(imgNode).not.toBeNull();
    const img = imgNode!;
    if (img.type !== "element") throw new Error("expected element");
    expect(img.properties.src).toBe("image.png");
    expect(img.properties.alt).toBe("alt text");
    expect(img.properties.title).toBe("my title");
  });

  test("HAST visitor text() sees all text content", () => {
    const handle = createHastHandle("Hello **world**");
    const source = getHandleSource(handle);
    const texts: string[] = [];
    const plugin: HastVisitorInstance = {
      text(node) {
        if (node.type === "text") texts.push(node.value);
      },
    };
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    dropHandle(handle);
    expect(texts).toContain("Hello ");
    expect(texts).toContain("world");
  });

  test("HAST visitor: setProperty mutation is applied to elements", () => {
    const handle = createHastHandle("# Hello");
    const source = getHandleSource(handle);
    const plugin: HastVisitorInstance = {
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.setProperty(node, "id", "my-title");
        },
      },
    };
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    const html = renderHandle(handle);
    dropHandle(handle);
    expect(html).toContain('id="my-title"');
  });

  test("HAST visitor: remove mutation removes element from result", () => {
    const handle = createHastHandle("# Keep\n\nRemove this");
    const source = getHandleSource(handle);
    const plugin: HastVisitorInstance = {
      element: {
        filter: ["p"],
        visit(node, ctx) {
          ctx.removeNode(node);
        },
      },
    };
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    const html = renderHandle(handle);
    dropHandle(handle);
    expect(html).not.toContain("Remove this");
    expect(html).toContain("Keep");
  });

  test("HAST visitor: replace mutation swaps an element", () => {
    const handle = createHastHandle("# Hello");
    const source = getHandleSource(handle);
    const plugin: HastVisitorInstance = {
      element: {
        filter: ["h1"],
        visit(node) {
          if (node.type === "element" && node.tagName === "h1") {
            return {
              type: "element" as const,
              _nodeId: -1,
              tagName: "h2",
              properties: {},
              children: node.children ?? [],
              data: undefined,
            };
          }
        },
      },
    };
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    const html = renderHandle(handle);
    dropHandle(handle);
    expect(html).toContain("<h2>");
    expect(html).not.toContain("<h1>");
  });

  test("HAST visitor: diagnostics from HAST plugins are collected", () => {
    const handle = createHastHandle("# Hello");
    const source = getHandleSource(handle);
    let diags: { message: string; severity: string }[] = [];
    const plugin: HastVisitorInstance = {
      element: {
        filter: ["h1"],
        visit(node, ctx) {
          ctx.report({ message: "headings should have IDs", node, severity: "warning" });
          diags = ctx.getDiagnostics();
        },
      },
    };
    visitHastHandle(handle, plugin, resolveSubscriptions(plugin), source, undefined);
    dropHandle(handle);
    expect(diags.length).toBe(1);
    expect(diags[0]!.message).toBe("headings should have IDs");
    expect(diags[0]!.severity).toBe("warning");
  });
});

// PART 3: Handle-based HAST pipeline

describe("Handle-based HAST pipeline", () => {
  test("serializeHandle returns valid HAST binary", () => {
    const handle = createHastHandle("Hello");
    const buf = serializeHandle(handle);
    expect(buf).toBeInstanceOf(Uint8Array);
    expect(buf.length).toBeGreaterThan(44); // at least header
    // Verify magic bytes (MDAR as LE u32)
    const view = new DataView(buf.buffer, buf.byteOffset);
    expect(view.getUint32(0, true)).toBe(0x5241444d);
    dropHandle(handle);
  });

  test("convertMdastToHastHandle produces valid HAST", () => {
    const mdastHandle = createMdastHandle("# Test\n\nParagraph");
    const hastHandle = convertMdastToHastHandle(mdastHandle);
    const buf = serializeHandle(hastHandle);
    const reader = new HastReader(buf);
    expect(reader.nodeCount).toBeGreaterThan(0);
    dropHandle(hastHandle);
  });

  test("renderHandle produces valid HTML", () => {
    const handle = createHastHandle("**bold**");
    const html = renderHandle(handle);
    dropHandle(handle);
    expect(html).toContain("<strong>");
    expect(html).toContain("bold");
    expect(html).toContain("</strong>");
  });

  test("parseToHtml produces same result as handle pipeline", () => {
    const source = "# Hello\n\nA [link](https://example.com) here.\n\n> blockquote";
    const singleCall = parseToHtml(source);
    const handle = createHastHandle(source);
    const handleResult = renderHandle(handle);
    dropHandle(handle);
    expect(singleCall).toBe(handleResult);
  });

  test("full pipeline handles HTML entities and special characters", () => {
    const { html } = markdownToHtml('Use `<div>` and `"quotes"` & ampersands');
    // Text content should be escaped
    expect(html).toContain("&amp;");
    // Code content should be escaped
    expect(html).toContain("<code>");
  });

  test("full pipeline handles tables", () => {
    const md = "| A | B |\n|---|---|\n| 1 | 2 |";
    const { html } = markdownToHtml(md);
    expect(html).toContain("<table>");
    expect(html).toContain("<th>");
    expect(html).toContain("<td>");
  });

  test("full pipeline handles nested lists", () => {
    const md = "- a\n  - b\n    - c";
    const { html } = markdownToHtml(md);
    expect(html).toContain("<ul>");
    expect(html).toContain("<li>");
  });

  test("HastReader reads correct node count", () => {
    const { reader, handle } = makeHastReader("# Hello\n\nWorld");
    dropHandle(handle);
    // root + h1 + text("Hello") + p + text("World") + possible newlines
    expect(reader.nodeCount).toBeGreaterThanOrEqual(5);
  });
});

// PART 4: Combined MDAST + HAST plugin scenarios

describe("combined MDAST + HAST plugin scenarios", () => {
  test("MDAST plugin removes heading, HAST tree reflects the removal", () => {
    const { html } = markdownToHtml("# Gone\n\nStays", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "remove-headings",
          heading(_node, ctx) {
            ctx.removeNode(_node);
          },
        }),
      ],
    });
    expect(html).not.toContain("<h1>");
    expect(html).not.toContain("Gone");
    expect(html).toContain("<p>");
    expect(html).toContain("Stays");
  });

  test("MDAST plugin replaces heading → HAST sees paragraph instead of h1", () => {
    const { html } = markdownToHtml("# Was Heading\n\nParagraph", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "heading-to-paragraph",
          heading(node) {
            if (node.type === "heading") {
              return { type: "paragraph", children: node.children };
            }
          },
        }),
      ],
    });
    expect(html).not.toContain("<h1>");
    expect(html).toContain("<p>");
    expect(html).toContain("Was Heading");
  });

  test("HAST visitor can inspect the result of MDAST plugin transforms", () => {
    const { html } = markdownToHtml("See [link](https://example.com) here", {
      mdastPlugins: [
        defineMdastPlugin({
          name: "remove-links",
          link(_node, ctx) {
            ctx.removeNode(_node);
          },
        }),
      ],
    });
    // HAST result should not have any <a> elements
    expect(html).not.toContain("<a");
    expect(html).toContain("here");
  });
});
