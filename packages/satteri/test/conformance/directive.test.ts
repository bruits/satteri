import { describe, test, expect } from "vitest";
import { assertExtMdastConformance, assertExtMdastConformanceNoPosition } from "./helpers.js";
import { mdxToMdast } from "../../src/index.js";

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

  describe("directive labels: emphasis, strong, links", () => {
    // Regression (Issue 3): labels are inline-parsed in full, not just for
    // backticks, so emphasis/strong/links resolve the way remark renders them.
    test("strong with nested emphasis in container label", () => {
      assertExtMdastConformance(":::note[Custom **strong with _emphasis_** Label]\nx\n:::", DIR);
    });

    test("emphasis in leaf directive label", () => {
      assertExtMdastConformance("::video[A *great* clip]{src=x}", DIR);
    });

    test("emphasis in text directive label", () => {
      assertExtMdastConformance("text :tip[be *careful* now] more", DIR);
    });

    test("link inside a container directive label", () => {
      assertExtMdastConformance(":::tip[See [docs](https://example.com) now]\nx\n:::", DIR);
    });

    test("mixed inline code and emphasis in one label", () => {
      assertExtMdastConformance(":::note[Set `x` and **really** mean it]\nx\n:::", DIR);
    });

    test("plain label stays a single text node", () => {
      assertExtMdastConformance(":::note[Just plain words]\nx\n:::", DIR);
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

    test("directive attached to preceding word with no space", () => {
      // Regression: prose like `is:inline` (an Astro attribute name written
      // bare, not in backticks) parses as text + textDirective `:inline`.
      // Both remark and satteri must agree, so plugin payloads carrying this
      // directive round-trip through the JS<->Rust JSON boundary.
      assertExtMdastConformance("Add is:inline to the slot.", DIR);
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

  // The character class around a `:` decides whether it begins a text
  // directive. This matters for downstream slug/anchor generation: a
  // textDirective's `name` is metadata, not a Text child, so heading slugs
  // computed from `textContent` will silently truncate when a colon is
  // (mis)read as starting a directive. A real-world hit was a Japanese
  // heading where `:GatsbyレイアウトをAstro…` was consumed as a directive
  // named `GatsbyレイアウトをAstro…`, leaving the slug as just `ガイド付き例`.
  describe("text directives: colon boundary in headings", () => {
    test("ASCII colon + CJK id_start consumes rest as directive name", () => {
      assertExtMdastConformanceNoPosition("## ガイド付き例:GatsbyレイアウトをAstroへ変換する", DIR);
    });

    test("ASCII colon + space leaves colon as plain text", () => {
      assertExtMdastConformanceNoPosition("## 参考: Astro構文への変換する", DIR);
    });

    test("full-width colon never triggers directive parsing", () => {
      assertExtMdastConformanceNoPosition(
        "### ヒント：JSXファイルでReactコンポーネントを定義する方法",
        DIR,
      );
    });

    test("ASCII colon + latin letter consumes following word as directive", () => {
      assertExtMdastConformance("## Section:Followed by latin", DIR);
    });

    test("ASCII colon + ASCII digit is a directive name (digits are name-start)", () => {
      assertExtMdastConformance("## Colon then digit:1234 next", DIR);
    });

    test("ASCII colon at end of heading is plain text", () => {
      assertExtMdastConformance("## Trailing colon:", DIR);
    });

    test("ASCII colon + non-id punctuation (CJK full stop) leaves colon as text", () => {
      assertExtMdastConformanceNoPosition("## Punct colon:。後ろ", DIR);
    });
  });

  // MDX: JSX inside a directive label is tokenized by the parser (no remark
  // reference here since it lacks remark-directive), and source-parsed JSX must
  // keep its `_mdxExplicitJsx` marking, as the deleted label pass used to set.
  describe("MDX JSX inside directive labels", () => {
    const labelChildren = (md: string, depth: number): any[] => {
      let node: any = mdxToMdast(md, { features: { directive: true } });
      for (let i = 0; i < depth; i++) node = node.children[0];
      return node.children;
    };

    test("container directive label", () => {
      const [, jsx] = labelChildren(":::note[The <code>x</code> prop]\nbody\n:::", 2);
      expect(jsx.type).toBe("mdxJsxTextElement");
      expect(jsx.name).toBe("code");
      expect(jsx.data?._mdxExplicitJsx).toBe(true);
    });

    test("text directive label", () => {
      // root > paragraph > [text, textDirective[text, jsx, text], text]
      const directive = labelChildren("see :abbr[a <b>c</b> d] end", 1)[1];
      const jsx = directive.children[1];
      expect(directive.type).toBe("textDirective");
      expect(jsx.type).toBe("mdxJsxTextElement");
      expect(jsx.data?._mdxExplicitJsx).toBe(true);
    });
  });
});
