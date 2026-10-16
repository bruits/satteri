import { describe, test, expect } from "vitest";
import MarkdownIt from "markdown-it";
import markdownItAttrs from "markdown-it-attrs";
import { markdownToHtml } from "../../src/index.js";

// markdown-it-attrs is the reference: remark has no maintained attributes plugin.
const reference = new MarkdownIt().use(markdownItAttrs);

// markdown-it renders strikethrough as `<s>` where remark and Sätteri use `<del>`,
// whatever the attributes.
function referenceHtml(md: string): string {
  return reference
    .render(md)
    .trim()
    .replace(/<(\/?)s([ >])/g, "<$1del$2");
}

function satteriHtml(md: string): string {
  const result = markdownToHtml(md, { features: { attributes: true } });
  if (result instanceof Promise) throw new Error("expected sync");
  return result.html.trim();
}

describe("attributes: conformance vs markdown-it-attrs", () => {
  const CASES: Array<[string, string]> = [
    ["heading attributes in source order", "# Heading {.title #top}"],
    ["heading with a key-value and merged classes", "## Section {#s data-x=1 .a .b}"],
    ["fence: attribute class before the language class", "```js {.highlight}\nx\n```"],
    ["fence: class, id and key-value", "```ts {.foo #bar data-lang=typescript}\nx\n```"],
    ["fence: no language", "``` {.plain}\nx\n```"],
    ["fence: no space before the block", "```js{.a}\nx\n```"],
    ["fence: tilde fence", "~~~js {.a}\nx\n~~~"],
    ["fence: text before the block stays meta", '```js title="x" {.a}\nx\n```'],
    ["fence: only the last block is read", "```js {.a} {.b}\nx\n```"],
    ["fence: a block that doesn't end the info string", "```js {.a} extra\nx\n```"],
    ["fence: an empty block", "```js {}\nx\n```"],
    ["fence: an unclosed block", "```js {.a\nx\n```"],
    ["inline: emphasis, strong and a code span", "*em*{.a} **strong**{#b} `code`{c=d}"],
    [
      "inline: link, image and reference link",
      "[link](/u){.a} ![alt](/i.png){.b} [ref][r]{#c}\n\n[r]: /r",
    ],
    ["inline: strikethrough", "~~gone~~{.x}"],
    ["inline: nested elements", "**a *b*{.in}**{.out}"],
    ["inline: blocks that follow one another merge", "*em*{.a}{#i}{.b k=v}"],
    ["inline: markup characters inside a block", "*em*{.a_b}"],
    ["inline: a block after a space or plain text stays text", "*em* {.a} text{.b} more"],
    ["inline: an escaped, empty or unclosed block stays text", "*em*\\{.c} *em*{} *em*{.d"],
  ];

  for (const [name, md] of CASES) {
    test(name, () => {
      expect(satteriHtml(md)).toBe(referenceHtml(md));
    });
  }

  // The language is written before the braces, so its class comes first, in
  // source order; markdown-it-attrs appends it after the attributes instead.
  test("fence: the language class comes first, unlike the reference", () => {
    const md = "```python {#code-block}\nx\n```";
    expect(satteriHtml(md)).toBe(
      '<pre><code class="language-python" id="code-block">x\n</code></pre>',
    );
    expect(referenceHtml(md)).toBe(
      '<pre><code id="code-block" class="language-python">x\n</code></pre>',
    );
  });
});
