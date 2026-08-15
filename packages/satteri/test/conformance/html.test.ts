import { describe, test } from "vitest";
import { assertHtmlConformance } from "./helpers.js";

describe("HTML conformance: list spread detection", () => {
  test("spec 259: nested blockquote ordered list with blank continuation", () => {
    assertHtmlConformance("   > > 1.  one\n>>\n>>     two\n");
  });

  test("spec 325: list item with sublist and trailing content becomes loose", () => {
    assertHtmlConformance("* foo\n  * bar\n\n  baz\n");
  });
});

describe("HTML conformance: HTML block in list item", () => {
  test("regression 175: code block followed by HTML block in list item", () => {
    assertHtmlConformance("*\n      <div>\n     <div>\n");
  });
});

describe("HTML conformance: leading BOM", () => {
  test("a leading BOM never renders", () => {
    assertHtmlConformance("\u{feff}# h\n\ntext\n");
    assertHtmlConformance("\u{feff}- one\n- two\n");
  });
});
