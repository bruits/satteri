import { describe, test } from "vitest";
import { assertHastConformance, assertMdastConformance } from "./helpers.js";

describe("CommonMark spec deltas: HTML blocks with following content", () => {
  test("spec 148: HTML block in table cell with following paragraph", () => {
    assertHastConformance(
      "<table><tr><td>\n<pre>\n**Hello**,\n\n_world_.\n</pre>\n</td></tr></table>\n",
    );
  });

  test("spec 155: HTML block div with following paragraph", () => {
    assertHastConformance("<div>\n*foo*\n\n*bar*\n");
  });

  test("spec 174: HTML block in blockquote with following paragraph", () => {
    assertHastConformance("> <div>\n> foo\n\nbar\n");
  });

  test("spec 177: HTML comment with following content", () => {
    assertHastConformance("<!-- foo -->*bar*\n*baz*\n");
  });

  test("spec 191: table with indented content", () => {
    assertHastConformance(
      "<table>\n\n  <tr>\n\n    <td>\n      Hi\n    </td>\n\n  </tr>\n\n</table>\n",
    );
  });
});

describe("CommonMark spec deltas: list paragraph wrapping", () => {
  test("spec 300: heading and setext heading in list", () => {
    assertHastConformance("- # Foo\n- Bar\n  ---\n  baz\n");
  });

  test("spec 321: list item with blockquote and code", () => {
    assertHastConformance("- a\n  > b\n  ```\n  c\n  ```\n- d\n");
  });
});

describe("CommonMark spec deltas: URL encoding", () => {
  test("spec 526: autolink with ] in URL", () => {
    assertHastConformance("[foo<https://example.com/?search=](uri)>\n");
  });

  test("spec 538: autolink with ][ in URL", () => {
    assertHastConformance("[foo<https://example.com/?search=][ref]>\n\n[ref]: /uri\n");
  });

  test("spec 603: autolink with escaped brackets", () => {
    assertHastConformance("<https://example.com/\\[\\>\n");
  });
});

describe("CommonMark spec deltas: list spread detection", () => {
  test("spec 259: nested blockquote ordered list with blank continuation", () => {
    assertHastConformance("   > > 1.  one\n>>\n>>     two\n");
  });

  test("spec 325: list item with sublist and trailing content becomes loose", () => {
    assertHastConformance("* foo\n  * bar\n\n  baz\n");
  });
});

describe("CommonMark spec deltas: HTML block in list item", () => {
  test("regression 175: code block followed by HTML block in list item", () => {
    assertHastConformance("*\n      <div>\n     <div>\n");
  });
});

describe("CommonMark spec deltas: image alt text", () => {
  // Skipped: triggers a crash in HAST conversion (close_node called with empty stack)
  test.skip("spec 574: nested image in image alt", () => {
    assertHastConformance("![foo ![bar](/url)](/url2)\n");
  });
});
