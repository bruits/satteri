import { describe, test } from "vitest";
import { assertExtMdastConformance, assertExtHastConformance } from "./helpers.js";

const FM: ["frontmatter"] = ["frontmatter"];

describe("Frontmatter MDAST conformance", () => {
  test("basic YAML frontmatter", () => {
    assertExtMdastConformance("---\ntitle: Hello\n---\n\nContent", FM);
  });

  test("YAML with multiple fields", () => {
    assertExtMdastConformance(
      "---\ntitle: Test\ndate: 2024-01-01\ntags:\n  - a\n  - b\n---\n\nBody",
      FM,
    );
  });

  test("empty YAML frontmatter", () => {
    assertExtMdastConformance("---\n---\n\nContent", FM);
  });

  test("YAML frontmatter only", () => {
    assertExtMdastConformance("---\ntitle: Hello\n---", FM);
  });

  test("TOML frontmatter", () => {
    assertExtMdastConformance('+++\ntitle = "Hello"\n+++\n\nContent', FM);
  });

  test("no frontmatter", () => {
    assertExtMdastConformance("Just a paragraph", FM);
  });

  test("thematic break not confused with frontmatter", () => {
    assertExtMdastConformance("Paragraph\n\n---\n\nAnother paragraph", FM);
  });

  test("frontmatter with blank lines in value", () => {
    assertExtMdastConformance("---\ndescription: |\n  Line one\n  Line two\n---\n\nContent", FM);
  });

  test("frontmatter with blank line as first content line", () => {
    // Regression: the metadata-block scanner used to bail when the first
    // line after the opening `---` was blank, treating it as a thematic
    // break + heading instead of a yaml block.
    assertExtMdastConformance("---\n\ntitle: test\n---\n", FM);
  });

  test("frontmatter with multiple blank lines", () => {
    assertExtMdastConformance("---\n\n\ntitle: test\n---\n", FM);
  });

  test("frontmatter preserves CRLF line endings in value", () => {
    // Regression: metadata blocks used to go through `append_code_text`,
    // which normalizes CRLF→LF (correct for code blocks, wrong for
    // frontmatter — remark keeps `\r\n` in `yaml.value`).
    assertExtMdastConformance("---\r\ntitle: X\r\nauthor: Y\r\n---\r\n", FM);
  });
});

describe("Frontmatter HAST conformance", () => {
  test("basic YAML frontmatter", () => {
    assertExtHastConformance("---\ntitle: Hello\n---\n\nContent", FM);
  });

  test("TOML frontmatter", () => {
    assertExtHastConformance('+++\ntitle = "Hello"\n+++\n\nContent', FM);
  });

  test("frontmatter only", () => {
    assertExtHastConformance("---\ntitle: Hello\n---", FM);
  });

  test("empty YAML frontmatter", () => {
    assertExtHastConformance("---\n---\n\nContent", FM);
  });
});
