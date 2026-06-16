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
