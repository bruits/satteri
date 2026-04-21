import { describe, test } from "vitest";
import { assertExtMdastConformance, assertExtHastConformance } from "./helpers.js";

const FM: ["frontmatter"] = ["frontmatter"];

describe("Frontmatter MDAST conformance", () => {
  test("basic YAML frontmatter", () => {
    assertExtMdastConformance("---\ntitle: Hello\n---\n\nContent", FM);
  });

  test("YAML with multiple fields", () => {
    assertExtMdastConformance("---\ntitle: Test\ndate: 2024-01-01\ntags:\n  - a\n  - b\n---\n\nBody", FM);
  });

  test("empty YAML frontmatter", () => {
    assertExtMdastConformance("---\n---\n\nContent", FM);
  });

  test("YAML frontmatter only", () => {
    assertExtMdastConformance("---\ntitle: Hello\n---", FM);
  });

  test("TOML frontmatter", () => {
    assertExtMdastConformance("+++\ntitle = \"Hello\"\n+++\n\nContent", FM);
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
});

describe("Frontmatter HAST conformance", () => {
  test("basic YAML frontmatter", () => {
    assertExtHastConformance("---\ntitle: Hello\n---\n\nContent", FM);
  });

  test("TOML frontmatter", () => {
    assertExtHastConformance("+++\ntitle = \"Hello\"\n+++\n\nContent", FM);
  });

  test("frontmatter only", () => {
    assertExtHastConformance("---\ntitle: Hello\n---", FM);
  });

  test("empty YAML frontmatter", () => {
    assertExtHastConformance("---\n---\n\nContent", FM);
  });
});
