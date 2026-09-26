import { describe, test } from "vitest";
import { assertMdxInlineStyleConformance } from "./helpers.js";

describe("MDX conformance: inline styles", () => {
  test("custom property preserves camelCase", async () => {
    await assertMdxInlineStyleConformance("hello", "p", "--tmLabel: 'a'; color: red");
  });

  test("custom property with uppercase letters", async () => {
    await assertMdxInlineStyleConformance("hello", "p", "--MyVar: 10px");
  });

  test("custom property mixed with vendor-prefixed and standard properties", async () => {
    await assertMdxInlineStyleConformance(
      "hello",
      "p",
      "--tmLabel: blue; -webkit-line-clamp: 2; background-color: red",
    );
  });
});
