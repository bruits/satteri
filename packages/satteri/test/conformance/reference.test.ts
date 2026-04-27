import { describe, test } from "vitest";
import { assertMdastConformance, assertHastConformance, assertHtmlConformance } from "./helpers.js";

describe("MDAST conformance: link references", () => {
  test("full reference", () => {
    assertMdastConformance("[text][ref]\n\n[ref]: https://example.com");
  });

  test("full reference with title", () => {
    assertMdastConformance('[text][ref]\n\n[ref]: https://example.com "title"');
  });

  test("collapsed reference", () => {
    assertMdastConformance("[ref][]\n\n[ref]: https://example.com");
  });

  test("shortcut reference", () => {
    assertMdastConformance("[ref]\n\n[ref]: https://example.com");
  });

  test("identifier normalization: case fold", () => {
    assertMdastConformance("[FOO][]\n\n[foo]: https://example.com");
  });

  test("identifier normalization: whitespace collapse", () => {
    assertMdastConformance("[a  b\tc][]\n\n[a b c]: https://example.com");
  });

  test("multiple references", () => {
    assertMdastConformance(
      "[one] and [two][] and [three][ref]\n\n[one]: /one\n[two]: /two\n[ref]: /three",
    );
  });

  test("unresolved reference (no matching definition)", () => {
    assertMdastConformance("[missing][nope]");
  });

  test("unresolved shortcut reference", () => {
    assertMdastConformance("[missing]");
  });

  test("definition-only document", () => {
    assertMdastConformance("[unused]: https://example.com");
  });

  test("definition with title", () => {
    assertMdastConformance('[ref]: https://example.com "the title"');
  });

  test("reference with emphasis in label text", () => {
    assertMdastConformance("[*em*][ref]\n\n[ref]: https://example.com");
  });
});

describe("MDAST conformance: image references", () => {
  test("full reference", () => {
    assertMdastConformance("![alt text][ref]\n\n[ref]: https://example.com/img.png");
  });

  test("collapsed reference", () => {
    assertMdastConformance("![ref][]\n\n[ref]: https://example.com/img.png");
  });

  test("shortcut reference", () => {
    assertMdastConformance("![ref]\n\n[ref]: https://example.com/img.png");
  });

  test("image reference with title", () => {
    assertMdastConformance('![alt][ref]\n\n[ref]: https://example.com/img.png "a title"');
  });

  test("unresolved image reference", () => {
    assertMdastConformance("![alt][missing]");
  });
});

describe("HAST conformance: link references", () => {
  test("full reference renders as anchor", () => {
    assertHastConformance("[text][ref]\n\n[ref]: https://example.com");
  });

  test("shortcut reference renders as anchor", () => {
    assertHastConformance("[ref]\n\n[ref]: https://example.com");
  });

  test("reference with title renders title attribute", () => {
    assertHastConformance('[t][r]\n\n[r]: https://example.com "hello"');
  });

  test("definition does not render", () => {
    assertHastConformance("[ref]: https://example.com");
  });
});

describe("HAST conformance: image references", () => {
  test("full image reference renders as img", () => {
    assertHastConformance("![alt text][ref]\n\n[ref]: https://example.com/img.png");
  });

  test("shortcut image reference renders as img", () => {
    assertHastConformance("![ref]\n\n[ref]: https://example.com/img.png");
  });
});

describe("HTML conformance: references", () => {
  test("link reference round-trip", () => {
    assertHtmlConformance("[text][ref]\n\n[ref]: https://example.com");
  });

  test("image reference round-trip", () => {
    assertHtmlConformance("![alt][ref]\n\n[ref]: https://example.com/img.png");
  });

  test("shortcut link round-trip", () => {
    assertHtmlConformance("[ref]\n\n[ref]: https://example.com");
  });
});
