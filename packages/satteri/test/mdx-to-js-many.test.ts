import { describe, test, expect } from "vitest";
import { mdxToJs, mdxToJsMany } from "../src/index.js";

describe("mdxToJsMany", () => {
  test("matches mdxToJs output item by item", () => {
    const sources = [
      "# Hello\n\nSome *text* with a [link](https://example.com).\n",
      "---\ntitle: Doc\n---\n\n<Note kind=\"tip\">JSX content</Note>\n",
      "export const x = 1;\n\nValue is {x}.\n",
      "",
    ];
    const batch = mdxToJsMany(sources);
    expect(batch).toHaveLength(sources.length);
    for (let i = 0; i < sources.length; i++) {
      const single = mdxToJs(sources[i] as string);
      expect(batch[i]?.error).toBeUndefined();
      expect(batch[i]?.code).toBe(single.code);
      expect(batch[i]?.frontmatter).toEqual(single.frontmatter);
    }
  });

  test("a failing item reports its error without sinking the batch", () => {
    const bad = "<Unclosed>\n";
    const good = "fine\n";
    let expected = "";
    try {
      mdxToJs(bad);
    } catch (e) {
      expected = String((e as Error).message);
    }
    expect(expected).not.toBe("");
    const batch = mdxToJsMany([good, bad, good]);
    expect(batch[0]?.code).toBeDefined();
    expect(batch[1]?.code).toBeUndefined();
    expect(batch[1]?.error).toBe(expected);
    expect(batch[2]?.code).toBeDefined();
  });

  test("passes compile options through", () => {
    const src = "# H\n";
    const batch = mdxToJsMany([src], { jsxImportSource: "preact" });
    const single = mdxToJs(src, { jsxImportSource: "preact" });
    expect(batch[0]?.code).toBe(single.code);
    expect(batch[0]?.code).toContain("preact");
  });

  test("handles a large batch in parallel without reordering", () => {
    const sources = Array.from({ length: 200 }, (_, i) => `# Doc ${i}\n\nParagraph ${i}.\n`);
    const batch = mdxToJsMany(sources);
    for (let i = 0; i < sources.length; i++) {
      expect(batch[i]?.code).toContain(`Doc ${i}`);
    }
  });
});
