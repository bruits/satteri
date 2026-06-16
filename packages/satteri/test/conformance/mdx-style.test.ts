import { describe, test } from "vitest";
import { assertMdxInlineStyleConformance } from "./helpers.js";

// Inline `style="…"` strings produced by hast plugins are parsed into JSX style
// objects during HAST→JSX compilation. CSS custom properties (`--*`) are
// case-sensitive, so their casing must be preserved — unlike standard property
// names, which are case-insensitive. Regression test for
// https://github.com/withastro/astro/issues/16940, where satteri-expressive-code
// emits `--tmLabel` and satteri lowercased it to `--tmlabel`, breaking
// `var(--tmLabel)` references in MDX (but not in plain `.md`).
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
