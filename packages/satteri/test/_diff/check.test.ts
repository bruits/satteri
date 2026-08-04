import { test } from "vitest";
import { assertMdastConformance, assertHtmlConformance } from "../conformance/helpers.js";

test("fnr-adjacent VT cases reconcile", () => {
  assertMdastConformance("\u{b}www.a.com b\n");
  assertMdastConformance("\u{c}www.a.com b\n");
  assertHtmlConformance("\u{b}www.a.com b\n");
});
