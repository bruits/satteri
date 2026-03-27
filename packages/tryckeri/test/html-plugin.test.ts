import { describe, test, expect } from "vitest";
import {
  parseToBuffer,
  parseToHastBuffer,
  mdastBufferToHastBuffer,
  hastBufferToHtmlStr,
  parseToHtml,
  applyMutations,
} from "../index.js";
import { MdastReader } from "../src/mdast-reader.js";
import { HastReader } from "../src/hast-reader.js";
import { DataMap } from "../src/data-map.js";
import { visitMdast } from "../src/visitor.js";
import { visitHast } from "../src/hast-visitor.js";
import { materializeHastTree } from "../src/hast-materializer.js";
import { runPluginsOnBuffer } from "../src/pipeline.js";
import type { MdastNode } from "../src/types.js";
import type { HastNode } from "../src/hast-materializer.js";
import type { HastVisitorContext } from "../src/hast-visitor.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Full pipeline: markdown → MDAST plugins → HAST → HTML string */
function markdownToHtml(source: string): string {
  return hastBufferToHtmlStr(parseToHastBuffer(source));
}

/** Pipeline with MDAST plugins applied before HAST conversion */
async function markdownToHtmlWithMdastPlugins(
  source: string,
  plugins: { instance: Record<string, unknown>; name: string }[],
): Promise<string> {
  const mdastBuf = parseToBuffer(source);
  const result = await runPluginsOnBuffer(mdastBuf, plugins);
  const hastBuf = mdastBufferToHastBuffer(
    result.buffer instanceof Uint8Array ? result.buffer : new Uint8Array(result.buffer),
  );
  return hastBufferToHtmlStr(hastBuf);
}

// =========================================================================
// PART 1: MDAST plugins that affect the Markdown → HTML result
// =========================================================================

describe("MDAST plugins affecting HTML output", () => {
  test("no plugins: simple markdown renders correct HTML", () => {
    const html = markdownToHtml("# Hello\n\nWorld");
    expect(html).toContain("<h1>");
    expect(html).toContain("Hello");
    expect(html).toContain("<p>");
    expect(html).toContain("World");
  });

  test("MDAST plugin that removes headings — heading disappears from HTML", async () => {
    const removeHeadings = {
      heading(_node: MdastNode, ctx: { removeNode(n: MdastNode): void }) {
        ctx.removeNode(_node);
      },
    };
    const html = await markdownToHtmlWithMdastPlugins("# Title\n\nKeep this paragraph", [
      { instance: removeHeadings, name: "remove-headings" },
    ]);
    expect(html).not.toContain("<h1>");
    expect(html).not.toContain("Title");
    expect(html).toContain("<p>");
    expect(html).toContain("Keep this paragraph");
  });

  test("MDAST plugin that replaces heading with paragraph — h1 becomes p in HTML", async () => {
    const replaceHeading = {
      heading(node: MdastNode) {
        return { type: "paragraph", children: node.children } as unknown as MdastNode;
      },
    };
    const html = await markdownToHtmlWithMdastPlugins("# Hello\n\nWorld", [
      { instance: replaceHeading, name: "heading-to-paragraph" },
    ]);
    expect(html).not.toContain("<h1>");
    // "Hello" should be in a <p> now
    expect(html).toContain("<p>");
    expect(html).toContain("Hello");
  });

  test("MDAST plugin chain: plugin 1 sets data, plugin 2 reads it (data persists)", async () => {
    let seenIdInPlugin2: string | null = null;
    const setId = {
      heading(node: MdastNode) {
        node.data = { id: "custom-id" };
      },
    };
    const readId = {
      heading(node: MdastNode) {
        seenIdInPlugin2 = (node.data as { id?: string } | null)?.id ?? null;
      },
    };
    await markdownToHtmlWithMdastPlugins("# Test\n\nBody", [
      { instance: setId, name: "set-id" },
      { instance: readId, name: "read-id" },
    ]);
    expect(seenIdInPlugin2).toBe("custom-id");
  });

  test("MDAST plugin removing a link — anchor disappears from HTML", async () => {
    const removeLinks = {
      link(_node: MdastNode, ctx: { removeNode(n: MdastNode): void }) {
        ctx.removeNode(_node);
      },
    };
    const html = await markdownToHtmlWithMdastPlugins("Visit [example](https://example.com) today", [
      { instance: removeLinks, name: "remove-links" },
    ]);
    expect(html).not.toContain("<a");
    expect(html).not.toContain("href");
    expect(html).not.toContain("example.com");
  });
});

// =========================================================================
// PART 2: HAST plugins that affect the HTML result
// =========================================================================

describe("HAST plugins affecting HTML output", () => {
  test("no HAST plugin: basic rendering is correct", () => {
    const html = markdownToHtml("**bold** and *italic*");
    expect(html).toContain("<strong>");
    expect(html).toContain("bold");
    expect(html).toContain("<em>");
    expect(html).toContain("italic");
  });

  test("HAST visitor sees all element nodes", async () => {
    const uint8 = parseToHastBuffer("# Title\n\n- one\n- two\n\n> quote");
    const reader = new HastReader(uint8);
    const dataMap = new DataMap();
    const tags: string[] = [];
    await visitHast(
      reader,
      {
        element(node: HastNode) {
          tags.push(node.tagName!);
        },
      },
      dataMap,
    );
    expect(tags).toContain("h1");
    expect(tags).toContain("ul");
    expect(tags).toContain("li");
    expect(tags).toContain("blockquote");
  });

  test("HAST visitor can inspect link properties (href)", async () => {
    const uint8 = parseToHastBuffer("[click](https://example.com)");
    const reader = new HastReader(uint8);
    const dataMap = new DataMap();
    const hrefs: string[] = [];
    await visitHast(
      reader,
      {
        element(node: HastNode) {
          if (node.tagName === "a" && node.properties?.href) {
            hrefs.push(node.properties.href as string);
          }
        },
      },
      dataMap,
    );
    expect(hrefs).toContain("https://example.com");
  });

  test("HAST visitor can identify images and their attributes", async () => {
    const uint8 = parseToHastBuffer('![alt text](image.png "my title")');
    const reader = new HastReader(uint8);
    const dataMap = new DataMap();
    let imgNode: HastNode | null = null;
    await visitHast(
      reader,
      {
        element(node: HastNode) {
          if (node.tagName === "img") {
            imgNode = node;
          }
        },
      },
      dataMap,
    );
    expect(imgNode).not.toBeNull();
    expect(imgNode!.properties!.src).toBe("image.png");
    expect(imgNode!.properties!.alt).toBe("alt text");
    expect(imgNode!.properties!.title).toBe("my title");
  });

  test("HAST visitor text() sees all text content", async () => {
    const uint8 = parseToHastBuffer("Hello **world**");
    const reader = new HastReader(uint8);
    const dataMap = new DataMap();
    const texts: string[] = [];
    await visitHast(
      reader,
      {
        text(node: HastNode) {
          texts.push(node.value!);
        },
      },
      dataMap,
    );
    expect(texts).toContain("Hello ");
    expect(texts).toContain("world");
  });

  test("HAST visitor: setProperty mutation is recorded for elements", async () => {
    const uint8 = parseToHastBuffer("# Hello");
    const reader = new HastReader(uint8);
    const dataMap = new DataMap();
    const result = await visitHast(
      reader,
      {
        element(node: HastNode, ctx: HastVisitorContext) {
          if (node.tagName === "h1") {
            ctx.setProperty(node, "id", "my-title");
          }
        },
      },
      dataMap,
    );
    expect(result.hasMutations).toBe(true);
    // Apply mutations and verify HTML output
    const newBuf = applyMutations(uint8, result.commandBuffer);
    const html = hastBufferToHtmlStr(newBuf);
    expect(html).toContain('id="my-title"');
  });

  test("HAST visitor: remove mutation removes element from result", async () => {
    const uint8 = parseToHastBuffer("# Keep\n\nRemove this");
    const reader = new HastReader(uint8);
    const dataMap = new DataMap();
    const result = await visitHast(
      reader,
      {
        element(node: HastNode, ctx: HastVisitorContext) {
          if (node.tagName === "p") {
            ctx.removeNode(node);
          }
        },
      },
      dataMap,
    );
    expect(result.hasMutations).toBe(true);
    const newBuf = applyMutations(uint8, result.commandBuffer);
    const html = hastBufferToHtmlStr(newBuf);
    expect(html).not.toContain("Remove this");
    expect(html).toContain("Keep");
  });

  test("HAST visitor: replace mutation swaps an element", async () => {
    const uint8 = parseToHastBuffer("# Hello");
    const reader = new HastReader(uint8);
    const dataMap = new DataMap();
    const result = await visitHast(
      reader,
      {
        element(node: HastNode) {
          if (node.tagName === "h1") {
            return {
              type: "element",
              _nodeId: -1,
              tagName: "h2",
              properties: {},
              children: node.children ?? [],
              data: null,
            };
          }
        },
      },
      dataMap,
    );
    expect(result.hasMutations).toBe(true);
    const newBuf = applyMutations(uint8, result.commandBuffer);
    const html = hastBufferToHtmlStr(newBuf);
    expect(html).toContain("<h2>");
    expect(html).not.toContain("<h1>");
  });

  test("HAST visitor: transformRoot receives complete tree with deep structure", async () => {
    const source = "# Title\n\n- item 1\n- item 2\n\n```js\ncode\n```";
    const uint8 = parseToHastBuffer(source);
    const reader = new HastReader(uint8);
    const dataMap = new DataMap();
    let rootChildren = 0;
    await visitHast(
      reader,
      {
        transformRoot(root: HastNode) {
          rootChildren = root.children!.length;
        },
      },
      dataMap,
    );
    // Should have h1, ul, and pre elements at the root level
    expect(rootChildren).toBeGreaterThanOrEqual(3);
  });

  test("HAST visitor: diagnostics from HAST plugins are collected", async () => {
    const uint8 = parseToHastBuffer("# Hello");
    const reader = new HastReader(uint8);
    const dataMap = new DataMap();
    const result = await visitHast(
      reader,
      {
        element(node: HastNode, ctx: HastVisitorContext) {
          if (node.tagName === "h1") {
            ctx.report({
              message: "headings should have IDs",
              node,
              severity: "warning",
            });
          }
        },
      },
      dataMap,
    );
    expect(result.diagnostics.length).toBe(1);
    expect(result.diagnostics[0]!.message).toBe("headings should have IDs");
    expect(result.diagnostics[0]!.severity).toBe("warning");
  });
});

// =========================================================================
// PART 3: NAPI HAST functions (end-to-end binary pipeline)
// =========================================================================

describe("NAPI HAST pipeline functions", () => {
  test("parseToHastBuffer returns valid HAST binary", () => {
    const uint8 = parseToHastBuffer("Hello");
    expect(uint8).toBeInstanceOf(Uint8Array);
    expect(uint8.length).toBeGreaterThan(44); // at least header
    // Verify magic bytes (MDAR as LE u32)
    const view = new DataView(uint8.buffer, uint8.byteOffset);
    expect(view.getUint32(0, true)).toBe(0x5241444d);
  });

  test("mdastBufferToHastBuffer converts MDAST → HAST", () => {
    const mdast = parseToBuffer("# Test\n\nParagraph");
    const hast = mdastBufferToHastBuffer(mdast);
    expect(hast).toBeInstanceOf(Uint8Array);
    const reader = new HastReader(hast);
    expect(reader.nodeCount).toBeGreaterThan(0);
  });

  test("hastBufferToHtmlStr produces valid HTML from HAST buffer", () => {
    const hast = parseToHastBuffer("**bold**");
    const html = hastBufferToHtmlStr(hast);
    expect(html).toContain("<strong>");
    expect(html).toContain("bold");
    expect(html).toContain("</strong>");
  });

  test("parseToHtml produces same result as the 3-step pipeline", () => {
    const source = "# Hello\n\nA [link](https://example.com) here.\n\n> blockquote";
    const singleCall = parseToHtml(source);
    const threeStep = hastBufferToHtmlStr(
      mdastBufferToHastBuffer(parseToBuffer(source)),
    );
    expect(singleCall).toBe(threeStep);
  });

  test("full pipeline handles HTML entities and special characters", () => {
    const html = markdownToHtml('Use `<div>` and `"quotes"` & ampersands');
    // Text content should be escaped
    expect(html).toContain("&amp;");
    // Code content should be escaped
    expect(html).toContain("<code>");
  });

  test("full pipeline handles tables", () => {
    const md = "| A | B |\n|---|---|\n| 1 | 2 |";
    const html = markdownToHtml(md);
    expect(html).toContain("<table>");
    expect(html).toContain("<th>");
    expect(html).toContain("<td>");
  });

  test("full pipeline handles nested lists", () => {
    const md = "- a\n  - b\n    - c";
    const html = markdownToHtml(md);
    expect(html).toContain("<ul>");
    expect(html).toContain("<li>");
  });

  test("HastReader reads correct node count", () => {
    const uint8 = parseToHastBuffer("# Hello\n\nWorld");
    const reader = new HastReader(uint8);
    // root + h1 + text("Hello") + p + text("World") + possible newlines
    expect(reader.nodeCount).toBeGreaterThanOrEqual(5);
  });
});

// =========================================================================
// PART 4: Combined MDAST + HAST plugin scenarios
// =========================================================================

describe("combined MDAST + HAST plugin scenarios", () => {
  test("MDAST plugin removes heading, HAST tree reflects the removal", async () => {
    const removeHeadings = {
      heading(_node: MdastNode, ctx: { removeNode(n: MdastNode): void }) {
        ctx.removeNode(_node);
      },
    };
    // Run MDAST plugins
    const mdastBuf = parseToBuffer("# Gone\n\nStays");
    const result = await runPluginsOnBuffer(mdastBuf, [
      { instance: removeHeadings, name: "remove-headings" },
    ]);
    // Convert to HAST and inspect
    const hastBuf = mdastBufferToHastBuffer(
      result.buffer instanceof Uint8Array ? result.buffer : new Uint8Array(result.buffer),
    );
    const reader = new HastReader(hastBuf);
    const dataMap = new DataMap();
    const tree = materializeHastTree(reader, dataMap);
    // Should not have an h1 element
    const hasH1 = tree.children!.some((c) => c.tagName === "h1");
    expect(hasH1).toBe(false);
    // Should still have a p element
    const hasP = tree.children!.some((c) => c.tagName === "p");
    expect(hasP).toBe(true);
  });

  test("MDAST plugin replaces heading → HAST sees paragraph instead of h1", async () => {
    const replaceHeading = {
      heading(node: MdastNode) {
        return { type: "paragraph", children: node.children } as unknown as MdastNode;
      },
    };
    const mdastBuf = parseToBuffer("# Was Heading\n\nParagraph");
    const result = await runPluginsOnBuffer(mdastBuf, [
      { instance: replaceHeading, name: "heading-to-paragraph" },
    ]);
    const hastBuf = mdastBufferToHastBuffer(
      result.buffer instanceof Uint8Array ? result.buffer : new Uint8Array(result.buffer),
    );
    const html = hastBufferToHtmlStr(hastBuf);
    expect(html).not.toContain("<h1>");
    expect(html).toContain("<p>");
    expect(html).toContain("Was Heading");
  });

  test("HAST visitor can inspect the result of MDAST plugin transforms", async () => {
    // MDAST plugin removes all links
    const removeLinks = {
      link(_node: MdastNode, ctx: { removeNode(n: MdastNode): void }) {
        ctx.removeNode(_node);
      },
    };
    const mdastBuf = parseToBuffer("See [link](https://example.com) here");
    const result = await runPluginsOnBuffer(mdastBuf, [
      { instance: removeLinks, name: "remove-links" },
    ]);
    const hastBuf = mdastBufferToHastBuffer(
      result.buffer instanceof Uint8Array ? result.buffer : new Uint8Array(result.buffer),
    );
    const reader = new HastReader(hastBuf);
    const dataMap = new DataMap();
    // HAST visitor should not find any <a> elements
    const tags: string[] = [];
    await visitHast(
      reader,
      {
        element(node: HastNode) {
          tags.push(node.tagName!);
        },
      },
      dataMap,
    );
    expect(tags).not.toContain("a");
  });
});
