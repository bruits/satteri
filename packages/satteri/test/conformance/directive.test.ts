import { describe, test, expect } from "vitest";
import { assertExtMdastConformance } from "./helpers.js";
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
    test("CJK letters in textDirective name", () => {
      assertExtMdastConformance("text :API를 more", DIR);
    });

    test("Japanese full-stop terminates directive name", () => {
      assertExtMdastConformance("text :word。more", DIR);
    });

    test("Han letters in directive name", () => {
      assertExtMdastConformance(":日本語\ncontent\n:::", DIR);
    });
  });

  describe("directive labels: inline code", () => {
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

    test("a reference label ending inside a directive's attributes", () => {
      assertExtMdastConformance('[a][:x{k="]"}]\n\n[:x{k="]: /u\n', DIR);
    });

    test("directive attached to preceding word with no space", () => {
      assertExtMdastConformance("Add is:inline to the slot.", DIR);
    });
  });

  describe("text directives: ports after a bare URL", () => {
    test("underscore in the penultimate label keeps the port directive", () => {
      assertExtMdastConformance("http://a_b.c:4321/x", DIR);
    });

    test("underscored host with a real port keeps the port directive", () => {
      assertExtMdastConformance("http://my_app.localhost:3000/admin", DIR);
    });

    test("underscored host in running prose", () => {
      assertExtMdastConformance("see http://a_b.c:4321/x end", DIR);
    });

    test("unknown scheme keeps the port directive", () => {
      assertExtMdastConformance("foo://bar:80/x", DIR);
    });

    test("letter before the scheme keeps the port directive", () => {
      assertExtMdastConformance("xhttp://example.com:3000/x", DIR);
    });

    test("linkable host swallows the port into the URL", () => {
      assertExtMdastConformance("http://example.com:3000/x", DIR);
    });

    test("dotless linkable host swallows the port into the URL", () => {
      assertExtMdastConformance("http://localhost:3000/x", DIR);
    });

    test("numeric directive with no URL before it", () => {
      assertExtMdastConformance("Port :3000 here.", DIR);
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

  describe("text directives: colon boundary in headings", () => {
    test("ASCII colon + CJK id_start consumes rest as directive name", () => {
      assertExtMdastConformance("## ガイド付き例:GatsbyレイアウトをAstroへ変換する", DIR);
    });

    test("ASCII colon + space leaves colon as plain text", () => {
      assertExtMdastConformance("## 参考: Astro構文への変換する", DIR);
    });

    test("full-width colon never triggers directive parsing", () => {
      assertExtMdastConformance("### ヒント：JSXファイルでReactコンポーネントを定義する方法", DIR);
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
      assertExtMdastConformance("## Punct colon:。後ろ", DIR);
    });
  });

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
      const directive = labelChildren("see :abbr[a <b>c</b> d] end", 1)[1];
      const jsx = directive.children[1];
      expect(directive.type).toBe("textDirective");
      expect(jsx.type).toBe("mdxJsxTextElement");
      expect(jsx.data?._mdxExplicitJsx).toBe(true);
    });
  });
});
