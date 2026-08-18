import { describe, test } from "vitest";
import { assertFootnoteHastConformance } from "./helpers.js";

const SINGLE = "Text with footnote[^1].\n\n[^1]: First footnote.\n";
const MULTI = "See[^a] and[^a] again.\n\n[^a]: Shared note.\n";
const TRIPLE = "Ref[^x], again[^x], once more[^x].\n\n[^x]: Triple-referenced.\n";
const MIXED = "Two refs[^a] and one[^b], plus[^a].\n\n[^a]: A note.\n\n[^b]: B note.\n";

describe("GFM footnote options conformance (vs remark-rehype)", () => {
  test("default options match remark-rehype (single ref)", () => {
    assertFootnoteHastConformance(SINGLE);
  });

  test("default options match remark-rehype (repeated ref)", () => {
    assertFootnoteHastConformance(MULTI);
  });

  test("default options match remark-rehype (mixed refs)", () => {
    assertFootnoteHastConformance(MIXED);
  });

  test("custom label matches", () => {
    assertFootnoteHastConformance(SINGLE, { label: "Notes de bas de page" });
    assertFootnoteHastConformance(MULTI, { label: "Notes de bas de page" });
  });

  test("custom backLabel substitutes {reference} on first backref", () => {
    assertFootnoteHastConformance(SINGLE, {
      backLabel: "Retour à la référence {reference}",
    });
  });

  test("custom backLabel substitutes {reference} with suffix on reused refs", () => {
    assertFootnoteHastConformance(MULTI, {
      backLabel: "Retour à la référence {reference}",
    });
    assertFootnoteHastConformance(TRIPLE, {
      backLabel: "Retour à la référence {reference}",
    });
  });

  test("custom backContent matches (single ref, no sup)", () => {
    assertFootnoteHastConformance(SINGLE, { backContent: "haut" });
  });

  test("custom backContent matches (repeated refs append <sup>K</sup>)", () => {
    assertFootnoteHastConformance(MULTI, { backContent: "haut" });
    assertFootnoteHastConformance(TRIPLE, { backContent: "haut" });
  });

  test("all three customizations together", () => {
    assertFootnoteHastConformance(MIXED, {
      label: "Notes de bas de page",
      backContent: "↑",
      backLabel: "Retour à la référence {reference}",
    });
  });

  test("non-ASCII strings round-trip cleanly", () => {
    assertFootnoteHastConformance(MULTI, {
      label: "脚注",
      backContent: "↩",
      backLabel: "返回引用 {reference}",
    });
  });

  test("backLabel callback matches remark-rehype's callback shape", () => {
    assertFootnoteHastConformance(MULTI, {
      backLabel: (n, k) => (k > 1 ? `Retour ${n}-${k}` : `Retour ${n}`),
    });
    assertFootnoteHastConformance(TRIPLE, {
      backLabel: (n, k) => (k > 1 ? `Retour ${n}-${k}` : `Retour ${n}`),
    });
  });

  test("backContent callback owns content (no auto-sup) and matches remark-rehype", () => {
    assertFootnoteHastConformance(MULTI, {
      backContent: (_n, k) => (k === 1 ? "↑" : `↑${k}`),
    });
  });

  test("callback can branch off the reference number", () => {
    assertFootnoteHastConformance(MIXED, {
      backLabel: (n, k) => {
        const ordinal = n === 1 ? "premier" : n === 2 ? "deuxième" : `n°${n}`;
        return k > 1 ? `${ordinal} appel ${k}` : `${ordinal} appel`;
      },
    });
  });
});

// Identifiers reach `href`/`id` through micromark's `normalizeUri` safe set.
const IDENTIFIERS = [
  "plain",
  'a"b',
  "a<b",
  "a>b",
  "a&b",
  "a'b",
  "a`b",
  "a{b}c",
  "a|b",
  "a\\b",
  "a^b",
  "a?b",
  "a#b",
  "a/b",
  "a%b",
  "a%%b",
  "ab%4",
  "a%41b",
  "a%2gb",
  "café",
  "café",
  "CAFÉ",
  "straße",
  "ΣΣ",
  "Ωμέγα",
  "日本",
  "한국어",
  "עברית",
  "🎉",
];

// Escaped so the precomposed and combining forms of "café" get distinct titles.
const title = (id: string): string =>
  id.replace(/[^\x20-\x7e]/g, (c) => `\\u${c.codePointAt(0)!.toString(16).padStart(4, "0")}`);

describe("GFM footnote identifier conformance (vs remark-rehype)", () => {
  for (const id of IDENTIFIERS) {
    // One per paragraph: two in a row let identifier punctuation pair into a code span.
    test(`identifier "${title(id)}" matches remark`, () => {
      assertFootnoteHastConformance(`Once[^${id}].\n\nTwice[^${id}].\n\n[^${id}]: Note.\n`);
    });
  }

  test("identifiers differing only by case resolve to one footnote", () => {
    assertFootnoteHastConformance("Upper[^Foo] lower[^foo].\n\n[^foo]: Shared.\n");
  });

  test("distinct identifiers that encode differently stay distinct", () => {
    assertFootnoteHastConformance("One[^a<b] two[^a>b].\n\n[^a<b]: Less.\n\n[^a>b]: Greater.\n");
  });

  test("identifier punctuation that opens an inline construct stays inert", () => {
    for (const id of ["a*b", "a_b", "a~b", "a[b", "a]b", "a<b", "a>b", "a$b", "a!b", "a\\b"]) {
      assertFootnoteHastConformance(`Once[^${id}] and twice[^${id}].\n\n[^${id}]: Note.\n`);
    }
  });

  test("emphasis around two references on one line keeps both", () => {
    assertFootnoteHastConformance("*Once[^a*b] and twice[^a*b]*.\n\n[^a*b]: Note.\n");
  });

  // Divergence: CommonMark gives code spans precedence, so the backticks pair and
  // satteri emits `Once[^a<code>b] and twice[^a</code>b].` — GitHub also produces no
  // reference here. remark scans footnote calls before code spans, so it keeps both.
  test.fails("two backtick identifiers on one line keep both references", () => {
    assertFootnoteHastConformance("Once[^a`b] and twice[^a`b].\n\n[^a`b]: Note.\n");
  });

  test("a backtick identifier is a reference when the backticks cannot pair", () => {
    assertFootnoteHastConformance("Once[^a`b] end.\n\n[^a`b]: Note.\n");
    assertFootnoteHastConformance("A `code` then[^a`b].\n\n[^a`b]: Note.\n");
  });
});

describe("GFM footnote semantics conformance (vs remark-rehype)", () => {
  test("numbering follows first reference, not definition order", () => {
    assertFootnoteHastConformance("First[^b] then[^a].\n\n[^a]: Note A.\n\n[^b]: Note B.\n");
  });

  test("a definition nothing references is omitted", () => {
    assertFootnoteHastConformance("Plain text.\n\n[^unused]: Nobody points here.\n");
  });

  test("a reference with no definition stays literal text", () => {
    assertFootnoteHastConformance("Dangling[^missing] text.\n");
  });

  test("an identifier containing a space is not a footnote", () => {
    assertFootnoteHastConformance("Text[^a b].\n\n[^a b]: Spaced id.\n");
  });

  test("duplicate definitions keep the first", () => {
    assertFootnoteHastConformance("Text[^a].\n\n[^a]: First wins.\n\n[^a]: Second loses.\n");
  });

  test("a definition can hold several blocks", () => {
    assertFootnoteHastConformance("Text[^a].\n\n[^a]: First para.\n\n    Second para.\n");
  });

  // remark appends the backref after a non-paragraph last block; satteri keeps it in the paragraph.
  test.fails("a definition ending in a list puts the backref after the list", () => {
    assertFootnoteHastConformance("Text[^a].\n\n[^a]: Body.\n\n    - one\n    - two\n");
  });

  test.fails("a definition ending in a list and code block places the backref last", () => {
    assertFootnoteHastConformance(
      "Text[^a].\n\n[^a]: Body.\n\n    - one\n    - two\n\n        code\n",
    );
  });

  test("a definition can reference another footnote", () => {
    assertFootnoteHastConformance("Text[^a].\n\n[^a]: Note A with[^b].\n\n[^b]: Note B.\n");
  });

  test("a reference inside emphasis keeps its numbering", () => {
    assertFootnoteHastConformance("*Emphasised[^a]* and plain[^b].\n\n[^a]: A.\n\n[^b]: B.\n");
  });

  test("references inside a list item are numbered in document order", () => {
    assertFootnoteHastConformance("- one[^b]\n- two[^a]\n\n[^a]: A.\n\n[^b]: B.\n");
  });

  test("inline markup inside a definition is parsed", () => {
    assertFootnoteHastConformance(
      "Text[^a].\n\n[^a]: With *em* and [link](https://example.com).\n",
    );
  });

  test("a definition body can be empty", () => {
    assertFootnoteHastConformance("Text[^a].\n\n[^a]:\n");
  });
});
