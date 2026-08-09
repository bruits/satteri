// Deliberate divergence (see website/content/docs/divergences.md): find-and-replace
// autolinks carry source positions here and none in remark. A wrong position is
// worse than an absent one, so these assert the exact span.

import { describe, test, expect } from "vitest";
import {
  createMdastHandle,
  dropHandle,
  materializeMdastTree,
  MdastReader,
  serializeHandle,
} from "../../src/index.js";
import { satteriMdast, referenceMdast } from "./helpers.js";
import { markdownToMdast } from "../../src/index.js";

interface Positioned {
  type: string;
  value?: string;
  url?: string;
  children?: Positioned[];
  position?: {
    start: { line: number; column: number; offset: number };
    end: { line: number; column: number; offset: number };
  };
}

function flatten(tree: unknown): Positioned[] {
  const out: Positioned[] = [];
  const walk = (node: Positioned): void => {
    out.push(node);
    for (const child of node.children ?? []) walk(child);
  };
  walk(tree as Positioned);
  return out;
}

/** Every emitted node, paired with the source text its span covers. */
function spans(md: string): Array<[string, string, string]> {
  return spansOf(md, satteriMdast(md));
}

function spansOf(md: string, tree: unknown): Array<[string, string, string]> {
  return flatten(tree)
    .filter((n) => n.type === "link" || n.type === "text")
    .map((n) => [
      n.type,
      String(n.url ?? n.value),
      n.position ? md.slice(n.position.start.offset, n.position.end.offset) : "NO POSITION",
    ]);
}

function firstLink(tree: unknown): Positioned {
  const link = flatten(tree).find((n) => n.type === "link");
  expect(link, "expected a link node").toBeDefined();
  return link!;
}

/** The reference must keep taking the find-and-replace path, or the case moved. */
function expectReferenceTakesFnr(md: string): void {
  expect(
    firstLink(referenceMdast(md)).position,
    `${md}: reference no longer uses findAndReplace`,
  ).toBeUndefined();
}

describe("find-and-replace autolink positions", () => {
  test("1. a value identical to its source", () => {
    const md = "[www.a.com/x";
    expectReferenceTakesFnr(md);
    expect(spans(md)).toEqual([
      ["text", "[", "["],
      ["link", "http://www.a.com/x", "www.a.com/x"],
      ["text", "www.a.com/x", "www.a.com/x"],
    ]);
  });

  test("2. a match spanning a character reference covers it whole", () => {
    const md = "[www.a.com/&amp;b";
    expectReferenceTakesFnr(md);
    // The URL stays decoded, as in remark; only the span is raw.
    expect(spans(md)).toEqual([
      ["text", "[", "["],
      ["link", "http://www.a.com/&b", "www.a.com/&amp;b"],
      ["text", "www.a.com/&b", "www.a.com/&amp;b"],
    ]);
  });

  test("3. a match starting at a character reference starts at the `&`", () => {
    // The trigger `h` is the reference's output, so the raw source begins at `&`.
    const md = "[&#104;ttp://x.y";
    expectReferenceTakesFnr(md);
    expect(spans(md)).toEqual([
      ["text", "[", "["],
      ["link", "http://x.y", "&#104;ttp://x.y"],
      ["text", "http://x.y", "&#104;ttp://x.y"],
    ]);
  });

  test("3b. a `www` trigger supplied by a character reference", () => {
    const md = "[&#119;ww.x.y";
    expectReferenceTakesFnr(md);
    expect(spans(md)).toEqual([
      ["text", "[", "["],
      ["link", "http://www.x.y", "&#119;ww.x.y"],
      ["text", "www.x.y", "&#119;ww.x.y"],
    ]);
  });

  test("3c. an `@` trigger supplied by a character reference", () => {
    const md = "[a&#64;b.com";
    expectReferenceTakesFnr(md);
    expect(spans(md)).toEqual([
      ["text", "[", "["],
      ["link", "mailto:a@b.com", "a&#64;b.com"],
      ["text", "a@b.com", "a&#64;b.com"],
    ]);
  });

  test("4. a backslash escape inside the match", () => {
    const md = "[www.x.y/a\\_b";
    expectReferenceTakesFnr(md);
    expect(spans(md)).toEqual([
      ["text", "[", "["],
      ["link", "http://www.x.y/a_b", "www.x.y/a\\_b"],
      ["text", "www.x.y/a_b", "www.x.y/a\\_b"],
    ]);
  });

  test("5. a table cell maps through pipe unescaping", () => {
    const md = "| a |\n| - |\n| [www.x.y\\|z |";
    expectReferenceTakesFnr(md);
    expect(spans(md)).toEqual([
      ["text", "a", "a"],
      ["text", "[", "["],
      ["link", "http://www.x.y|z", "www.x.y\\|z"],
      ["text", "www.x.y|z", "www.x.y\\|z"],
    ]);
  });

  test("6. a match on a continuation line reports that line", () => {
    const md = "[a\nwww.x.y";
    expectReferenceTakesFnr(md);
    const link = firstLink(satteriMdast(md));
    expect(link.position).toEqual({
      start: { line: 2, column: 1, offset: 3 },
      end: { line: 2, column: 8, offset: 10 },
    });
  });

  test("7. a continuation prefix belongs to neither neighbour", () => {
    const md = "> [a\n> www.x.y";
    expectReferenceTakesFnr(md);
    expect(spans(md)).toEqual([
      ["text", "[a\n", "[a\n"],
      ["link", "http://www.x.y", "www.x.y"],
      ["text", "www.x.y", "www.x.y"],
    ]);
    const link = firstLink(satteriMdast(md));
    expect(link.position!.start).toEqual({ line: 2, column: 3, offset: 7 });
  });

  test("7b. a lone carriage return ends the line the prefix follows", () => {
    const md = "> [a\r> www.x.y";
    expectReferenceTakesFnr(md);
    expect(spans(md)).toEqual([
      ["text", "[a\r", "[a\r"],
      ["link", "http://www.x.y", "www.x.y"],
      ["text", "www.x.y", "www.x.y"],
    ]);
  });

  test("8. a multi-character reference is included whole or not at all", () => {
    // `&fjlig;` decodes to two characters, so a boundary inside it has no raw
    // offset to name; here it falls within the match.
    const md = "[www.a.com/&fjlig;b";
    expectReferenceTakesFnr(md);
    expect(spans(md)).toEqual([
      ["text", "[", "["],
      ["link", "http://www.a.com/fjb", "www.a.com/&fjlig;b"],
      ["text", "www.a.com/fjb", "www.a.com/&fjlig;b"],
    ]);
  });

  test("9. the splitUrl trail is its own node, with its own exact span", () => {
    const md = "[www.x.y&amp;";
    expectReferenceTakesFnr(md);
    expect(spans(md)).toEqual([
      ["text", "[", "["],
      ["link", "http://www.x.y", "www.x.y"],
      ["text", "www.x.y", "www.x.y"],
      ["text", "&", "&amp;"],
    ]);
  });

  test("10. offsets are UTF-16 indices at the JS boundary", () => {
    const md = "你好[www.x.y";
    expectReferenceTakesFnr(md);
    const link = firstLink(satteriMdast(md));
    // Three UTF-16 units precede the trigger, not the seven bytes it occupies.
    expect(link.position!.start.offset).toBe(3);
    expect(md.slice(link.position!.start.offset, link.position!.end.offset)).toBe("www.x.y");
  });

  test("11. skip-positions mode builds no map and reports nothing", () => {
    const md = "[www.a.com&amp;b";
    const handle = createMdastHandle(md, { frontmatter: false, math: false }, false);
    try {
      const tree = materializeMdastTree(new MdastReader(serializeHandle(handle)));
      for (const node of flatten(JSON.parse(JSON.stringify(tree)))) {
        expect(node.position, `${node.type} should carry no position`).toBeUndefined();
      }
    } finally {
      dropHandle(handle);
    }
  });
});

// Smart punctuation rewrites the text a find-and-replace autolink is found in,
// so the decoded value no longer matches its source span byte for byte. The
// alignment undoes the three rewrites, and these pin that it lands exactly —
// an approximate span would be worse than the absent one it replaces.
describe("smart punctuation does not cost the autolink its position", () => {
  const smart = (md: string) =>
    spansOf(
      md,
      markdownToMdast(md, {
        features: { frontmatter: false, math: false, smartPunctuation: true },
      }),
    );

  test.each([
    // Inside the URL: the value curls, the span still covers the raw run.
    ["[www.a.com/a--b", "http://www.a.com/a\u{2013}b", "www.a.com/a--b"],
    ["[www.a.com/a...b", "http://www.a.com/a\u{2026}b", "www.a.com/a...b"],
    ['[www.a.com/a"b', "http://www.a.com/a\u{201c}b", 'www.a.com/a"b'],
    // The `%6` em/en formula, which the alignment shares with the first pass.
    ["[www.a.com/a----b", "http://www.a.com/a\u{2013}\u{2013}b", "www.a.com/a----b"],
    ["[www.a.com/a-----b", "http://www.a.com/a\u{2014}\u{2013}b", "www.a.com/a-----b"],
  ])("%j", (md, url, slice) => {
    const link = smart(md).find(([type]) => type === "link");
    expect(link).toEqual(["link", url, slice]);
  });

  test("a rewrite before or after the match keeps every span exact", () => {
    expect(smart('"q" [www.a.com/x')).toEqual([
      ["text", "\u{201c}q\u{201d} [", '"q" ['],
      ["link", "http://www.a.com/x", "www.a.com/x"],
      ["text", "www.a.com/x", "www.a.com/x"],
    ]);
    expect(smart("[www.a.com/x -- y")).toEqual([
      ["text", "[", "["],
      ["link", "http://www.a.com/x", "www.a.com/x"],
      ["text", "www.a.com/x", "www.a.com/x"],
      ["text", " \u{2013} y", " -- y"],
    ]);
  });

  test("it composes with the alignment's existing rules", () => {
    // A continuation prefix, a character reference, and an escape that keeps
    // its `-` out of the dash run.
    expect(smart('> "q"\n> [www.a.com/x').at(1)).toEqual([
      "link",
      "http://www.a.com/x",
      "www.a.com/x",
    ]);
    expect(smart("-- [www.a.com/&amp;b").at(1)).toEqual([
      "link",
      "http://www.a.com/&b",
      "www.a.com/&amp;b",
    ]);
    expect(smart('[www.a.com/a\\"b').at(1)).toEqual([
      "link",
      'http://www.a.com/a"b',
      'www.a.com/a\\"b',
    ]);
  });

  test("a lone carriage return keeps the alignment on the rails", () => {
    expect(smart('> "q"\r> [www.a.com/x').at(1)).toEqual([
      "link",
      "http://www.a.com/x",
      "www.a.com/x",
    ]);
  });

  test("the construct path is untouched: smart punctuation never enters a URL", () => {
    expect(smart("www.a.com/a--b")).toEqual([
      ["link", "http://www.a.com/a--b", "www.a.com/a--b"],
      ["text", "www.a.com/a--b", "www.a.com/a--b"],
    ]);
  });
});
