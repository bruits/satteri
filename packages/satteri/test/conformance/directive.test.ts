import { describe, test } from "vitest";
import { assertExtMdastConformance, assertExtMdastConformanceNoPosition } from "./helpers.js";

const DIR: ["directive"] = ["directive"];

describe("Directive MDAST conformance", () => {
  describe("container directives", () => {
    test("basic container", () => {
      assertExtMdastConformance(":::note\nContent here\n:::", DIR);
    });

    test("container with label", () => {
      assertExtMdastConformance(":::note[Title]\nContent\n:::", DIR);
    });

    test("container with attributes", () => {
      assertExtMdastConformance(":::warning{.big}\nBe careful!\n:::", DIR);
    });

    test("container with id shortcut", () => {
      assertExtMdastConformance(":::note{#my-note}\nContent\n:::", DIR);
    });

    test("container with multiple classes", () => {
      assertExtMdastConformance(":::note{.a .b .c}\nContent\n:::", DIR);
    });

    test("container with named attribute", () => {
      assertExtMdastConformance(':::note{key="value"}\nContent\n:::', DIR);
    });

    test("container with label and attributes", () => {
      assertExtMdastConformance(":::note[My Title]{.special}\nContent\n:::", DIR);
    });

    test("empty container", () => {
      assertExtMdastConformance(":::note\n:::", DIR);
    });

    test("nested containers", () => {
      assertExtMdastConformance("::::outer\n:::inner\nContent\n:::\n::::", DIR);
    });

    test("container with multiple paragraphs", () => {
      assertExtMdastConformance(":::note\nParagraph 1\n\nParagraph 2\n:::", DIR);
    });

    test("not a directive without name", () => {
      assertExtMdastConformance(":::\nJust text\n:::", DIR);
    });

    test("container with unquoted attribute value", () => {
      assertExtMdastConformance(":::note{key=value}\nContent\n:::", DIR);
    });

    test("container with single-quoted attribute", () => {
      assertExtMdastConformance(":::note{key='value'}\nContent\n:::", DIR);
    });

    test("closing fence closes through open list", () => {
      // Regression: the closing `:::` used to bail at any non-directive
      // ancestor (list, blockquote, …), trapping every subsequent block
      // inside the directive.
      assertExtMdastConformance(
        ":::tip\nintro\n\n- item a\n- item b\n:::\n\n## next\n\nafter\n",
        DIR,
      );
    });

    test("closing fence closes through open blockquote", () => {
      assertExtMdastConformance(":::note\n> quoted\n:::\n\nafter\n", DIR);
    });
  });

  describe("directive names: unicode", () => {
    // Regression: the name scanner used to treat every non-ASCII byte as a
    // valid name character, swallowing Japanese `。` and similar punctuation
    // into the name. Now uses unicode_id_continue, which rejects Po/Ps/etc.
    // These tests strip position because satteri counts bytes and remark
    // counts code points — unrelated to this fix.
    test("CJK letters in textDirective name", () => {
      assertExtMdastConformanceNoPosition("text :API를 more", DIR);
    });

    test("Japanese full-stop terminates directive name", () => {
      assertExtMdastConformanceNoPosition("text :word。more", DIR);
    });

    test("Han letters in directive name", () => {
      assertExtMdastConformanceNoPosition(":日本語\ncontent\n:::", DIR);
    });
  });

  describe("directive labels: inline code", () => {
    // Regression: directive labels used to be stored as a single raw Text
    // node. Post-pass now splits on backtick pairs so `:::tip[Set a \`x\`]`
    // renders an `inlineCode` child.
    test("inline code inside container directive label", () => {
      assertExtMdastConformance(":::tip[Set a `baseUrl`]\ncontent\n:::", DIR);
    });

    test("inline code inside leaf directive label", () => {
      assertExtMdastConformance("::video[See `baseUrl` option]{src=x}", DIR);
    });

    test("inline code inside text directive label", () => {
      assertExtMdastConformance("text :tip[Set a `baseUrl`] more", DIR);
    });

    test("multiple inline code spans in one label", () => {
      assertExtMdastConformance(":::note[Use `a` then `b` here]\nx\n:::", DIR);
    });
  });

  describe("leaf directives", () => {
    test("basic leaf", () => {
      assertExtMdastConformance("::video[Title]{src=video.mp4}", DIR);
    });

    test("leaf without label", () => {
      assertExtMdastConformance("::hr{.red}", DIR);
    });

    test("leaf with label only", () => {
      assertExtMdastConformance("::component[Some content]", DIR);
    });

    test("leaf with empty label", () => {
      assertExtMdastConformance("::component[]", DIR);
    });

    test("leaf with multiple attributes", () => {
      assertExtMdastConformance('::youtube[Video]{vid=abc123 width="100%"}', DIR);
    });

    test("leaf name only", () => {
      assertExtMdastConformance("::break", DIR);
    });
  });

  describe("text directives", () => {
    test("basic text directive", () => {
      assertExtMdastConformance('A :abbr[HTML]{title="HyperText Markup Language"} example.', DIR);
    });

    test("text directive with label only", () => {
      assertExtMdastConformance("This is :cite[smith04] reference.", DIR);
    });

    test("text directive with attrs only", () => {
      assertExtMdastConformance("This is :span{.highlight} text.", DIR);
    });

    test("text directive with empty label", () => {
      assertExtMdastConformance("This :name[] works.", DIR);
    });

    test("text directive with empty attrs", () => {
      assertExtMdastConformance("This :name{} works.", DIR);
    });

    test("bare name is not a text directive", () => {
      assertExtMdastConformance("This :smile is not a directive.", DIR);
    });

    test("colon emoji-style is not a directive", () => {
      assertExtMdastConformance("Hello :smile: world", DIR);
    });

    test("multiple text directives", () => {
      assertExtMdastConformance(
        "A :abbr[CSS]{title=Cascading} and :abbr[HTML]{title=HyperText} example.",
        DIR,
      );
    });
  });

  describe("edge cases", () => {
    test("directive at start of paragraph", () => {
      assertExtMdastConformance(":name[label]{key=val} at start.", DIR);
    });

    test("directive at end of paragraph", () => {
      assertExtMdastConformance("At end :name[label]{key=val}", DIR);
    });

    test("two colons is leaf not two text directives", () => {
      assertExtMdastConformance("::leaf[content]", DIR);
    });

    test("three colons is container", () => {
      assertExtMdastConformance(":::container\ncontent\n:::", DIR);
    });

    test("name with hyphens", () => {
      assertExtMdastConformance("::my-component[text]", DIR);
    });

    test("name with underscores", () => {
      assertExtMdastConformance("::my_component[text]", DIR);
    });
  });

  describe("closing fence indentation and context", () => {
    // Regressions for remark-directive's fence-closing rules:
    //   * up to 3 spaces of leading whitespace on the closing fence line
    //   * closing works across intervening list/listItem containers
    //   * a `:::` that is also valid blockquote content (prefixed by `>`)
    //     does NOT close an outer directive.

    test("closing fence indented 2 spaces inside a list", () => {
      assertExtMdastConformance(
        ":::caution[Slugs]\ntext\n\n- one\n- two\n  :::\n\n## After\n",
        DIR,
      );
    });

    test("closing fence indented 3 spaces at top level", () => {
      assertExtMdastConformance(":::note\ntext\n   :::\n", DIR);
    });

    test("closing fence indented 1 space at top level", () => {
      assertExtMdastConformance(":::note\ntext\n :::\n", DIR);
    });

    test("`> :::` inside blockquote does not close outer directive", () => {
      assertExtMdastConformance(":::container\n> text\n> :::\n> x\n:::\n", DIR);
    });
  });
});
