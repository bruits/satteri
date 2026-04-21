import { describe, test } from "vitest";
import { assertExtMdastConformance, assertExtHastConformance } from "./helpers.js";

const DL: ["definitionList"] = ["definitionList"];

describe("Definition list MDAST conformance", () => {
  test("basic definition list", () => {
    assertExtMdastConformance("apple\n: red fruit\n\norange\n: orange fruit", DL);
  });

  test("multiple definitions per term", () => {
    assertExtMdastConformance("apple\n: red fruit\n: computer company", DL);
  });

  test("definition with paragraph content", () => {
    assertExtMdastConformance("apple\n: red fruit\n\n  contains seeds", DL);
  });

  test("multiple terms and definitions", () => {
    assertExtMdastConformance("apple\n: red fruit\n\norange\n: orange fruit\n\nbanana\n: yellow fruit", DL);
  });

  test("definition with inline formatting", () => {
    assertExtMdastConformance("*apple*\n: **red** fruit", DL);
  });

  test("definition with code block", () => {
    assertExtMdastConformance("term\n:   definition\n\n        code block", DL);
  });

  test("not a definition list without colon", () => {
    assertExtMdastConformance("just a paragraph\nwith text", DL);
  });

  test("definition list after paragraph", () => {
    assertExtMdastConformance("Some text.\n\napple\n: red fruit", DL);
  });
});

describe("Definition list HAST conformance", () => {
  test("basic definition list", () => {
    assertExtHastConformance("apple\n: red fruit\n\norange\n: orange fruit", DL);
  });

  test("multiple definitions per term", () => {
    assertExtHastConformance("apple\n: red fruit\n: computer company", DL);
  });

  test("definition with inline formatting", () => {
    assertExtHastConformance("*apple*\n: **red** fruit", DL);
  });

  test("definition list after paragraph", () => {
    assertExtHastConformance("Some text.\n\napple\n: red fruit", DL);
  });
});
